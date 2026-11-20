# Metric contract

Custom metrics for electricity market prices. OpenTelemetry has no semantic convention for this,
so names follow the `energy.market.*` namespace. The decision is recorded in
[ADR-004](adr/004-metric-contract.md). Names and attributes are stable from the first release.
A change is a breaking change.

Metrics are pushed over OTLP/HTTP (see [ADR-002](adr/002-otlp-push-only.md)). There is no scrape endpoint.

## Gauges

Observable gauges. Values are read from the cache when the SDK exports, not when a fetch happens.

| Metric | Unit | Meaning |
| --- | --- | --- |
| `energy.market.price` | `EUR/MWh` | Day-ahead price for the interval containing now. Absent when no price covers now. |
| `energy.market.price.next` | `EUR/MWh` | Price for the interval after the one containing now. Absent at the end of cached coverage. |
| `energy.market.data.age` | `s` | Seconds since the last successful fetch for the zone. Absent until the first success. |
| `energy.market.data.horizon` | `s` | Seconds from now until cached coverage ends. Negative when the cache has run out. |
| `energy.market.price.day.min` | `EUR/MWh` | Lowest price in the zone's local calendar day. Absent until the cache covers the whole day. |
| `energy.market.price.day.max` | `EUR/MWh` | Highest price in the zone's local calendar day. Same rule as `day.min`. |
| `energy.market.price.day.avg` | `EUR/MWh` | Mean price in the zone's local calendar day, equal weight per interval. Same rule as `day.min`. |
| `energy.market.price.cheapest_window.start` | `s` | Unix time of the start of the cheapest run of intervals from now on. The run length is set by `SPOTPRICE_CHEAPEST_WINDOW_HOURS` (default 3) and is carried in the `energy.market.window` attribute. Runs never span a gap in the source data. |

## Counter

| Metric | Unit | Meaning |
| --- | --- | --- |
| `energy.market.fetch.attempts` | `{attempt}` | Fetch attempts per zone and outcome. |

## Attributes

| Attribute | Values | Used by |
| --- | --- | --- |
| `energy.market.bidding_zone` | EIC bidding zone code, for example `10Y1001A1001A46L` (SE3) | all |
| `energy.market.type` | `day_ahead` | all |
| `energy.market.source` | `entsoe` | all |
| `energy.market.resolution` | `PT15M`, `PT60M` | `energy.market.price`, `energy.market.price.next`, `energy.market.price.day.*`, `energy.market.price.cheapest_window.start` |
| `energy.market.window` | ISO 8601 duration, for example `PT3H` | `energy.market.price.cheapest_window.start` |
| `energy.market.fetch.result` | `success`, `no_data`, `unauthorized`, `rate_limited`, `transient`, `malformed` | `energy.market.fetch.attempts` |

Timestamps never go in attributes. The zone attribute is the only high-cardinality dimension, and
it is bounded by the configured zones.

## Resource attributes

| Attribute | Value |
| --- | --- |
| `service.name` | `spotprice-exporter` |
| `service.version` | crate version, for example `0.1.0` |

## Units

Prices are in EUR/MWh, the unit ENTSO-E publishes (see [ADR-003](adr/003-canonical-unit-eur-mwh.md)).
Conversion to öre/kWh or another currency is not in v1. Divide by 1000 for EUR/kWh.

## Example queries

Current price in SE3, with the zone attribute as a label:

    energy_market_price{energy_market_bidding_zone="10Y1001A1001A46L"}

Alert when no data has arrived for two hours:

    energy_market_data_age{energy_market_bidding_zone="10Y1001A1001A46L"} > 7200

Alert on sustained authentication failures:

    sum by (energy_market_bidding_zone) (
      rate(energy_market_fetch_attempts{energy_market_fetch_result="unauthorized"}[15m])
    ) > 0

Note: the queries above use the Prometheus-style names that Mimir stores for the OTLP names (dots
become underscores). Checked against Mimir on 2026-11-19. Mimir does not add a `_total` suffix to the
fetch counter, so the series is `energy_market_fetch_attempts`. Other backends may differ, so check
the names there first. The OTLP names are the source of truth.
