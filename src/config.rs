//! Runtime configuration, read from CLI flags or `SPOTPRICE_*` environment variables.
//!
//! OpenTelemetry export is configured by the standard `OTEL_EXPORTER_OTLP_*` variables, which the
//! SDK reads directly. They are not duplicated here.

use std::{fs, path::PathBuf};

use clap::Parser;
use secrecy::SecretString;
use thiserror::Error;

/// Configuration errors. Each message names the field at fault.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// Arguments or environment could not be parsed.
    #[error("invalid configuration: {0}")]
    Parse(String),
    /// No bidding zones were configured.
    #[error("SPOTPRICE_ZONES must list at least one bidding zone")]
    NoZones,
    /// A zone is not in the registry. The message lists the known names.
    #[error("SPOTPRICE_ZONES contains an unknown zone {zone:?}; known zones: {known}")]
    UnknownZone {
        /// The value that was not recognised.
        zone: String,
        /// Comma-separated list of supported names.
        known: String,
    },
    /// Both the token and the token file were given.
    #[error("set either SPOTPRICE_ENTSOE_TOKEN or SPOTPRICE_ENTSOE_TOKEN_FILE, not both")]
    TokenConflict,
    /// No token was given.
    #[error("set SPOTPRICE_ENTSOE_TOKEN or SPOTPRICE_ENTSOE_TOKEN_FILE")]
    TokenMissing,
    /// The token file could not be read.
    #[error("SPOTPRICE_ENTSOE_TOKEN_FILE could not be read: {0}")]
    TokenFile(String),
    /// The token resolved to an empty string.
    #[error("the ENTSO-E token is empty")]
    TokenEmpty,
}

/// Command-line and environment configuration.
#[derive(Debug, Clone, Parser)]
#[command(name = "spotprice-exporter", version, about)]
pub struct Config {
    /// Bidding zones to export, comma separated (for example `SE3,SE4,NO2`).
    #[arg(long, env = "SPOTPRICE_ZONES", value_delimiter = ',', required = true)]
    pub zones: Vec<String>,

    /// ENTSO-E Transparency Platform security token.
    #[arg(long, env = "SPOTPRICE_ENTSOE_TOKEN", hide_env_values = true)]
    pub entsoe_token: Option<String>,

    /// Path to a file containing the ENTSO-E token. Use with Kubernetes secret mounts.
    #[arg(long, env = "SPOTPRICE_ENTSOE_TOKEN_FILE")]
    pub entsoe_token_file: Option<PathBuf>,

    /// ENTSO-E REST base URL. Override only for tests against a mock server.
    #[arg(long, env = "SPOTPRICE_ENTSOE_BASE_URL", default_value = crate::provider::entsoe::DEFAULT_BASE_URL)]
    pub entsoe_base_url: String,

    /// Log level filter, used when `RUST_LOG` is not set.
    #[arg(long, env = "SPOTPRICE_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Emit logs as JSON.
    #[arg(long, env = "SPOTPRICE_LOG_JSON", default_value_t = true)]
    pub log_json: bool,

    /// Address for the health and readiness HTTP server.
    #[arg(long, env = "SPOTPRICE_LISTEN", default_value = "0.0.0.0:8080")]
    pub listen: String,
}

impl Config {
    /// Parse configuration from the process arguments and environment, then validate it.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the configuration is invalid.
    pub fn load() -> Result<Self, ConfigError> {
        Self::try_parse_from(std::env::args_os())
            .map_err(|e| ConfigError::Parse(e.to_string()))
            .and_then(Self::validated)
    }

    /// Validate an already-parsed configuration. Zone names are resolved to EIC codes, duplicates
    /// are removed, and blank entries are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when no zone is given, a zone is unknown, or the token is invalid.
    pub fn validated(mut self) -> Result<Self, ConfigError> {
        let mut resolved: Vec<String> = Vec::new();
        for value in &self.zones {
            if value.trim().is_empty() {
                continue;
            }
            let zone = crate::zones::lookup(value).ok_or_else(|| ConfigError::UnknownZone {
                zone: value.clone(),
                known: crate::zones::ZONES
                    .iter()
                    .map(|z| z.name)
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;
            if !resolved.iter().any(|eic| eic == zone.eic) {
                resolved.push(zone.eic.to_owned());
            }
        }
        if resolved.is_empty() {
            return Err(ConfigError::NoZones);
        }
        self.zones = resolved;
        self.resolve_token()?;
        Ok(self)
    }

    /// Resolve the ENTSO-E token from either the direct value or the file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when neither or both sources are set, the file is unreadable,
    /// or the token is empty.
    pub fn resolve_token(&self) -> Result<SecretString, ConfigError> {
        let raw = match (&self.entsoe_token, &self.entsoe_token_file) {
            (Some(_), Some(_)) => return Err(ConfigError::TokenConflict),
            (None, None) => return Err(ConfigError::TokenMissing),
            (Some(token), None) => token.clone(),
            (None, Some(path)) => fs::read_to_string(path)
                .map_err(|e| ConfigError::TokenFile(e.kind().to_string()))?,
        };
        let token = raw.trim().to_owned();
        if token.is_empty() {
            return Err(ConfigError::TokenEmpty);
        }
        Ok(SecretString::from(token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn parse(args: &[&str]) -> Result<Config, ConfigError> {
        let mut full = vec!["spotprice-exporter"];
        full.extend_from_slice(args);
        Config::try_parse_from(full)
            .map_err(|e| ConfigError::Parse(e.to_string()))
            .and_then(Config::validated)
    }

    #[test]
    fn accepts_comma_separated_zones_with_direct_token() {
        let cfg = parse(&[
            "--zones",
            "10Y1001A1001A46L,10Y1001A1001A47J",
            "--entsoe-token",
            "abc",
        ])
        .expect("valid config");
        assert_eq!(cfg.zones.len(), 2);
    }

    #[test]
    fn rejects_unknown_zone() {
        let err = parse(&["--zones", "SE9", "--entsoe-token", "abc"]).expect_err("unknown");
        assert!(matches!(err, ConfigError::UnknownZone { zone, .. } if zone == "SE9"));
    }

    #[test]
    fn resolves_names_to_eic_codes_and_removes_duplicates() {
        let cfg = parse(&["--zones", "se3,SE4,SE3,", "--entsoe-token", "abc"]).expect("valid");
        assert_eq!(
            cfg.zones,
            vec!["10Y1001A1001A46L".to_owned(), "10Y1001A1001A47J".to_owned()]
        );
    }

    #[test]
    fn rejects_token_when_both_sources_set() {
        let err = parse(&[
            "--zones",
            "10Y1001A1001A46L",
            "--entsoe-token",
            "abc",
            "--entsoe-token-file",
            "/does/not/matter",
        ])
        .expect_err("conflict");
        assert_eq!(err, ConfigError::TokenConflict);
    }

    #[test]
    fn rejects_missing_token() {
        let err = parse(&["--zones", "10Y1001A1001A46L"]).expect_err("missing");
        assert_eq!(err, ConfigError::TokenMissing);
    }

    #[test]
    fn reads_token_from_file_and_trims_whitespace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("token");
        let mut file = fs::File::create(&path).expect("create");
        writeln!(file, "  secret-value  ").expect("write");
        let path_str = path.to_str().expect("utf8 path");
        let cfg = parse(&[
            "--zones",
            "10Y1001A1001A46L",
            "--entsoe-token-file",
            path_str,
        ])
        .expect("valid");
        assert!(cfg.resolve_token().is_ok());
    }

    #[test]
    fn rejects_empty_token_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("token");
        fs::write(&path, "   \n").expect("write");
        let path_str = path.to_str().expect("utf8 path");
        let err = parse(&[
            "--zones",
            "10Y1001A1001A46L",
            "--entsoe-token-file",
            path_str,
        ])
        .expect_err("empty");
        assert_eq!(err, ConfigError::TokenEmpty);
    }
}
