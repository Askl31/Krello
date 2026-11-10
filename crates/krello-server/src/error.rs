//! HTTP error mapping: core `Error` → status codes + JSON body.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use krello_core::Error as CoreError;

pub struct ApiError(pub CoreError);

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self.0 {
            CoreError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            CoreError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            CoreError::Cycle(_) => (StatusCode::CONFLICT, "dependency_cycle"),
            CoreError::Validation(_) => (StatusCode::BAD_REQUEST, "validation"),
            CoreError::Ai(msg) if msg.starts_with("unavailable:") => {
                (StatusCode::SERVICE_UNAVAILABLE, "ai_unavailable")
            }
            CoreError::Ai(_) => (StatusCode::BAD_GATEWAY, "ai_error"),
            CoreError::Sql(_) | CoreError::Io(_) | CoreError::Json(_) | CoreError::Lock => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        let body = serde_json::json!({
            "error": kind,
            "message": self.0.to_string(),
        });
        (status, axum::Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
