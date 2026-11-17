# ADR-001: Provider abstraction for price sources

**Status:** Proposed
**Date:** 2026-11-17

## Context

Price data comes from different sources with different coverage, auth and rate limits. ENTSO-E
covers all target bidding zones but needs a token. elprisetjustnu.se covers SE1-SE4 without auth.
energi-data-service covers Denmark. The project must be configurable by region, and new sources
should not force changes to the metrics code.

## Decision

Define a `Provider` trait with one core operation, fetch day-ahead prices for a zone and a day,
returning a normalised `PriceSeries`. ENTSO-E is the first implementation (v1). Other providers
are added later (backlog S4.1).

Each zone is served by one configured provider. Providers are not mixed within a zone.

## Alternatives considered

- **Hard-code ENTSO-E:** simplest, but it blocks the token-free Swedish option and couples the
  metrics code to one API.
- **Generic HTTP scraper with per-source config:** flexible, but puts source-specific parsing
  into config. Not worth it for one or two sources.

## Consequences

- The `PriceSeries` type is the contract between providers and the rest of the system. It holds
  UTC intervals, the resolution, and the price in EUR/MWh.
- Contract tests run against every provider with shared fixtures.
