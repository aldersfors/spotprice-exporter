# Getting started

## What you need

1. An ENTSO-E Transparency Platform security token. Register, then request API access by email.
   See [data-sources.md](data-sources.md) for the steps and what is still unverified.
2. The bidding zones you want, by name (`SE1`-`SE4`, `NO1`-`NO5`, `DK1`, `DK2`, `FI`, `DE-LU`). The EIC
   code also works. The exporter rejects unknown names at startup and lists the known ones.
3. An OpenTelemetry Collector that accepts OTLP/HTTP on port 4318.

## Run locally

```sh
export SPOTPRICE_ZONES=10Y1001A1001A46L
export SPOTPRICE_ENTSOE_TOKEN=...            # or SPOTPRICE_ENTSOE_TOKEN_FILE=/path/to/token
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
cargo run --release
```

Check that it is running:

```sh
curl -s -o /dev/null -w "%{http_code}\n" localhost:8080/healthz   # 200 while running
curl -s -o /dev/null -w "%{http_code}\n" localhost:8080/ready     # 200 after the first successful fetch
```

## Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `SPOTPRICE_ZONES` | required | Comma-separated EIC bidding zone codes |
| `SPOTPRICE_ENTSOE_TOKEN` | - | ENTSO-E token. Set this or the file variant, not both |
| `SPOTPRICE_ENTSOE_TOKEN_FILE` | - | Path to a file holding the token (preferred in Kubernetes) |
| `SPOTPRICE_ENTSOE_BASE_URL` | production endpoint | Override for tests against a mock server |
| `SPOTPRICE_CHEAPEST_WINDOW_HOURS` | `3` | Length of the cheapest-window gauge, 1 to 24 hours |
| `SPOTPRICE_LISTEN` | `0.0.0.0:8080` | Health server address |
| `SPOTPRICE_LOG_LEVEL` | `info` | Used when `RUST_LOG` is not set |
| `SPOTPRICE_LOG_JSON` | `true` | JSON logs |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | SDK default | Collector base URL. Standard OpenTelemetry variable |
| `OTEL_EXPORTER_OTLP_HEADERS` | - | Extra headers for the collector. Standard OpenTelemetry variable |

Exit code 2 means the configuration is invalid. The error names the field at fault.

## Run in Kubernetes

Install the chart from `charts/spotprice-exporter`. Create the token secret first:

```sh
kubectl create secret generic entsoe-token --from-literal=token="$ENTSOE_TOKEN"
helm install spotprice charts/spotprice-exporter \
  --set zones=10Y1001A1001A46L \
  --set entsoe.existingSecret=entsoe-token \
  --set otlp.endpoint=http://otel-collector.observability:4318
```

The chart fails at render time if `zones`, `entsoe.existingSecret` or `otlp.endpoint` is missing.
It runs one replica, because two would push duplicate series. For the Cilium egress policy, see
`ciliumNetworkPolicy` in `values.yaml`.
