//! Binary entry point for spotprice-exporter.

use std::{
    process::ExitCode,
    sync::{Arc, RwLock},
};

use opentelemetry::metrics::MeterProvider;
use spotprice_exporter::{
    config::Config,
    health, metrics,
    provider::entsoe::EntsoeProvider,
    scheduler::{self, SchedulerConfig},
    state::MarketState,
    telemetry,
};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> ExitCode {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("spotprice-exporter: {err}");
            return ExitCode::from(2);
        }
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.log_level.as_str()));
    let builder = fmt().with_env_filter(filter);
    if config.log_json {
        builder.json().init();
    } else {
        builder.init();
    }
    tracing::info!(
        version = spotprice_exporter::version(),
        zones = %config.zones.join(","),
        "starting spotprice-exporter"
    );

    match run(&config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "spotprice-exporter stopped with an error");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let token = config.resolve_token()?;
    let provider = Arc::new(EntsoeProvider::with_base_url(
        &config.entsoe_base_url,
        token,
    )?);

    let state = Arc::new(RwLock::new(MarketState::new(config.zones.iter().cloned())));

    let meter_provider = telemetry::init_meter_provider()?;
    // Take the meter from this provider directly. A meter from the global provider created before
    // `set_meter_provider` would be a no-op and silently export nothing.
    let meter = meter_provider.meter("spotprice-exporter");
    let _metrics = metrics::register(&meter, &state);
    let fetches = metrics::FetchCounter::new(&meter);

    let scheduler_task = tokio::spawn(scheduler::run(
        Arc::clone(&provider),
        Arc::clone(&state),
        SchedulerConfig::default(),
        fetches,
    ));

    let listener = tokio::net::TcpListener::bind(&config.listen).await?;
    tracing::info!(address = %config.listen, "health server listening");
    axum::serve(listener, health::router(Arc::clone(&state)))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    scheduler_task.abort();
    // Flush buffered metrics before exiting so the last interval is not lost.
    if let Err(err) = meter_provider.shutdown() {
        tracing::warn!(error = %err, "metrics flush on shutdown failed");
    }
    tracing::info!("spotprice-exporter stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
