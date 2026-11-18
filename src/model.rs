//! Normalised price data. Providers produce a [`PriceSeries`], and the rest of the system only
//! reads this type.
//!
//! Resolution is data: a series is either 15-minute (`PT15M`, since the SDAC go-live for delivery
//! day 2025-10-01) or hourly (`PT60M`, older history). Points are kept in UTC and must sit on the
//! resolution grid. Gaps are allowed and are represented by missing points, never filled.

use jiff::{SignedDuration, Timestamp};
use thiserror::Error;

/// Market time unit of a price series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// 15-minute market time unit (`PT15M`).
    Minutes15,
    /// Hourly market time unit (`PT60M`).
    Hour,
}

impl Resolution {
    /// Length of one interval.
    #[must_use]
    pub const fn duration(self) -> SignedDuration {
        match self {
            Self::Minutes15 => SignedDuration::from_mins(15),
            Self::Hour => SignedDuration::from_hours(1),
        }
    }

    /// ISO 8601 duration as used in ENTSO-E documents and metric attributes.
    #[must_use]
    pub const fn iso8601(self) -> &'static str {
        match self {
            Self::Minutes15 => "PT15M",
            Self::Hour => "PT60M",
        }
    }

    /// Parse an ISO 8601 resolution string. Only the two market time units are accepted.
    #[must_use]
    pub fn from_iso8601(value: &str) -> Option<Self> {
        match value {
            "PT15M" => Some(Self::Minutes15),
            "PT60M" => Some(Self::Hour),
            _ => None,
        }
    }
}

/// One priced interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PricePoint {
    /// Start of the interval, UTC.
    pub start: Timestamp,
    /// Day-ahead price in EUR/MWh.
    pub eur_per_mwh: f64,
}

/// Errors raised when a series violates its invariants.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SeriesError {
    /// A price is not a finite number.
    #[error("price at {0} is not a finite number")]
    NonFinitePrice(Timestamp),
    /// A point does not start on the resolution grid.
    #[error("point at {0} is not aligned to the {1} grid")]
    Misaligned(Timestamp, &'static str),
    /// Points are not strictly increasing, so there is a duplicate or out-of-order point.
    #[error("point at {0} is duplicated or out of order")]
    NotIncreasing(Timestamp),
}

/// Day-ahead prices for one bidding zone.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceSeries {
    zone: String,
    resolution: Resolution,
    points: Vec<PricePoint>,
}

impl PriceSeries {
    /// Build a validated series. Points must be strictly increasing, aligned to the resolution
    /// grid (on the epoch), and have finite prices.
    ///
    /// # Errors
    ///
    /// Returns [`SeriesError`] when an invariant is violated.
    pub fn new(
        zone: impl Into<String>,
        resolution: Resolution,
        points: Vec<PricePoint>,
    ) -> Result<Self, SeriesError> {
        let step = resolution.duration().as_secs();
        let mut previous: Option<Timestamp> = None;
        for point in &points {
            if !point.eur_per_mwh.is_finite() {
                return Err(SeriesError::NonFinitePrice(point.start));
            }
            if point.start.as_second().rem_euclid(step) != 0 {
                return Err(SeriesError::Misaligned(point.start, resolution.iso8601()));
            }
            if previous.is_some_and(|prev| point.start <= prev) {
                return Err(SeriesError::NotIncreasing(point.start));
            }
            previous = Some(point.start);
        }
        Ok(Self {
            zone: zone.into(),
            resolution,
            points,
        })
    }

    /// Bidding zone code this series belongs to.
    #[must_use]
    pub fn zone(&self) -> &str {
        &self.zone
    }

    /// Market time unit of the series.
    #[must_use]
    pub const fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Points in chronological order.
    #[must_use]
    pub fn points(&self) -> &[PricePoint] {
        &self.points
    }

    /// The point whose interval contains `at`, if the series covers it.
    #[must_use]
    pub fn point_at(&self, at: Timestamp) -> Option<&PricePoint> {
        let index = self.points.partition_point(|p| p.start <= at);
        let candidate = self.points.get(index.checked_sub(1)?)?;
        (at < candidate.start + self.resolution.duration()).then_some(candidate)
    }

    /// End of the last covered interval, if any.
    #[must_use]
    pub fn covered_until(&self) -> Option<Timestamp> {
        self.points
            .last()
            .map(|p| p.start + self.resolution.duration())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn utc(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> Timestamp {
        jiff::civil::date(y as i16, mo as i8, d as i8)
            .at(h as i8, mi as i8, 0, 0)
            .to_zoned_datetime(jiff::tz::TimeZone::UTC)
            .expect("valid time")
            .timestamp()
    }

    fn point(start: Timestamp, price: f64) -> PricePoint {
        PricePoint {
            start,
            eur_per_mwh: price,
        }
    }

    #[test]
    fn point_at_finds_containing_interval_and_respects_gaps() {
        let series = PriceSeries::new(
            "SE3",
            Resolution::Minutes15,
            vec![
                point(utc(2026, 11, 17, 10, 0), 10.0),
                point(utc(2026, 11, 17, 10, 30), 20.0),
            ],
        )
        .expect("valid");

        assert_eq!(
            series
                .point_at(utc(2026, 11, 17, 10, 14))
                .map(|p| p.eur_per_mwh),
            Some(10.0)
        );
        assert_eq!(
            series.point_at(utc(2026, 11, 17, 10, 15)),
            None,
            "gap after 10:15"
        );
        assert_eq!(
            series
                .point_at(utc(2026, 11, 17, 10, 31))
                .map(|p| p.eur_per_mwh),
            Some(20.0)
        );
        assert_eq!(
            series.point_at(utc(2026, 11, 17, 9, 59)),
            None,
            "before first point"
        );
    }

    #[test]
    fn covered_until_is_end_of_last_interval() {
        let series = PriceSeries::new(
            "SE3",
            Resolution::Hour,
            vec![point(utc(2026, 11, 17, 23, 0), 5.0)],
        )
        .expect("valid");
        assert_eq!(series.covered_until(), Some(utc(2026, 11, 18, 0, 0)));
    }

    #[test]
    fn rejects_misaligned_point() {
        let err = PriceSeries::new(
            "SE3",
            Resolution::Minutes15,
            vec![point(utc(2026, 11, 17, 10, 7), 1.0)],
        )
        .expect_err("misaligned");
        assert!(matches!(err, SeriesError::Misaligned(_, "PT15M")));
    }

    #[test]
    fn rejects_duplicate_or_unordered_points() {
        let dup = PriceSeries::new(
            "SE3",
            Resolution::Hour,
            vec![
                point(utc(2026, 11, 17, 10, 0), 1.0),
                point(utc(2026, 11, 17, 10, 0), 2.0),
            ],
        );
        assert!(matches!(dup, Err(SeriesError::NotIncreasing(_))));
    }

    #[test]
    fn rejects_non_finite_price() {
        let err = PriceSeries::new(
            "SE3",
            Resolution::Hour,
            vec![point(utc(2026, 11, 17, 10, 0), f64::NAN)],
        )
        .expect_err("nan");
        assert!(matches!(err, SeriesError::NonFinitePrice(_)));
    }

    #[test]
    fn resolution_parses_only_market_time_units() {
        assert_eq!(
            Resolution::from_iso8601("PT15M"),
            Some(Resolution::Minutes15)
        );
        assert_eq!(Resolution::from_iso8601("PT60M"), Some(Resolution::Hour));
        assert_eq!(Resolution::from_iso8601("P1D"), None);
    }
}
