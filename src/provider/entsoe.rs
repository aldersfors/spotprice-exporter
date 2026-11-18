//! ENTSO-E Transparency Platform day-ahead prices (document type A44).
//!
//! Only the parts of the schema this exporter needs are read: `TimeSeries`, `Period`
//! (`timeInterval/start`, `resolution`), and `Point` (`position`, `price.amount`). Prices are
//! positioned as `period start + (position - 1) * resolution`. Acknowledgement documents with
//! reason code 999 mean no data.
//!
//! The parser in this file is tested against synthetic documents shaped like the A44 schema.
//! Real responses from backlog spike S0.2 must be added as fixtures before this is trusted.

use std::time::Duration;

use jiff::Timestamp;
use quick_xml::{Reader, events::Event};
use secrecy::{ExposeSecret, SecretString};

use super::{PriceProvider, ProviderError};
use crate::model::{PricePoint, PriceSeries, Resolution};

/// Production REST endpoint.
pub const DEFAULT_BASE_URL: &str = "https://web-api.tp.entsoe.eu/api";

/// ENTSO-E reason code meaning "no matching data found".
const REASON_NO_DATA: &str = "999";

/// HTTP client for ENTSO-E day-ahead prices.
#[derive(Debug, Clone)]
pub struct EntsoeProvider {
    client: reqwest::Client,
    base_url: String,
    token: SecretString,
}

impl EntsoeProvider {
    /// Create a provider with the production endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Transient`] if the HTTP client cannot be built.
    pub fn new(token: SecretString) -> Result<Self, ProviderError> {
        Self::with_base_url(DEFAULT_BASE_URL, token)
    }

    /// Create a provider with a custom base URL (used by tests against a mock server).
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Transient`] if the HTTP client cannot be built.
    pub fn with_base_url(base_url: &str, token: SecretString) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ProviderError::Transient(e.without_url().to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.to_owned(),
            token,
        })
    }
}

impl PriceProvider for EntsoeProvider {
    async fn fetch(
        &self,
        zone: &str,
        from: Timestamp,
        to: Timestamp,
    ) -> Result<PriceSeries, ProviderError> {
        let period_start = from.strftime("%Y%m%d%H%M").to_string();
        let period_end = to.strftime("%Y%m%d%H%M").to_string();
        let response = self
            .client
            .get(&self.base_url)
            .query(&[
                ("securityToken", self.token.expose_secret()),
                ("documentType", "A44"),
                ("in_Domain", zone),
                ("out_Domain", zone),
                ("periodStart", period_start.as_str()),
                ("periodEnd", period_end.as_str()),
            ])
            .send()
            .await
            // Errors carry the request URL, which includes the token. Strip it.
            .map_err(|e| ProviderError::Transient(e.without_url().to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::Unauthorized);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        let body = response
            .text()
            .await
            .map_err(|e| ProviderError::Transient(e.without_url().to_string()))?;
        if !status.is_success() && !body.contains("Acknowledgement_MarketDocument") {
            return Err(ProviderError::Transient(format!("HTTP {status}")));
        }
        parse_a44(zone, &body)
    }
}

/// Parse an A44 document into a series for `zone`.
///
/// # Errors
///
/// Returns [`ProviderError::NoData`] for reason code 999 or a document with no points,
/// [`ProviderError::Malformed`] for XML or timestamp problems, and [`ProviderError::Series`] when
/// the points break a series invariant.
pub fn parse_a44(zone: &str, xml: &str) -> Result<PriceSeries, ProviderError> {
    let mut reader = Reader::from_str(xml);
    let mut collector = A44Collector::default();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| ProviderError::Malformed(format!("XML: {e}")))?;
        match event {
            Event::Start(start) => {
                collector.start(&String::from_utf8_lossy(start.local_name().as_ref()));
            }
            Event::End(end) => {
                collector.end(&String::from_utf8_lossy(end.local_name().as_ref()))?;
            }
            Event::Text(text) => {
                let value = text
                    .decode()
                    .map_err(|e| ProviderError::Malformed(format!("text: {e}")))?;
                collector.text(value.trim())?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    collector.finish(zone)
}

/// Accumulates A44 elements while the XML is read, then produces the series.
#[derive(Debug, Default)]
struct A44Collector {
    path: Vec<String>,
    period_start: Option<Timestamp>,
    period_resolution: Option<Resolution>,
    position: Option<usize>,
    price: Option<f64>,
    resolutions_seen: Vec<Resolution>,
    points: Vec<PricePoint>,
    ack_reason: Option<String>,
    is_acknowledgement: bool,
}

impl A44Collector {
    fn start(&mut self, name: &str) {
        match name {
            "Acknowledgement_MarketDocument" => self.is_acknowledgement = true,
            "Period" => {
                self.period_start = None;
                self.period_resolution = None;
            }
            "Point" => {
                self.position = None;
                self.price = None;
            }
            _ => {}
        }
        self.path.push(name.to_owned());
    }

    fn end(&mut self, name: &str) -> Result<(), ProviderError> {
        match name {
            "Point" => self.close_point()?,
            "Period" => {
                if let Some(res) = self.period_resolution {
                    self.resolutions_seen.push(res);
                }
            }
            _ => {}
        }
        self.path.pop();
        Ok(())
    }

    fn close_point(&mut self) -> Result<(), ProviderError> {
        let (Some(pos), Some(amount), Some(start), Some(res)) = (
            self.position,
            self.price,
            self.period_start,
            self.period_resolution,
        ) else {
            return Err(ProviderError::Malformed(
                "Point without position, price, period start or resolution".to_owned(),
            ));
        };
        if pos == 0 {
            return Err(ProviderError::Malformed("position is 1-based".to_owned()));
        }
        let steps = i32::try_from(pos - 1).unwrap_or(i32::MAX);
        self.points.push(PricePoint {
            start: start + res.duration() * steps,
            eur_per_mwh: amount,
        });
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), ProviderError> {
        if value.is_empty() {
            return Ok(());
        }
        let current = self.path.last().map(String::as_str);
        let parent = self.path.iter().rev().nth(1).map(String::as_str);
        match (current, parent) {
            (Some("start"), Some("timeInterval")) => {
                self.period_start = Some(parse_timestamp(value)?);
            }
            (Some("resolution"), Some("Period")) => {
                self.period_resolution =
                    Some(Resolution::from_iso8601(value).ok_or_else(|| {
                        ProviderError::Malformed(format!("unsupported resolution {value}"))
                    })?);
            }
            (Some("position"), Some("Point")) => {
                self.position = Some(
                    value
                        .parse()
                        .map_err(|_| ProviderError::Malformed(format!("bad position {value}")))?,
                );
            }
            (Some("price.amount"), Some("Point")) => {
                self.price = Some(
                    value
                        .parse()
                        .map_err(|_| ProviderError::Malformed(format!("bad price {value}")))?,
                );
            }
            (Some("code"), Some("Reason")) => self.ack_reason = Some(value.to_owned()),
            _ => {}
        }
        Ok(())
    }

    fn finish(mut self, zone: &str) -> Result<PriceSeries, ProviderError> {
        if self.is_acknowledgement {
            return match self.ack_reason.as_deref() {
                Some(REASON_NO_DATA) => Err(ProviderError::NoData),
                Some(code) => Err(ProviderError::Malformed(format!(
                    "acknowledgement reason {code}"
                ))),
                None => Err(ProviderError::Malformed(
                    "acknowledgement without reason".to_owned(),
                )),
            };
        }
        if self.points.is_empty() {
            return Err(ProviderError::NoData);
        }
        let resolution = self
            .resolutions_seen
            .first()
            .copied()
            .ok_or_else(|| ProviderError::Malformed("no period resolution".to_owned()))?;
        if self.resolutions_seen.iter().any(|r| *r != resolution) {
            return Err(ProviderError::Malformed(
                "mixed resolutions in one document".to_owned(),
            ));
        }
        self.points.sort_by_key(|p| p.start);
        Ok(PriceSeries::new(zone, resolution, self.points)?)
    }
}

fn parse_timestamp(value: &str) -> Result<Timestamp, ProviderError> {
    // ENTSO-E writes timestamps as `YYYY-MM-DDTHH:MMZ`, without seconds. Add them, then parse as RFC 3339.
    let without_zone = value
        .strip_suffix('Z')
        .ok_or_else(|| ProviderError::Malformed(format!("timestamp {value} is not UTC")))?;
    format!("{without_zone}:00Z")
        .parse::<Timestamp>()
        .map_err(|e| ProviderError::Malformed(format!("timestamp {value}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> Timestamp {
        jiff::civil::date(y as i16, mo as i8, d as i8)
            .at(h as i8, mi as i8, 0, 0)
            .to_zoned_datetime(jiff::tz::TimeZone::UTC)
            .expect("valid")
            .timestamp()
    }

    /// Synthetic A44 document (not a real ENTSO-E response). Two periods, hourly then 15-minute,
    /// in separate series. Replace with real S0.2 fixtures when available.
    fn synthetic_hourly() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Publication_MarketDocument xmlns="urn:iec62325.351:tc57wg16:451-3:publicationdocument:7:3">
  <TimeSeries>
    <Period>
      <timeInterval><start>2026-11-17T23:00Z</start><end>2026-11-18T01:00Z</end></timeInterval>
      <resolution>PT60M</resolution>
      <Point><position>1</position><price.amount>42.10</price.amount></Point>
      <Point><position>2</position><price.amount>-3.50</price.amount></Point>
    </Period>
  </TimeSeries>
</Publication_MarketDocument>"#
    }

    fn synthetic_quarter_hour() -> &'static str {
        r#"<Publication_MarketDocument>
  <TimeSeries>
    <Period>
      <timeInterval><start>2026-11-17T23:00Z</start><end>2026-11-17T23:45Z</end></timeInterval>
      <resolution>PT15M</resolution>
      <Point><position>1</position><price.amount>10.0</price.amount></Point>
      <Point><position>2</position><price.amount>11.0</price.amount></Point>
      <Point><position>3</position><price.amount>12.0</price.amount></Point>
    </Period>
  </TimeSeries>
</Publication_MarketDocument>"#
    }

    fn utc(h: i32, m: i32) -> Timestamp {
        ts(2026, 11, 17, h, m)
    }

    #[test]
    fn parses_hourly_document_with_negative_prices() {
        let series = parse_a44("SE3", synthetic_hourly()).expect("parses");
        assert_eq!(series.resolution(), Resolution::Hour);
        assert_eq!(series.points().len(), 2);
        assert_eq!(series.points()[0].start, utc(23, 0));
        assert_eq!(series.points()[1].start, ts(2026, 11, 18, 0, 0));
        assert!((series.points()[1].eur_per_mwh - -3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parses_quarter_hour_positions_into_utc_starts() {
        let series = parse_a44("SE3", synthetic_quarter_hour()).expect("parses");
        assert_eq!(series.resolution(), Resolution::Minutes15);
        let starts: Vec<_> = series.points().iter().map(|p| p.start).collect();
        assert_eq!(starts, vec![utc(23, 0), utc(23, 15), utc(23, 30)]);
    }

    #[test]
    fn acknowledgement_999_is_no_data() {
        let xml = r"<Acknowledgement_MarketDocument>
  <Reason><code>999</code><text>No matching data found</text></Reason>
</Acknowledgement_MarketDocument>";
        assert!(matches!(parse_a44("SE3", xml), Err(ProviderError::NoData)));
    }

    #[test]
    fn other_acknowledgement_is_malformed() {
        let xml = r"<Acknowledgement_MarketDocument>
  <Reason><code>400</code><text>Bad request</text></Reason>
</Acknowledgement_MarketDocument>";
        assert!(matches!(
            parse_a44("SE3", xml),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn empty_document_is_no_data() {
        let xml = "<Publication_MarketDocument></Publication_MarketDocument>";
        assert!(matches!(parse_a44("SE3", xml), Err(ProviderError::NoData)));
    }

    #[test]
    fn position_zero_is_rejected() {
        let xml = r"<Publication_MarketDocument><TimeSeries><Period>
  <timeInterval><start>2026-11-17T23:00Z</start></timeInterval>
  <resolution>PT60M</resolution>
  <Point><position>0</position><price.amount>1</price.amount></Point>
</Period></TimeSeries></Publication_MarketDocument>";
        assert!(matches!(
            parse_a44("SE3", xml),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn unsupported_resolution_is_rejected() {
        let xml = r"<Publication_MarketDocument><TimeSeries><Period>
  <timeInterval><start>2026-11-17T23:00Z</start></timeInterval>
  <resolution>P1D</resolution>
  <Point><position>1</position><price.amount>1</price.amount></Point>
</Period></TimeSeries></Publication_MarketDocument>";
        assert!(matches!(
            parse_a44("SE3", xml),
            Err(ProviderError::Malformed(_))
        ));
    }
}
