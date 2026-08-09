//! Request / response types for the search API.

use serde::{Deserialize, Serialize};

/// One product hit returned by `GET /search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price_cents: u64,
    /// Relevance score in [0.0, 1.0]. Higher is better.
    pub score: f64,
}

/// Body returned by `GET /search?q=...`.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub total: usize,
    pub hits: Vec<SearchHit>,
}

/// Body returned by the health / ready endpoints.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}
