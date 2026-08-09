//! search-api — product search service for the Marketly platform.
//!
//! Exposes a single read-only `GET /search?q=` endpoint backed by an
//! in-memory inverted index. The index is seeded at startup from a small
//! static catalog (in a real deployment it would be hydrated from the
//! catalog-service via gRPC, but for now we keep it local so the service
//! is self-contained and easy to reason about).

mod error;
mod handlers;
mod index;
mod models;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::index::SearchIndex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(true)
        .json()
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let index = Arc::new(SearchIndex::with_seed_catalog());

    let app = Router::new()
        .route("/search", get(handlers::search))
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(index);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "search-api listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
