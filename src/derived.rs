//! Statistics derived from a price series: local-day statistics and the cheapest window.
//!
//! Day boundaries use the zone's timezone, so a day is 23 or 25 hours on the DST changes. Windows
//! never span a gap in the source data.

use jiff::{SignedDuration, Span, Timestamp, tz::TimeZone};

use crate::model::PriceSeries;

/// Minimum, maximum and average price over an interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayStats {
    /// Lowest price in EUR/MWh.
    pub min: f64,
    /// Highest price in EUR/MWh.
    pub max: f64,
    /// Mean price in EUR/MWh, weighted equally per interval.
    pub avg: f64,
}

/// Start and end of the local calendar day containing `now`, in the given timezone.
#[must_use]
pub fn local_day(now: Timestamp, zone: &TimeZone) -> Option<(Timestamp, Timestamp)> {
    let date = now.to_zoned_datetime(zone.clone()).date();
    let next = date.checked_add(Span::new().days(1)).ok()?;
    let start = date
        .at(0, 0, 0, 0)
        .to_zoned_datetime(zone.clone())
        .ok()?
        .timestamp();
    let end = next
        .at(0, 0, 0, 0)
        .to_zoned_datetime(zone.clone())
        .ok()?
        .timestamp();
    Some((start, end))
}

/// Statistics over the points whose intervals start in `[start, end)`. Returns `None` unless the
/// cache covers the whole interval at both ends.
#[must_use]
pub fn day_stats(series: &PriceSeries, start: Timestamp, end: Timestamp) -> Option<DayStats> {
    let prices: Vec<f64> = series
        .points()
        .iter()
        .filter(|p| p.start >= start && p.start < end)
        .map(|p| p.eur_per_mwh)
        .collect();
    // Only report when the cache covers both ends of the interval. A partial day would give
    // statistics that look complete but are not.
    let first = series.points().first()?.start;
    let covered = series.covered_until()?;
    if prices.is_empty() || first > start || covered < end {
        return None;
    }
    let min = prices.iter().copied().fold(f64::INFINITY, f64::min);
    let max = prices.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    #[allow(clippy::cast_precision_loss)]
    let avg = prices.iter().sum::<f64>() / prices.len() as f64;
    Some(DayStats { min, max, avg })
}

/// Start of the cheapest window of `length` that begins at or after `from`.
///
/// A window is a run of consecutive intervals with no gaps. Among equally cheap windows the
/// earliest wins. Returns `None` when no complete window fits in the cached data.
#[must_use]
pub fn cheapest_window(
    series: &PriceSeries,
    from: Timestamp,
    length: SignedDuration,
) -> Option<Timestamp> {
    let step = series.resolution().duration();
    let count = usize::try_from(length.as_secs() / step.as_secs()).ok()?;
    if count == 0 || length.as_secs() % step.as_secs() != 0 {
        return None;
    }
    let points = series.points();
    let mut best: Option<(f64, Timestamp)> = None;
    for (index, candidate) in points.iter().enumerate() {
        if candidate.start < from || index + count > points.len() {
            continue;
        }
        let window = &points[index..index + count];
        let contiguous = window.iter().enumerate().all(|(offset, p)| {
            p.start == candidate.start + step * i32::try_from(offset).unwrap_or(0)
        });
        if !contiguous {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let average = window.iter().map(|p| p.eur_per_mwh).sum::<f64>() / count as f64;
        if best.is_none_or(|(cheapest, _)| average < cheapest) {
            best = Some((average, candidate.start));
        }
    }
    best.map(|(_, start)| start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PricePoint, Resolution};

    fn ts(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> Timestamp {
        jiff::civil::date(y as i16, mo as i8, d as i8)
            .at(h as i8, mi as i8, 0, 0)
            .to_zoned_datetime(TimeZone::UTC)
            .expect("valid")
            .timestamp()
    }

    fn zone(name: &str) -> TimeZone {
        TimeZone::get(name).expect("bundled tz database has this zone")
    }

    fn quarter_hours(start: Timestamp, prices: &[f64]) -> PriceSeries {
        let points = prices
            .iter()
            .enumerate()
            .map(|(i, &p)| PricePoint {
                start: start + SignedDuration::from_mins(15 * i as i64),
                eur_per_mwh: p,
            })
            .collect();
        PriceSeries::new("SE3", Resolution::Minutes15, points).expect("valid")
    }

    #[test]
    fn spring_forward_local_day_is_23_hours() {
        let (start, end) =
            local_day(ts(2026, 3, 29, 12, 0), &zone("Europe/Stockholm")).expect("day");
        assert_eq!(start, ts(2026, 3, 28, 23, 0));
        assert_eq!(end - start, SignedDuration::from_hours(23));
    }

    #[test]
    fn autumn_back_local_day_is_25_hours() {
        let (start, end) =
            local_day(ts(2026, 10, 25, 12, 0), &zone("Europe/Stockholm")).expect("day");
        assert_eq!(start, ts(2026, 10, 24, 22, 0));
        assert_eq!(end - start, SignedDuration::from_hours(25));
    }

    #[test]
    fn ordinary_local_day_is_24_hours_and_contains_now() {
        let now = ts(2026, 11, 18, 8, 0);
        let (start, end) = local_day(now, &zone("Europe/Stockholm")).expect("day");
        assert_eq!(end - start, SignedDuration::from_hours(24));
        assert!(start <= now && now < end);
    }

    #[test]
    fn day_stats_cover_only_the_requested_interval() {
        let series = quarter_hours(ts(2026, 11, 18, 0, 0), &[10.0, 20.0, 30.0, 100.0]);
        let stats =
            day_stats(&series, ts(2026, 11, 18, 0, 0), ts(2026, 11, 18, 0, 30)).expect("stats");
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 20.0);
        assert!((stats.avg - 15.0).abs() < f64::EPSILON);
        assert!(day_stats(&series, ts(2026, 11, 19, 0, 0), ts(2026, 11, 20, 0, 0)).is_none());
    }

    #[test]
    fn cheapest_window_is_the_lowest_average_run() {
        let series = quarter_hours(ts(2026, 11, 18, 0, 0), &[50.0, 40.0, 5.0, 6.0, 30.0, 30.0]);
        let start = cheapest_window(
            &series,
            ts(2026, 11, 18, 0, 0),
            SignedDuration::from_mins(30),
        );
        assert_eq!(start, Some(ts(2026, 11, 18, 0, 30)));
    }

    #[test]
    fn cheapest_window_never_spans_a_gap() {
        // Points at 00:00 and 00:15, a gap, then 01:00 and 01:15. The cheap pair at 00:15 and
        // 01:00 is not contiguous, so 00:15 must not be picked. Both 00:00 and 01:00 are valid
        // windows averaging 45.5, and the earliest wins.
        let points = vec![
            PricePoint {
                start: ts(2026, 11, 18, 0, 0),
                eur_per_mwh: 90.0,
            },
            PricePoint {
                start: ts(2026, 11, 18, 0, 15),
                eur_per_mwh: 1.0,
            },
            PricePoint {
                start: ts(2026, 11, 18, 1, 0),
                eur_per_mwh: 1.0,
            },
            PricePoint {
                start: ts(2026, 11, 18, 1, 15),
                eur_per_mwh: 90.0,
            },
        ];
        let series = PriceSeries::new("SE3", Resolution::Minutes15, points).expect("valid");
        let start = cheapest_window(
            &series,
            ts(2026, 11, 18, 0, 0),
            SignedDuration::from_mins(30),
        );
        assert_eq!(start, Some(ts(2026, 11, 18, 0, 0)));
    }

    #[test]
    fn cheapest_window_respects_the_start_bound() {
        let series = quarter_hours(ts(2026, 11, 18, 0, 0), &[1.0, 1.0, 9.0, 9.0]);
        let start = cheapest_window(
            &series,
            ts(2026, 11, 18, 0, 30),
            SignedDuration::from_mins(30),
        );
        assert_eq!(start, Some(ts(2026, 11, 18, 0, 30)));
    }
}
