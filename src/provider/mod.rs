//! Price sources. A provider turns a bidding zone and a UTC window into a [`PriceSeries`].

pub mod entsoe;

use std::future::Future;

use jiff::Timestamp;
use thiserror::Error;

use crate::model::{PriceSeries, SeriesError};

/// Errors from a price source.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The source has no prices for the requested window yet (for example, next-day prices before
    /// publication). Retried later with backoff.
    #[error("no prices published for the requested window")]
    NoData,
    /// The source rejected the credentials. Retrying will not help until the token is fixed.
    #[error("authentication rejected (HTTP 401): check the ENTSO-E security token")]
    Unauthorized,
    /// The source is rate limiting this client.
    #[error("rate limited (HTTP 429)")]
    RateLimited,
    /// A network or server failure. Retried with backoff.
    #[error("transient failure: {0}")]
    Transient(String),
    /// The response could not be parsed into prices.
    #[error("unexpected response: {0}")]
    Malformed(String),
    /// The parsed prices violated a series invariant.
    #[error(transparent)]
    Series(#[from] SeriesError),
}

impl ProviderError {
    /// Whether retrying without a configuration change is pointless.
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }
}

/// A source of day-ahead prices.
pub trait PriceProvider: Send + Sync {
    /// Fetch prices for `zone` covering `[from, to)`.
    fn fetch(
        &self,
        zone: &str,
        from: Timestamp,
        to: Timestamp,
    ) -> impl Future<Output = Result<PriceSeries, ProviderError>> + Send;
}
