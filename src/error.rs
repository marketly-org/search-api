//! Error type for the search API.
//!
//! Implements `IntoResponse` so handlers can use `?` to short-circuit on
//! bad input without panicking the worker thread.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),

    // Reserved for internal errors (unavailable index, etc.). Unused on
    // main for now; kept so handlers can grow non-panic error paths.
    #[allow(dead_code)]
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    // Convenience constructor for handlers. Unused on main for now —
    // kept for the missing-q-parameter fix (see infra/ground-truth.md).
    #[allow(dead_code)]
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}
