# ADR-003: Canonical price unit is EUR/MWh

**Status:** Accepted (2026-11-18)
**Date:** 2026-11-17

## Context

ENTSO-E publishes day-ahead prices in EUR/MWh. elprisetjustnu.se publishes SEK/kWh and EUR/kWh
with an exchange rate. Swedish users usually think in öre/kWh. The exporter must avoid mixing
units in one metric.

## Decision

The canonical unit is EUR/MWh, the unit the source publishes. The `unit` metadata of every price
metric is `EUR/MWh`. Conversion to a local currency or per-kWh view is a derived metric, added
later (backlog S4.2 and S4.3), and it carries its own unit and attribute.

## Consequences

- The v1 exporter needs no exchange-rate source.
- Users who want öre/kWh apply a Prometheus or dashboard transformation until S4.3 lands.
- The unit annotation follows UCUM style (`EUR/MWh`). The OTel SDK passes it through unchanged.
