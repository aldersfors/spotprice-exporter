//! OpenTelemetry metrics pipeline.
//!
//! The OTLP endpoint and headers come from the standard `OTEL_EXPORTER_OTLP_*` environment
//! variables, read by the SDK. Nothing is hard-coded here.

use opentelemetry::KeyValue;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};

/// Errors while building the metrics pipeline.
#[derive(Debug, thiserror::Error)]
#[error("failed to build OTLP metric exporter: {0}")]
pub struct TelemetryError(String);

/// Build the meter provider with an OTLP/HTTP exporter and a periodic reader.
///
/// # Errors
///
/// Returns [`TelemetryError`] when the exporter cannot be built from the environment.
pub fn init_meter_provider() -> Result<SdkMeterProvider, TelemetryError> {
    let exporter = MetricExporter::builder()
        .with_http()
        .build()
        .map_err(|e| TelemetryError(e.to_string()))?;
    let resource = Resource::builder()
        .with_service_name("spotprice-exporter")
        .with_attribute(KeyValue::new("service.version", crate::version()))
        .build();
    Ok(SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter).build())
        .with_resource(resource)
        .build())
}
