#!/usr/bin/env bash
# Smoke test on a throwaway kind cluster: builds the image, deploys the Helm chart, a mock ENTSO-E
# API and an OpenTelemetry Collector, then asserts that price metrics reach the collector.
#
# Requires: docker (buildx), kind, kubectl, helm. Set KEEP_CLUSTER=1 to keep the cluster afterwards.
set -euo pipefail

CLUSTER="${CLUSTER:-spotprice-smoke}"
# Every kubectl and helm call is pinned to this context. The shell's current context may point at
# a real cluster, and the smoke test must never run against it.
CTX="kind-${CLUSTER}"
# Use a private kubeconfig. kind writes its context into KUBECONFIG and can change the current
# context, which would leave your shell pointing at the wrong cluster.
KUBECONFIG_FILE="$(mktemp -t spotprice-smoke-kubeconfig.XXXXXX)"
export KUBECONFIG="$KUBECONFIG_FILE"
IMAGE="spotprice-exporter:smoke"
ZONE="10Y1001A1001A46L"
TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-240}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

kc() { kubectl --context "$CTX" "$@"; }
hc() { helm --kube-context "$CTX" "$@"; }

log() { printf '\033[1;34m==> %s\033[0m\n' "$*"; }
fail() { printf '\033[1;31mFAIL: %s\033[0m\n' "$*" >&2; exit 1; }

for tool in docker kind kubectl helm; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is required"
done

cleanup() {
  if [[ "${KEEP_CLUSTER:-0}" != "1" ]]; then
    log "deleting kind cluster $CLUSTER"
    kind delete cluster --name "$CLUSTER" >/dev/null 2>&1 || true
    rm -f "$KUBECONFIG_FILE"
  else
    log "keeping cluster $CLUSTER; kubeconfig at $KUBECONFIG_FILE (use: KUBECONFIG=$KUBECONFIG_FILE kubectl --context $CTX ...)"
  fi
}
trap cleanup EXIT

log "creating kind cluster $CLUSTER"
kind delete cluster --name "$CLUSTER" >/dev/null 2>&1 || true
kind create cluster --name "$CLUSTER" --wait 120s

log "building image $IMAGE"
docker buildx build --load -t "$IMAGE" "$ROOT" >/dev/null
kind load docker-image "$IMAGE" --name "$CLUSTER"

log "preloading the collector image into the kind node (avoids slow in-cluster pulls)"
COLLECTOR_IMAGE="otel/opentelemetry-collector-contrib:0.165.0"
docker image inspect "$COLLECTOR_IMAGE" >/dev/null 2>&1 || docker pull -q "$COLLECTOR_IMAGE" >/dev/null
kind load docker-image "$COLLECTOR_IMAGE" --name "$CLUSTER" >/dev/null

log "deploying mock ENTSO-E and OpenTelemetry Collector"
kc create namespace observability --dry-run=client -o yaml | kc apply -f - >/dev/null
kc -n observability create configmap mock-entsoe-script \
  --from-file=mock_entsoe.py="$ROOT/hack/kind-smoke/mock_entsoe.py" \
  --dry-run=client -o yaml | kc apply -f - >/dev/null
kc apply -f "$ROOT/hack/kind-smoke/infra.yaml" >/dev/null
kc -n observability rollout status deploy/otel-collector --timeout=300s
kc -n observability rollout status deploy/mock-entsoe --timeout=300s

log "installing the chart"
kc create namespace spotprice --dry-run=client -o yaml | kc apply -f - >/dev/null
kc -n spotprice create secret generic entsoe-token \
  --from-literal=token=smoke-token --dry-run=client -o yaml | kc apply -f - >/dev/null
hc upgrade --install spotprice "$ROOT/charts/spotprice-exporter" --namespace spotprice \
  --set image.repository=spotprice-exporter --set image.tag=smoke --set image.pullPolicy=Never \
  --set zones="$ZONE" \
  --set entsoe.existingSecret=entsoe-token \
  -f "$ROOT/hack/kind-smoke/values.yaml" \
  --set entsoe.baseUrl="http://mock-entsoe.observability:8080/api" \
  --set otlp.endpoint="http://otel-collector.observability:4318" \
  --wait --timeout 240s >/dev/null
kc -n spotprice rollout status deploy/spotprice-spotprice-exporter --timeout=180s

log "waiting for price metrics at the collector (up to ${TIMEOUT_SECONDS}s)"
deadline=$((SECONDS + TIMEOUT_SECONDS))
while (( SECONDS < deadline )); do
  logs="$(kc -n observability logs deploy/otel-collector 2>/dev/null || true)"
  if grep -q "Name: energy.market.price" <<<"$logs" \
     && grep -q "energy.market.fetch.attempts" <<<"$logs"; then
    break
  fi
  sleep 10
done

logs="$(kc -n observability logs deploy/otel-collector)"
for metric in energy.market.price energy.market.price.next energy.market.data.age \
              energy.market.data.horizon energy.market.fetch.attempts; do
  grep -q "Name: $metric" <<<"$logs" || fail "metric $metric not seen at the collector"
done
grep -q "energy.market.bidding_zone: Str($ZONE)" <<<"$logs" \
  || fail "bidding zone attribute $ZONE not seen"
grep -q "Unit: EUR/MWh" <<<"$logs" || fail "unit EUR/MWh not seen"

log "PASS: all energy.market metrics reached the collector with the expected zone and unit"
