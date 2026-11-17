//! Decides when to fetch prices and keeps the cache filled.
//!
//! Policy, per zone:
//! - Coverage target: today (UTC) and tomorrow. Before `publish_after` (UTC), tomorrow's prices
//!   are not expected yet, so the target is the end of today. After it, the target is the end of
//!   tomorrow.
//! - One request per zone per attempt, covering `[today 00:00 UTC, target)`.
//! - Success: next attempt in 15 minutes, in case the target is not yet fully covered.
//! - No data yet: retry in 15 minutes, without counting as a failure.
//! - Transient failure: exponential backoff from 1 minute, capped at 60 minutes.
//! - Unauthorized: retry hourly at most. Fix the token and restart.
//!
//! Worst case is one request per zone per 15 minutes, so a 4-zone setup uses at most 384 requests
//! per day, well under the documented 400 requests per minute ENTSO-E limit.

use std::{
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock},
    time::Duration,
};

use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use tracing::{debug, error, info, warn};

use crate::{
    metrics::FetchCounter,
    provider::{PriceProvider, ProviderError},
    state::MarketState,
};

/// Wait before re-checking after a success or a no-data response.
pub const REFRESH_INTERVAL: TimeDelta = TimeDelta::minutes(15);
/// Backoff cap after repeated transient failures.
pub const MAX_BACKOFF: TimeDelta = TimeDelta::minutes(60);
/// Backoff base after the first transient failure.
pub const BASE_BACKOFF: TimeDelta = TimeDelta::minutes(1);
/// Wait after an authentication failure before trying again.
pub const UNAUTHORIZED_RETRY: TimeDelta = TimeDelta::minutes(60);

/// Scheduler settings.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerConfig {
    /// How often the scheduler wakes to check zones.
    pub tick: Duration,
    /// UTC time after which next-day prices are expected.
    pub publish_after: NaiveTime,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick: Duration::from_secs(60),
            // 11:00 UTC is before the 12:45 CET clearing time, so it is safe as an early attempt.
            publish_after: NaiveTime::from_hms_opt(11, 0, 0).unwrap_or(NaiveTime::MIN),
        }
    }
}

/// End of the coverage the scheduler should have for `now`.
#[must_use]
pub fn coverage_target(now: DateTime<Utc>, publish_after: NaiveTime) -> DateTime<Utc> {
    let today = now.date_naive();
    let days_ahead = if now.time() < publish_after { 1 } else { 2 };
    let target_day = today + TimeDelta::days(days_ahead);
    target_day.and_time(NaiveTime::MIN).and_utc()
}

/// Whether a fetch should be attempted now for one zone.
#[must_use]
pub fn should_fetch(
    covered_until: Option<DateTime<Utc>>,
    target: DateTime<Utc>,
    next_attempt: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    let needs_data = covered_until.is_none_or(|covered| covered < target);
    let attempt_due = next_attempt.is_none_or(|at| now >= at);
    needs_data && attempt_due
}

/// Backoff after `failures` consecutive transient failures (1-based).
#[must_use]
pub fn backoff_after(failures: u32) -> TimeDelta {
    let exponent = failures.saturating_sub(1).min(16);
    let factor = i64::from(2_u32.pow(exponent));
    (BASE_BACKOFF * i32::try_from(factor).unwrap_or(i32::MAX)).min(MAX_BACKOFF)
}

#[derive(Debug, Default, Clone, Copy)]
struct ZoneSchedule {
    next_attempt: Option<DateTime<Utc>>,
    failures: u32,
}

/// Label for a fetch outcome, used as the `energy.market.fetch.result` attribute.
#[must_use]
pub const fn outcome_label(result: &Result<(), &ProviderError>) -> &'static str {
    match result {
        Ok(()) => "success",
        Err(ProviderError::NoData) => "no_data",
        Err(ProviderError::Unauthorized) => "unauthorized",
        Err(ProviderError::RateLimited) => "rate_limited",
        Err(ProviderError::Transient(_)) => "transient",
        Err(ProviderError::Malformed(_) | ProviderError::Series(_)) => "malformed",
    }
}

/// Run the scheduler forever. Call from a task, and abort the task to stop it.
pub async fn run<P: PriceProvider>(
    provider: Arc<P>,
    state: Arc<RwLock<MarketState>>,
    config: SchedulerConfig,
    fetches: FetchCounter,
) {
    let mut schedules: HashMap<String, ZoneSchedule> = HashMap::new();
    let mut ticker = tokio::time::interval(config.tick);
    loop {
        ticker.tick().await;
        let now = Utc::now();
        let target = coverage_target(now, config.publish_after);
        let zones: Vec<String> = read(&state).zone_codes().map(str::to_owned).collect();

        for zone in zones {
            let covered = read(&state).covered_until(&zone);
            let schedule = schedules.entry(zone.clone()).or_default();
            if !should_fetch(covered, target, schedule.next_attempt, now) {
                continue;
            }

            let from = now.date_naive().and_time(NaiveTime::MIN).and_utc();
            let result = provider.fetch(&zone, from, target).await;
            fetches.record(&zone, outcome_label(&result.as_ref().map(|_| ())));
            match result {
                Ok(series) => {
                    let points = series.points().len();
                    write(&state).record_success(Utc::now(), series);
                    schedule.failures = 0;
                    schedule.next_attempt = Some(Utc::now() + REFRESH_INTERVAL);
                    info!(zone = %zone, points, "prices fetched");
                }
                Err(err) if err.is_fatal() => {
                    schedule.next_attempt = Some(Utc::now() + UNAUTHORIZED_RETRY);
                    error!(zone = %zone, error = %err, "price fetch rejected, retrying in 60 minutes");
                }
                Err(ProviderError::NoData) => {
                    schedule.next_attempt = Some(Utc::now() + REFRESH_INTERVAL);
                    debug!(zone = %zone, "no prices published yet for the window");
                }
                Err(err) => {
                    schedule.failures = schedule.failures.saturating_add(1);
                    let wait = backoff_after(schedule.failures);
                    schedule.next_attempt = Some(Utc::now() + wait);
                    warn!(
                        zone = %zone,
                        error = %err,
                        failures = schedule.failures,
                        retry_in_seconds = wait.num_seconds(),
                        "price fetch failed"
                    );
                }
            }
        }
    }
}

fn read(state: &RwLock<MarketState>) -> std::sync::RwLockReadGuard<'_, MarketState> {
    state.read().unwrap_or_else(PoisonError::into_inner)
}

fn write(state: &RwLock<MarketState>) -> std::sync::RwLockWriteGuard<'_, MarketState> {
    state.write().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 17, h, m, 0)
            .single()
            .expect("valid")
    }

    fn publish_after() -> NaiveTime {
        NaiveTime::from_hms_opt(11, 0, 0).expect("valid")
    }

    #[test]
    fn target_before_publication_is_end_of_today() {
        assert_eq!(
            coverage_target(at(9, 0), publish_after()),
            Utc.with_ymd_and_hms(2026, 11, 18, 0, 0, 0)
                .single()
                .expect("valid")
        );
    }

    #[test]
    fn target_after_publication_is_end_of_tomorrow() {
        assert_eq!(
            coverage_target(at(12, 30), publish_after()),
            Utc.with_ymd_and_hms(2026, 11, 19, 0, 0, 0)
                .single()
                .expect("valid")
        );
    }

    #[test]
    fn fetches_when_nothing_cached_and_no_attempt_pending() {
        let target = Utc
            .with_ymd_and_hms(2026, 11, 18, 0, 0, 0)
            .single()
            .expect("valid");
        assert!(should_fetch(None, target, None, at(9, 0)));
    }

    #[test]
    fn skips_when_coverage_reaches_target() {
        let target = Utc
            .with_ymd_and_hms(2026, 11, 18, 0, 0, 0)
            .single()
            .expect("valid");
        assert!(!should_fetch(Some(target), target, None, at(9, 0)));
    }

    #[test]
    fn honours_pending_backoff() {
        let target = Utc
            .with_ymd_and_hms(2026, 11, 18, 0, 0, 0)
            .single()
            .expect("valid");
        assert!(!should_fetch(None, target, Some(at(10, 0)), at(9, 59)));
        assert!(should_fetch(None, target, Some(at(10, 0)), at(10, 0)));
    }

    #[test]
    fn outcome_labels_are_stable() {
        assert_eq!(outcome_label(&Ok(())), "success");
        assert_eq!(outcome_label(&Err(&ProviderError::NoData)), "no_data");
        assert_eq!(
            outcome_label(&Err(&ProviderError::Unauthorized)),
            "unauthorized"
        );
        assert_eq!(
            outcome_label(&Err(&ProviderError::RateLimited)),
            "rate_limited"
        );
        assert_eq!(
            outcome_label(&Err(&ProviderError::Transient("x".into()))),
            "transient"
        );
        assert_eq!(
            outcome_label(&Err(&ProviderError::Malformed("x".into()))),
            "malformed"
        );
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff_after(1), TimeDelta::minutes(1));
        assert_eq!(backoff_after(2), TimeDelta::minutes(2));
        assert_eq!(backoff_after(3), TimeDelta::minutes(4));
        assert_eq!(backoff_after(7), TimeDelta::minutes(60));
        assert_eq!(backoff_after(100), TimeDelta::minutes(60));
    }
}
