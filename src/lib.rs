//! Core library for spotprice-exporter: day-ahead electricity prices as OpenTelemetry metrics.

pub mod config;
pub mod health;
pub mod metrics;
pub mod model;
pub mod provider;
pub mod scheduler;
pub mod state;
pub mod telemetry;

/// Crate version as set in `Cargo.toml`.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_matches_cargo_metadata() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
