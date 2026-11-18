# Backlog: spotprice-exporter

Electricity spot price exporter. Pushes day-ahead prices per bidding zone as OpenTelemetry metrics (OTLP).

Each story uses the house issue format: **Background**, **Description of Problem**, **Definition of Done**.
Release labels: **R0** spike, **R1** MVP, **R2** usable, **R3** production, **R4** growth.
Priorities: **P0** blocks the release, **P1** should ship, **P2** nice to have.

Planning source: the approved plan, `~/.claude/plans/do-we-track-electicity-splendid-axolotl.md`.

## Status (2026-11-17)

| Story | Status | Notes |
| --- | --- | --- |
| S0.1 ENTSO-E token | Done | Key obtained by the owner and read from a file, never committed. Verified with real requests |
| S0.2 A44 response spike | Done, rate limit not tested | Real responses in tests/fixtures: normal day, both DST days, next day. Next-day prices first appeared at 12:08 UTC (poll every 10 min; 11:58 UTC had none), so publication is around 12:00 UTC (13:00 CET). Rate limit deliberately not probed: a test that reaches 400 requests a minute risks the token, per the helpdesk. The exporter peaks at one request per zone every 15 minutes |
| S0.3 OTel spike | Done | opentelemetry 0.33.1; gauge pushed to collector; ADR-002 |
| S0.4 ENTSO-E terms | Done (owner confirmed) | Owner confirmed the licence is acceptable, 2026-11-18. Terms not re-read by the exporter's authors. Re-check before any public image. See docs/data-sources.md |
| S0.5 ADRs | Done | ADR-002 Accepted; 001, 003, 004 Proposed for review |
| S1.1 Scaffold | Done locally | make all passes; distroless image builds and runs; CI not yet run on GitHub |
| S1.2 Config | Done | Zones resolve through the registry; unknown zones exit 2 with the known list. Token from file or env |
| S1.3 Zone registry | Done | 13 zones (SE1-4, NO1-5, DK1-2, FI, DE-LU). Every EIC code checked against the live API. Timezones recorded for S2.2. DE-LU resolved: the API returns Sequence 1 (SDAC, tagged position 1) and Sequence 2 (EXAA 10:15 auction). The parser uses Sequence 1. Confirmed against the ENTSO-E table for 18/11/2026, which is shown in UTC |
| S1.4 ENTSO-E provider | Done against synthetic fixtures | Parser and HTTP error mapping (401, 429, 5xx, 999 no-data, token redaction). DST days: 92 and 100 intervals tested. Needs S0.2 real fixtures |
| S1.5 Scheduler | Done | Pure policy tested. Hourly full refresh per zone, so gaps the source fills later reach the cache (max 24 requests per zone per day, plus fetches while coverage is short). Publish window 11:00 UTC; <=1 request per zone per 15 min |
| S1.6 Gauges over OTLP | Done, E2E verified | price, .next, data.age, data.horizon arrive at a collector with zone/type/source/resolution attributes; values match mock. Found and fixed: meter taken from global provider before it was set (no-op) |
| S2.1 Staleness and horizon | Done | data.age and data.horizon exported |
| S2.2 Derived metrics | Done | Local-day min, max and avg (zone timezone, DST-aware, only when the cache covers the whole day) and cheapest contiguous window (`SPOTPRICE_CHEAPEST_WINDOW_HOURS`, default 3). Timezone database bundled with jiff (tzdb-bundle-always) so the distroless image needs no system zoneinfo. Verified end to end on live SE3 data |
| S2.3 Health and readiness | Done | /healthz liveness; /ready after first successful fetch |
| S2.4 Logs and self-telemetry | Done | JSON logs; `energy.market.fetch.attempts` counter by zone and outcome |
| S2.5 Graceful shutdown with flush | Done, verified | SIGTERM: HTTP stops, scheduler aborts, last metrics flushed to collector | SIGTERM/ctrl-c stops HTTP, aborts scheduler, flushes meter provider |
| S3.1 Helm chart | Done, rendered and linted | charts/spotprice-exporter. Single replica, Recreate, no Service, token via file from secret. Found in smoke test: token file was unreadable (fixed with runAsUser/fsGroup 65532, mode 0440) |
| S3.2 Release pipeline | Written, not verified | release-please config and workflow added. Chart version, appVersion and values.yaml image tag bumped by release-please. Unverified: the yaml `path`/`jsonpath` extra-file keys, and the first real release (needs the GitHub repo) |
| S3.3 kind smoke test | Done, passing | hack/kind-smoke.sh on a private kubeconfig. Found and fixed: token file unreadable (uid 65532), NetworkPolicy blocked the collector (chart now refuses to render without an OTLP destination) |
| S3.4 Supply chain | Done | deny.toml; cargo-deny passes locally and in CI job |
| S3.5 Docs | Done | docs/metric-contract.md, getting-started.md, runbook.md, data-sources.md |
| S3.6 Dashboard and alerts | Done (examples, unverified against a backend) | docs/examples/grafana-dashboard.json (8 panels, valid JSON); docs/examples/prometheus-rules.yaml (4 alerts, parsed; runbook links). Metric names are Prometheus-style guesses until checked |

---

## Epic 0: Unblock and de-risk (R0)

### S0.1 Request ENTSO-E API token (P0)
**Background.** ENTSO-E requires a token for the RESTful API. Access is granted by email and can take up to 3 working days, so it is the critical-path item.
**Description of Problem.** Register on the Transparency Platform, email transparency@entsoe.eu with "RESTful API access" in the subject, then generate the token under My Account.
**Definition of Done**
- [ ] Token issued
- [ ] Token stored outside the repo (password manager / OpenBao), never committed
- [ ] One manual A44 request for SE3 succeeds

### S0.2 Spike: ENTSO-E A44 response shape (P0)
**Background.** The 15-minute MTU went live for delivery day 2025-10-01. Parsing must not assume 24 points per day.
**Description of Problem.** Capture real responses to shape the parser and test fixtures. Publication time and rate limits are unverified.
**Definition of Done**
- [ ] Fixtures committed under `tests/fixtures/`: PT15M day, PT60M history, a DST-change day, a not-yet-published next day
- [ ] Observed publication time for next-day prices recorded in `docs/data-sources.md`
- [ ] Observed rate-limit behaviour recorded

### S0.3 Spike: Rust OpenTelemetry metrics over OTLP (P0)
**Background.** The OTel Rust metrics SDK is stable from 0.30. The latest release seen was 0.32.0. The OTLP crate stability and the observable-gauge API are unverified.
**Description of Problem.** Confirm exact crate versions and the gauge API, and whether http/protobuf with flush-on-shutdown works.
**Definition of Done**
- [ ] Minimal binary pushes one observable gauge to a local OTel Collector (http/protobuf)
- [ ] Exact crate versions and API recorded in ADR-002
- [ ] Flush on SIGTERM verified

### S0.4 Confirm ENTSO-E reuse and attribution terms (P1)
**Background.** A public image and a published dataset may carry licence and attribution obligations.
**Description of Problem.** Read the ENTSO-E terms and record what applies.
**Definition of Done**
- [ ] Terms summarised in `docs/data-sources.md`
- [ ] Anything unclear flagged "VERIFY WITH LEGAL COUNSEL"

### S0.5 Architecture decision records (P1)
**Definition of Done**
- [ ] ADR-001 provider abstraction
- [ ] ADR-002 OTLP push only (no scrape endpoint)
- [ ] ADR-003 canonical unit EUR/MWh
- [ ] ADR-004 metric contract

---

## Epic 1: Correct price data (R1 MVP)

### S1.1 Repo scaffold from nibe-exporter conventions (P0)
**Background.** `nibe-exporter` holds the house conventions for Rust, CI, Docker, release-please and Helm.
**Description of Problem.** Scaffold the crate and pipelines. Do not copy these known defects from nibe-exporter:
- `rustfmt.toml` says edition 2021 while Cargo says 2024
- `make docker-build` calls `docker buildx bake dev` with no bake file
- release-please `extra-files` for Chart.yaml needs `jsonpath` for `$.version` and `$.appVersion`
- CI uses `@stable` instead of the pinned `rust-toolchain.toml`
**Definition of Done**
- [ ] CI green on an empty crate (check, test, clippy `-D warnings`, fmt)
- [ ] Distroless image builds (`gcr.io/distroless/cc-debian13:nonroot`)
- [ ] release-please configured
- [ ] `renovate.json5` committed (see S5.2)

### S1.2 Config: zones, token, OTLP endpoint (P0)
**Definition of Done**
- [ ] `SPOTPRICE_ZONES`, `SPOTPRICE_ENTSOE_TOKEN` and `SPOTPRICE_ENTSOE_TOKEN_FILE` supported
- [ ] Validation errors name the bad field
- [ ] Unknown zone rejected at startup

### S1.3 Zone registry (P0)
**Definition of Done**
- [ ] SE1-SE4, NO1-NO5, DK1-DK2, FI, DE-LU present with EIC code and timezone
- [ ] EIC codes taken from the ENTSO-E area list (not guessed)
- [ ] Adding a zone is a data-only change
- [ ] Test parses every registry entry

### S1.4 ENTSO-E day-ahead provider (P0)
**Definition of Done**
- [ ] Fetches and parses A44 for a zone and day into `PriceSeries`
- [ ] Handles PT15M and PT60M
- [ ] DST days yield 92/100 points (15-minute) or 23/25 (hourly)
- [ ] Missing points are explicit, never silently filled
- [ ] wiremock tests use S0.2 fixtures

### S1.5 Refresh scheduler (P0)
**Definition of Done**
- [ ] Fetches today and tomorrow
- [ ] Polls with backoff after the publication window
- [ ] Documented request budget, never exceeded
- [ ] Keeps last good data on failure

### S1.6 Price gauges over OTLP (P0)
**Definition of Done**
- [ ] `energy.market.price` and `energy.market.price.next` exported per zone
- [ ] Snapshot test of the metric contract
- [ ] End-to-end verified against a local Collector

---

## Epic 2: Trustworthy operation (R2 usable)

### S2.1 Staleness and horizon metrics (P1)
**Definition of Done**
- [ ] `energy.market.data.age` and `energy.market.data.horizon` exported
- [ ] Example alert rules in docs

### S2.2 Derived metrics (P1)
**Definition of Done**
- [ ] `price.day.min`, `.max`, `.avg` computed in the zone's local day
- [ ] `price.cheapest_window.start` with configurable window length
- [ ] Unit tests include DST days

### S2.3 Health and readiness endpoints (P1)
**Definition of Done**
- [ ] `/ready` is false until the first successful fetch
- [ ] `/healthz` is independent of provider state

### S2.4 Structured logs and self-telemetry (P2)
**Definition of Done**
- [ ] Fetch outcomes logged with a stable schema
- [ ] Export errors surfaced, not swallowed

### S2.5 Graceful shutdown with flush (P1)
**Definition of Done**
- [ ] SIGTERM flushes pending metrics before exit

---

## Epic 3: Deployable (R3 production)

### S3.1 Helm chart (P1)
**Definition of Done**
- [ ] Deployment, ServiceAccount, NetworkPolicy and CiliumNetworkPolicy
- [ ] Egress only to ENTSO-E and the OTLP endpoint
- [ ] No Service or ServiceMonitor
- [ ] Values for OTLP endpoint, headers and zones; secret reference for the token
- [ ] `helm lint` in CI

### S3.2 Release pipeline (P1)
**Definition of Done**
- [ ] release-please, multi-arch image, keyless cosign, Helm OCI push
- [ ] Verified on a real release

### S3.3 kind smoke test with a Collector (P1)
**Definition of Done**
- [ ] `hack/kind-smoke.sh` deploys the chart and an OTel Collector
- [ ] Script asserts the price metric arrives

### S3.4 Supply chain checks (P1)
**Definition of Done**
- [ ] `cargo-deny` (or `cargo-audit`) in CI
- [ ] Renovate active on the repo

### S3.5 Documentation (P1)
**Definition of Done**
- [ ] `README.md`, `docs/getting-started.md`, `docs/metric-contract.md`, `docs/data-sources.md`
- [ ] Runbook for "no data" and "stale data"

### S3.6 Example dashboard and alerts (P2)
**Definition of Done**
- [ ] Grafana dashboard JSON
- [ ] Example alert rules
- [ ] Cost-join example: `nibe_total_power_consumption_watts * price`

---

## Epic 4: Growth (R4, ordered by demand)

- **S4.1 Second provider (P2).** Token-free elprisetjustnu.se (SE1-SE4) or energi-data-service (DK). DoD: implements `Provider`, selectable per zone, shared contract tests.
- **S4.2 Currency conversion (P2).** SEK, NOK, DKK output with a documented rate source. DoD: rate source and its terms verified first.
- **S4.3 Local-currency per-kWh and öre view (P2).** Depends on S4.2.
- **S4.4 gRPC OTLP feature flag (P2).**
- **S4.5 Price-level classification (P2).** Cheap, normal, expensive relative to a rolling window.
- **S4.6 Historical backfill command (P2).** One-shot job to push past days.

---

## Epic 5: Platform onboarding (Renovate and GitOps)

Lives in `~/projects/jarsater/k8s-home-lab`. Two parts: Renovate scans the new repo, and the deployed workload is kept current in the cluster.

### S5.1 Add the repo to the Renovate job (P0 for R1)
**Background.** The RenovateJob autodiscovers only repos in `spec.discoveryFilters`.
**Description of Problem.** Add `jalet/spotprice-exporter` to `charts/vendor/renovate-operator/manifests/renovatejob.yaml`. Also update the "Four repos in scope" comment. The repo must be added to the GitHub App installation (a GitHub settings change, manual).
**Definition of Done**
- [ ] `discoveryFilters` includes `jalet/spotprice-exporter`
- [ ] GitHub App installation includes the repo (manual)
- [ ] First hourly run discovers the repo and opens an onboarding PR or Dependency Dashboard issue
- [ ] No Hubble drops for the new repo's Renovate egress (re-check after first run)

### S5.2 Repo-side Renovate prerequisites (P0 for R1)
**Definition of Done**
- [ ] `renovate.json5` committed (Renovate ignores `renovate.jsonc`)
- [ ] `platformAutomerge` decided to match repo visibility and plan

### S5.3 Fix nibe-exporter's Renovate config (P2, optional, separate PR)
**Background.** `nibe-exporter/renovate.jsonc` uses an extension Renovate does not read, so its config is silently ignored.
**Definition of Done**
- [ ] Renamed to `renovate.json5`, after the owner agrees

### S5.4 GitOps wrapper chart for the app (P1, R3)
**Description of Problem.** Add `charts/apps/spotprice-exporter/` mirroring `charts/apps/nibe-exporter/`: `Chart.yaml`, `source.yaml`, `values.yaml`, `manifests/`. Renovate tracks the chart and image through the existing `helmv3` and `helm-values` managers.
**Definition of Done**
- [ ] ArgoCD syncs the app
- [ ] A Renovate PR for a chart or image bump is observed

### S5.5 Token secret via OpenBao and ESO (P1, R3)
**Description of Problem.** Mirror nibe-exporter: namespace store, OpenBao policy and role in `pulumi/pkgs/openbao/openbao.go`, and `openbaoNamespaceRoles` in `Pulumi.prod.yaml`.
**Definition of Done**
- [ ] Token reaches the pod via ExternalSecret, never in git

### S5.6 Decide the in-cluster OTLP destination (P1, R3)
**Definition of Done**
- [ ] Confirmed which OTLP endpoint (Alloy or Collector) accepts metrics
- [ ] Egress rule added to the app's NetworkPolicy

### S5.7 Kargo promotion (P2, optional)
**Background.** Epic 015 in the homelab backlog lists five criteria for Kargo promotion.
**Definition of Done**
- [ ] Decided: Renovate only first; revisit once `energy.market.data.age` exists as a health query

---

## Epic 6: Maintenance

### S6.1 Migrate chrono to jiff (P1), done
**Background.** RUSTSEC-2026-0543 marks chrono as unmaintained, with no safe upgrade inside chrono. Resolved by this migration.
**Description of Problem.** Replaced chrono with jiff 1.0.1 in the model, state, scheduler, metrics and provider code. The change touches most time handling, so it needs its own PR with the existing tests as the safety net.
**Definition of Done**
- [x] No chrono in `Cargo.lock`
- [x] The `RUSTSEC-2026-0543` ignore removed from `deny.toml` (cargo-deny passes)
- [x] All tests pass, including the DST and real-fixture tests (40 tests); real API run still fetches 95 points for SE3

## Ordering

1. S0.1 (token, calendar-time blocker) in parallel with S0.2 and S0.3
2. S1.1 scaffold, then S5.1 and S5.2 so Renovate starts early
3. S1.2 to S1.6
4. Epic 2
5. Epic 3 (S3.2 first release unblocks S5.4 to S5.6)
6. Epic 4 by demand

## Open questions

- Default cheapest-window length
- Gauges for current and next price, versus a per-interval series
- Whether to ship a Collector config example
- Whether the repo is public (affects ENTSO-E terms, S0.4)
