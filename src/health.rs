//! Kubernetes probe endpoints.
//!
//! `/healthz` is liveness: it is 200 whenever the process serves HTTP.
//! `/ready` is readiness: it is 200 once at least one zone has fetched prices successfully.

use std::sync::{Arc, PoisonError, RwLock};

use axum::{Router, extract::State, http::StatusCode, routing::get};

use crate::state::MarketState;

/// Build the probe router.
pub fn router(state: Arc<RwLock<MarketState>>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/ready", get(ready))
        .with_state(state)
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn ready(State(state): State<Arc<RwLock<MarketState>>>) -> StatusCode {
    let cache = state.read().unwrap_or_else(PoisonError::into_inner);
    if cache.has_any_data() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
