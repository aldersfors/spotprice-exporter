//! OpenTelemetry instruments. Gauges are observable: the SDK calls back at export time and reads
//! the cache, so the export cadence is independent of the fetch cadence.
//!
//! Metric names and attributes are a custom contract, documented in ADR-004.

use std::sync::{Arc, PoisonError, RwLock};

use chrono::{DateTime, Utc};
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter, ObservableGauge},
};

use crate::state::MarketState;

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
pub fn register(meter: &Meter, state: &Arc<RwLock<MarketState>>) -> MarketMetrics {
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
    F: Fn(&MarketState, &str, DateTime<Utc>) -> Option<(f64, Vec<KeyValue>)>
        + Send
        + Sync
        + 'static,
{
    let state = Arc::clone(state);
    meter
        .f64_observable_gauge(name)
        .with_unit(unit)
        .with_description(description)
        .with_callback(move |instrument| {
            let now = Utc::now();
            let cache = state.read().unwrap_or_else(PoisonError::into_inner);
            for zone in cache.zone_codes() {
                if let Some((reading, attributes)) = value(&cache, zone, now) {
                    instrument.observe(reading, &attributes);
                }
            }
        })
        .build()
}

fn base_attributes(zone: &str) -> Vec<KeyValue> {
    vec![
        KeyValue::new(ATTR_ZONE, zone.to_owned()),
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
fn seconds(delta: chrono::TimeDelta) -> f64 {
    delta.num_milliseconds() as f64 / 1000.0
}
