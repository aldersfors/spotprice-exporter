//! OpenTelemetry instruments. Gauges are observable: the SDK calls back at export time and reads
//! the cache, so the export cadence is independent of the fetch cadence.
//!
//! Metric names and attributes are a custom contract, documented in ADR-004.

use std::sync::{Arc, PoisonError, RwLock};

use jiff::{SignedDuration, Timestamp};
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter, ObservableGauge},
};

use crate::{derived, state::MarketState, zones};

/// Attribute: bidding zone code, for example `SE3`.
pub const ATTR_ZONE: &str = "energy.market.bidding_zone";
/// Attribute: market type. Only `day_ahead` in v1.
pub const ATTR_TYPE: &str = "energy.market.type";
/// Attribute: market time unit, for example `PT15M`.
pub const ATTR_RESOLUTION: &str = "energy.market.resolution";
/// Attribute: data source. `entsoe` in v1.
pub const ATTR_SOURCE: &str = "energy.market.source";
/// Attribute: outcome of one fetch attempt.
pub const ATTR_FETCH_RESULT: &str = "energy.market.fetch.result";

/// Counter of fetch attempts per zone and outcome. Use it to alert on sustained failures.
#[derive(Debug, Clone)]
pub struct FetchCounter(Counter<u64>);

impl FetchCounter {
    /// Create the counter on `meter`.
    #[must_use]
    pub fn new(meter: &Meter) -> Self {
        Self(
            meter
                .u64_counter("energy.market.fetch.attempts")
                .with_unit("{attempt}")
                .with_description("Price fetch attempts by zone and outcome")
                .build(),
        )
    }

    /// Record one attempt with its outcome label.
    pub fn record(&self, zone: &str, result: &'static str) {
        let mut attributes = base_attributes(zone);
        attributes.push(KeyValue::new(ATTR_FETCH_RESULT, result));
        self.0.add(1, &attributes);
    }
}

/// Keeps the observable gauges alive. Drop it to unregister them.
#[derive(Debug)]
pub struct MarketMetrics {
    _gauges: Vec<ObservableGauge<f64>>,
}

/// Register all market gauges on `meter`, reading from `state`.
pub fn register(
    meter: &Meter,
    state: &Arc<RwLock<MarketState>>,
    cheapest_window: SignedDuration,
) -> MarketMetrics {
    let gauges = vec![
        observe(
            meter,
            state,
            "energy.market.price",
            "EUR/MWh",
            "Day-ahead price for the interval containing now",
            |cache, zone, now| {
                let point = cache.current(zone, now)?;
                Some((point.eur_per_mwh, price_attributes(cache, zone)?))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.price.next",
            "EUR/MWh",
            "Day-ahead price for the interval after the one containing now",
            |cache, zone, now| {
                let point = cache.next(zone, now)?;
                Some((point.eur_per_mwh, price_attributes(cache, zone)?))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.data.age",
            "s",
            "Seconds since the last successful price fetch for the zone",
            |cache, zone, now| {
                let age = cache.data_age(zone, now)?;
                Some((seconds(age), base_attributes(zone)))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.data.horizon",
            "s",
            "Seconds from now until the cached price data ends",
            |cache, zone, now| {
                let horizon = cache.horizon(zone, now)?;
                Some((seconds(horizon), base_attributes(zone)))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.price.day.min",
            "EUR/MWh",
            "Lowest price in the zone's local calendar day, once the cache covers the whole day",
            |cache, zone, now| {
                let day = local_day_stats(cache, zone, now)?;
                Some((day.min, price_attributes(cache, zone)?))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.price.day.max",
            "EUR/MWh",
            "Highest price in the zone's local calendar day, once the cache covers the whole day",
            |cache, zone, now| {
                let day = local_day_stats(cache, zone, now)?;
                Some((day.max, price_attributes(cache, zone)?))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.price.day.avg",
            "EUR/MWh",
            "Mean price in the zone's local calendar day, once the cache covers the whole day",
            |cache, zone, now| {
                let day = local_day_stats(cache, zone, now)?;
                Some((day.avg, price_attributes(cache, zone)?))
            },
        ),
        observe(
            meter,
            state,
            "energy.market.price.cheapest_window.start",
            "s",
            "Start of the cheapest run of intervals of the configured length, from now on",
            move |cache, zone, now| {
                let series = cache.series(zone)?;
                let start = derived::cheapest_window(series, now, cheapest_window)?;
                let mut attributes = base_attributes(zone);
                attributes.push(KeyValue::new(ATTR_WINDOW, iso_hours(cheapest_window)));
                #[allow(clippy::cast_precision_loss)]
                Some((start.as_second() as f64, attributes))
            },
        ),
    ];
    MarketMetrics { _gauges: gauges }
}

fn observe<F>(
    meter: &Meter,
    state: &Arc<RwLock<MarketState>>,
    name: &'static str,
    unit: &'static str,
    description: &'static str,
    value: F,
) -> ObservableGauge<f64>
where
    F: Fn(&MarketState, &str, Timestamp) -> Option<(f64, Vec<KeyValue>)> + Send + Sync + 'static,
{
    let state = Arc::clone(state);
    meter
        .f64_observable_gauge(name)
        .with_unit(unit)
        .with_description(description)
        .with_callback(move |instrument| {
            let now = Timestamp::now();
            let cache = state.read().unwrap_or_else(PoisonError::into_inner);
            for zone in cache.zone_codes() {
                if let Some((reading, attributes)) = value(&cache, zone, now) {
                    instrument.observe(reading, &attributes);
                }
            }
        })
        .build()
}

/// Attribute: length of a cheapest-window gauge, as an ISO 8601 duration, for example `PT3H`.
pub const ATTR_WINDOW: &str = "energy.market.window";

fn local_day_stats(cache: &MarketState, zone: &str, now: Timestamp) -> Option<derived::DayStats> {
    let timezone = jiff::tz::TimeZone::get(zones::lookup(zone)?.timezone).ok()?;
    let (start, end) = derived::local_day(now, &timezone)?;
    derived::day_stats(cache.series(zone)?, start, end)
}

fn iso_hours(length: SignedDuration) -> String {
    format!("PT{}H", length.as_hours())
}

/// Attributes shared by every zone metric. The cache is keyed by EIC code, but the zone attribute
/// carries the registry name (for example `SE3`), as ADR-004 specifies.
fn base_attributes(zone: &str) -> Vec<KeyValue> {
    let label = crate::zones::lookup(zone).map_or(zone, |entry| entry.name);
    vec![
        KeyValue::new(ATTR_ZONE, label.to_owned()),
        KeyValue::new(ATTR_TYPE, "day_ahead"),
        KeyValue::new(ATTR_SOURCE, "entsoe"),
    ]
}

fn price_attributes(cache: &MarketState, zone: &str) -> Option<Vec<KeyValue>> {
    let resolution = cache.resolution(zone)?;
    let mut attributes = base_attributes(zone);
    attributes.push(KeyValue::new(ATTR_RESOLUTION, resolution.iso8601()));
    Some(attributes)
}

#[allow(clippy::cast_precision_loss)]
fn seconds(delta: SignedDuration) -> f64 {
    delta.as_secs_f64()
}

#[cfg(test)]
mod zone_label_tests {
    use super::*;

    #[test]
    fn zone_attribute_uses_the_registry_name_for_an_eic_code() {
        let attributes = base_attributes("10Y1001A1001A46L");
        assert_eq!(attributes[0].key.as_str(), ATTR_ZONE);
        assert_eq!(attributes[0].value.to_string(), "SE3");
    }

    #[test]
    fn zone_attribute_falls_back_to_the_input_for_unknown_zones() {
        let attributes = base_attributes("UNKNOWN");
        assert_eq!(attributes[0].value.to_string(), "UNKNOWN");
    }
}
