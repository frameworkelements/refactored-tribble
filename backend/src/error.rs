use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application-wide error type. Every variant maps to a clean JSON response;
/// internal details are logged but never leaked to the client.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("{0}")]
    BadRequest(String),

    #[error("internal server error")]
    Internal(#[from] anyhow_lite::Error),
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Internal(anyhow_lite::Error::wrap(other)),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// A tiny stand-in for `anyhow` so we avoid an extra dependency while still
/// being able to wrap arbitrary errors behind the opaque `Internal` variant.
pub mod anyhow_lite {
    use std::fmt;

    #[derive(Debug)]
    pub struct Error(Box<dyn std::error::Error + Send + Sync + 'static>);

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }

    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.0.as_ref())
        }
    }

    impl Error {
        /// Wrap any concrete error behind the opaque internal error.
        pub fn wrap<E>(e: E) -> Self
        where
            E: std::error::Error + Send + Sync + 'static,
        {
            Error(Box::new(e))
        }

        pub fn msg(m: impl Into<String>) -> Self {
            Error(Box::<dyn std::error::Error + Send + Sync>::from(m.into()))
        }
    }
}
