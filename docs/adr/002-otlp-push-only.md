# ADR-002: OTLP push only, no Prometheus scrape endpoint

**Status:** Accepted (verified by spike S0.3, 2026-11-17)
**Date:** 2026-11-17

## Context

The sibling project `nibe-exporter` exposes `/metrics` for Prometheus scrape. This project sends
metrics to an OpenTelemetry Collector over OTLP instead. The requirement is OTel, not Prometheus
scrape jobs.

## Decision

Export metrics by OTLP/HTTP (http/protobuf) using the OpenTelemetry Rust SDK. Do not expose a
`/metrics` endpoint. Keep `/healthz` and `/ready` for Kubernetes probes only.

Verified versions (crates.io, checked 2026-11-17):

- `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`: **0.33.1**. The research pass
  reported 0.32.0, but 0.33.1 is the latest release.
- Features used: `opentelemetry` (metrics), `opentelemetry_sdk` (metrics, rt-tokio),
  `opentelemetry-otlp` (metrics, http-proto, reqwest-blocking-client). gRPC is not used.

Spike result: a `f64_observable_gauge` with unit `EUR/MWh` and a `energy.market.bidding_zone`
attribute reached an OpenTelemetry Collector through `PeriodicReader`. `force_flush` and
`shutdown` both returned cleanly. The collector was `otel/opentelemetry-collector-contrib`
(latest, 0.165.0 at the time of the spike) with a debug exporter.

## Alternatives considered

- **gRPC OTLP:** pulls in tonic and is heavier. Deferred to a feature flag (backlog S4.4).
- **Prometheus exporter plus scrape:** conflicts with the requirement.

## Consequences

- Delivery depends on the Collector being reachable. Metrics are lost if the periodic reader
  cannot push. Staleness is visible through `energy.market.data.age` (backlog S2.1).
- The collector image tag `0.116.0` would not start on the development host (arm64 Docker VM),
  and `latest` worked. Pin a known-good tag in the kind smoke test (S3.3).
- Flush on SIGTERM must be wired into shutdown (backlog S2.5).
