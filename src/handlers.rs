//! HTTP handlers for the search API.
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use crate::error::AppError;
use crate::index::SearchIndex;
use crate::models::{HealthResponse, SearchResponse};

#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

/// GET /search?q=... — ranked product search (BUG: panics on missing q).
pub async fn search(State(index): State<Arc<SearchIndex>>, Query(params): Query<SearchQuery>) -> Result<Json<SearchResponse>, AppError> {
    let query = params.q.ok_or_else(|| AppError::BadRequest("missing q parameter".into()))?;
    let limit = params.limit.unwrap_or(20);
    let hits = index.search(&query);
    let total = hits.len();
    let hits = hits.into_iter().take(limit).collect();
    Ok(Json(SearchResponse { query, total, hits }))
}

/*
 * ------------------------------------------------------------------
 * BUG (search handler, line 18): `params.q.unwrap()` panics the
 * tokio worker thread when the `q` query parameter is missing
 * (`GET /search`) or empty.
 *
 * main.rs does NOT install tower_http::catch_panic::CatchPanic, so
 * the panic tears down the connection. Under load this surfaces as
 * intermittent 502s from the ingress and, once every tokio worker
 * has panicked once, the server stops accepting new connections
 * entirely → k8s liveness probes fail → CrashLoopBackOff.
 *
 * Pod logs (Sentinel sees):
 *   thread 'tokio-runtime-worker' panicked at src/handlers.rs:18:32:
 *   called `Option::unwrap()` on a `None` value
 *
 * FIX: replace `params.q.unwrap()` with
 *   params.q.ok_or_else(|| AppError::BadRequest("missing q parameter".into()))?
 * so a missing `q` returns a clean 400 instead of crashing the worker.
 * ------------------------------------------------------------------
 */

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
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ready_returns_ok_when_indexed() {
        let res = app()
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
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
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let resp: SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(!resp.hits.is_empty());
        assert!(resp
            .hits
            .iter()
            .all(|h| h.name.to_lowercase().contains("keyboard")));
    }

    // NOTE: A test for `GET /search` (no `q`) is deliberately omitted.
    // The current implementation panics, which would crash the test
    // runner. Sentinel should fix the bug (replace `.unwrap()` with
    // `.ok_or_else(...)?`) and then add the missing test:
    //
    //   #[tokio::test]
    //   async fn search_without_q_returns_400() {
    //       let res = app()
    //           .oneshot(Request::builder().uri("/search").body(Body::empty()).unwrap())
    //           .await
    //           .unwrap();
    //       assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    //   }
}
