//! HTTP handlers for the search API.
use crate::error::AppError;
use crate::index::SearchIndex;
use crate::models::{HealthResponse, SearchResponse};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub async fn search(
    State(index): State<Arc<SearchIndex>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    let query = match params.q {
        Some(q) => q,
        None => {
            return Err(AppError::BadRequest(
                "Missing required query parameter 'q'".into(),
            ));
        }
    };
    let limit = params.limit.unwrap_or(20);
    let hits = index.search(&query);
    let total = hits.len();
    let hits = hits.into_iter().take(limit).collect();
    Ok(Json(SearchResponse { query, total, hits }))
}

/// GET /health — liveness probe.
pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".into(),
        service: "search-api".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// GET /ready — readiness probe.
pub async fn ready(State(index): State<Arc<SearchIndex>>) -> impl IntoResponse {
    if index.len() == 0 {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "not_ready".into(),
                service: "search-api".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            }),
        );
    }
    (
        axum::http::StatusCode::OK,
        Json(HealthResponse {
            status: "ready".into(),
            service: "search-api".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn app() -> Router {
        let index = Arc::new(SearchIndex::with_seed_catalog());
        Router::new()
            .route("/search", get(search))
            .route("/health", get(health))
            .route("/ready", get(ready))
            .with_state(index)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ready_returns_ok_when_indexed() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn search_with_query_returns_hits() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/search?q=keyboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(!resp.hits.is_empty());
        assert!(resp
            .hits
            .iter()
            .all(|h| h.name.to_lowercase().contains("keyboard")));
    }
}
