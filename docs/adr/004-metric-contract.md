# ADR-004: Custom metric contract for energy market prices

**Status:** Accepted (2026-11-18)
**Date:** 2026-11-17

## Context

No OpenTelemetry semantic convention covers electricity prices. The nearest conventions are
`hw.energy` and `hw.host.energy`, which describe consumption, not market prices. Dashboards and
alerts depend on the metric names and attributes, so they must be stable once released.

## Decision

Use custom names under the `energy.market.*` namespace, documented in `docs/metric-contract.md`
(to be written with S3.5):

| Metric | Type | Unit | Meaning |
| --- | --- | --- | --- |
| `energy.market.price` | gauge | EUR/MWh | Price for the interval containing now |
| `energy.market.price.next` | gauge | EUR/MWh | Price for the following interval |
| `energy.market.price.day.min` / `.max` / `.avg` | gauge | EUR/MWh | Statistics over the current local day |
| `energy.market.price.cheapest_window.start` | gauge | s | Start of the cheapest N-hour window |
| `energy.market.data.age` | gauge | s | Seconds since the last successful fetch |
| `energy.market.data.horizon` | gauge | s | How far ahead the cache covers |
| `energy.market.fetch.attempts` | counter | {attempt} | Fetch attempts per zone and `energy.market.fetch.result` |
| `energy.market.price.day.min` / `.max` / `.avg` | gauge | EUR/MWh | Local calendar day statistics, once the cache covers the whole day |
| `energy.market.price.cheapest_window.start` | gauge | s | Start of the cheapest run of intervals from now on. Run length in `energy.market.window` |

Attributes: `energy.market.bidding_zone` (for example `SE3`), `energy.market.type` (`day_ahead`),
`energy.market.resolution` (`PT15M` or `PT60M`), `energy.market.source` (`entsoe`),
`energy.market.fetch.result` (fetch outcome, on the counter only).

Timestamps never go in attributes. Resource attributes: `service.name`, `service.version`.

## Consequences

- Names are final once the first release ships. Any change is a breaking change, so the contract
  needs review before R1.
- Gauges for the current and next price are a pragmatic choice. OpenTelemetry has no clean
  "future value" signal.
