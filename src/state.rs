//! In-memory cache of price data, shared by the scheduler, metrics and health endpoints.
//!
//! The scheduler writes, while metric callbacks and HTTP handlers only read. A failed fetch never
//! clears data: the last good series stays, and its age is exported so staleness stays visible.

use std::collections::BTreeMap;

use jiff::{SignedDuration, Timestamp};

use crate::model::{PricePoint, PriceSeries};

#[derive(Debug, Default, Clone)]
struct ZoneState {
    series: Option<PriceSeries>,
    last_success: Option<Timestamp>,
}

/// Price cache keyed by bidding zone.
#[derive(Debug, Default, Clone)]
pub struct MarketState {
    zones: BTreeMap<String, ZoneState>,
}

impl MarketState {
    /// Create an empty cache for the configured zones.
    pub fn new<I, S>(zones: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let zones = zones
            .into_iter()
            .map(|z| (z.into(), ZoneState::default()))
            .collect();
        Self { zones }
    }

    /// Configured zone codes, in sorted order.
    pub fn zone_codes(&self) -> impl Iterator<Item = &str> {
        self.zones.keys().map(String::as_str)
    }

    /// Merge a fresh series into the cache and record the success time.
    ///
    /// Points at or after the first new start are replaced by the new data. Earlier points are
    /// kept. If the resolution changed, the old series is replaced entirely.
    pub fn record_success(&mut self, fetched_at: Timestamp, incoming: PriceSeries) {
        let zone = self.zones.entry(incoming.zone().to_owned()).or_default();
        let existing = zone.series.take();
        zone.series = match (existing, incoming.points().first().map(|p| p.start)) {
            // An empty response must never erase cached prices.
            (Some(old), None) => Some(old),
            (Some(old), Some(first_new)) if old.resolution() == incoming.resolution() => {
                let mut points: Vec<PricePoint> = old
                    .points()
                    .iter()
                    .copied()
                    .filter(|p| p.start < first_new)
                    .collect();
                points.extend_from_slice(incoming.points());
                PriceSeries::new(incoming.zone(), incoming.resolution(), points)
                    .ok()
                    .or(Some(old))
            }
            _ => Some(incoming),
        };
        zone.last_success = Some(fetched_at);
    }

    /// The point whose interval contains `now`.
    #[must_use]
    pub fn current(&self, zone: &str, now: Timestamp) -> Option<PricePoint> {
        self.zones
            .get(zone)?
            .series
            .as_ref()?
            .point_at(now)
            .copied()
    }

    /// The point for the interval after the one containing `now`.
    #[must_use]
    pub fn next(&self, zone: &str, now: Timestamp) -> Option<PricePoint> {
        let series = self.zones.get(zone)?.series.as_ref()?;
        let current = series.point_at(now)?;
        series
            .point_at(current.start + series.resolution().duration())
            .copied()
    }

    /// Resolution of the cached series for the zone, if any.
    #[must_use]
    pub fn resolution(&self, zone: &str) -> Option<crate::model::Resolution> {
        Some(self.zones.get(zone)?.series.as_ref()?.resolution())
    }

    /// Time since the last successful fetch for the zone, if any fetch has succeeded.
    #[must_use]
    pub fn data_age(&self, zone: &str, now: Timestamp) -> Option<SignedDuration> {
        self.zones.get(zone)?.last_success.map(|at| now - at)
    }

    /// Time from `now` until the cached data ends, if any data is cached. Negative when the
    /// cache has already run out.
    #[must_use]
    pub fn horizon(&self, zone: &str, now: Timestamp) -> Option<SignedDuration> {
        let covered = self.zones.get(zone)?.series.as_ref()?.covered_until()?;
        Some(covered - now)
    }

    /// End of the cached coverage for the zone, if any.
    #[must_use]
    pub fn covered_until(&self, zone: &str) -> Option<Timestamp> {
        self.zones.get(zone)?.series.as_ref()?.covered_until()
    }

    /// Whether any zone has at least one successful fetch. Used for readiness.
    #[must_use]
    pub fn has_any_data(&self) -> bool {
        self.zones.values().any(|z| z.last_success.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Resolution;

    fn utc(h: i8, m: i8) -> Timestamp {
        jiff::civil::date(2026, 11, 17)
            .at(h, m, 0, 0)
            .to_zoned_datetime(jiff::tz::TimeZone::UTC)
            .expect("valid")
            .timestamp()
    }

    fn series(res: Resolution, points: &[(Timestamp, f64)]) -> PriceSeries {
        let points = points
            .iter()
            .map(|&(start, eur_per_mwh)| PricePoint { start, eur_per_mwh })
            .collect();
        PriceSeries::new("SE3", res, points).expect("valid")
    }

    #[test]
    fn current_and_next_follow_the_clock() {
        let mut state = MarketState::new(["SE3"]);
        state.record_success(
            utc(10, 0),
            series(
                Resolution::Minutes15,
                &[(utc(10, 0), 10.0), (utc(10, 15), 20.0)],
            ),
        );
        assert_eq!(
            state.current("SE3", utc(10, 5)).map(|p| p.eur_per_mwh),
            Some(10.0)
        );
        assert_eq!(
            state.next("SE3", utc(10, 5)).map(|p| p.eur_per_mwh),
            Some(20.0)
        );
        assert_eq!(
            state.current("SE3", utc(10, 20)).map(|p| p.eur_per_mwh),
            Some(20.0)
        );
        assert_eq!(state.next("SE3", utc(10, 20)), None);
    }

    #[test]
    fn merge_replaces_overlap_and_keeps_earlier_points() {
        let mut state = MarketState::new(["SE3"]);
        state.record_success(
            utc(9, 0),
            series(Resolution::Hour, &[(utc(10, 0), 1.0), (utc(11, 0), 2.0)]),
        );
        state.record_success(
            utc(10, 30),
            series(Resolution::Hour, &[(utc(11, 0), 9.0), (utc(12, 0), 3.0)]),
        );

        assert_eq!(
            state.current("SE3", utc(10, 30)).map(|p| p.eur_per_mwh),
            Some(1.0)
        );
        assert_eq!(
            state.current("SE3", utc(11, 30)).map(|p| p.eur_per_mwh),
            Some(9.0)
        );
        assert_eq!(state.covered_until("SE3"), Some(utc(13, 0)));
    }

    #[test]
    fn failed_fetch_keeps_last_good_data_and_ages() {
        let mut state = MarketState::new(["SE3"]);
        state.record_success(utc(10, 0), series(Resolution::Hour, &[(utc(10, 0), 5.0)]));
        // No record_success call for the failed attempt: data and last_success stay as they were.
        assert_eq!(
            state.current("SE3", utc(10, 30)).map(|p| p.eur_per_mwh),
            Some(5.0)
        );
        assert_eq!(
            state.data_age("SE3", utc(10, 45)),
            Some(SignedDuration::from_mins(45))
        );
    }

    #[test]
    fn unknown_zone_and_empty_zone_report_nothing() {
        let state = MarketState::new(["SE3"]);
        assert!(!state.has_any_data());
        assert_eq!(state.current("SE3", utc(10, 0)), None);
        assert_eq!(state.data_age("SE3", utc(10, 0)), None);
        assert_eq!(state.current("NO2", utc(10, 0)), None);
    }

    #[test]
    fn resolution_change_replaces_the_series() {
        let mut state = MarketState::new(["SE3"]);
        state.record_success(utc(9, 0), series(Resolution::Hour, &[(utc(10, 0), 1.0)]));
        state.record_success(
            utc(10, 0),
            series(Resolution::Minutes15, &[(utc(10, 0), 7.0)]),
        );
        assert_eq!(
            state.current("SE3", utc(10, 5)).map(|p| p.eur_per_mwh),
            Some(7.0)
        );
        assert_eq!(state.covered_until("SE3"), Some(utc(10, 15)));
    }
}
