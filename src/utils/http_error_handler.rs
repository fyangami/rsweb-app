use axum::extract::rejection::{
    FormRejection, JsonRejection, MatchedPathRejection, NestedPathRejection, PathRejection,
    QueryRejection, RawFormRejection, RawPathParamsRejection,
};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use serde::ser::{Serialize, Serializer};
use serde_json::json;
use std::fmt::Debug;
use tracing::{debug, error, info};

const ERROR_UNKNOWN: &str = "Unknown Error";

#[derive(Debug)]
pub struct ErrorCode {
    status_code: StatusCode,
    // may use for meaningful status
    code: Option<u16>,
}

impl Serialize for ErrorCode {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(code) = self.code {
            serializer.serialize_u16(code)
        } else {
            serializer.serialize_none()
        }
    }
}

impl ErrorCode {
    pub fn from_status_code(status_code: StatusCode) -> Self {
        Self {
            status_code,
            code: None,
        }
    }

    pub fn new_bad_request() -> Self {
        Self {
            status_code: StatusCode::BAD_REQUEST,
            code: None,
        }
    }

    pub fn new_internal_error() -> Self {
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            code: None,
        }
    }

    pub fn code(&mut self, code: u16) -> &Self {
        self.code = Some(code);
        self
    }
}

#[derive(Debug)]
pub enum ErrorResponse {
    InternalError(anyhow::Error),
    AppError {
        code: ErrorCode,
        message: Option<String>,
    },
}

pub type Result<T, E = ErrorResponse> = core::result::Result<T, E>;

impl ErrorResponse {
    pub fn new_with_message(message: &str) -> Self {
        Self::AppError {
            code: ErrorCode::from_status_code(StatusCode::BAD_REQUEST),
            message: Some(message.to_owned()),
        }
    }

    pub fn new_default() -> Self {
        Self::AppError {
            code: ErrorCode::from_status_code(StatusCode::BAD_REQUEST),
            message: None,
        }
    }

    pub fn new(code: ErrorCode, message: Option<String>) -> Self {
        Self::AppError { code, message }
    }

    pub fn new_with_status_code(status_code: StatusCode) -> Self {
        Self::AppError {
            code: ErrorCode::from_status_code(status_code),
            message: None,
        }
    }

    pub fn new_forb() -> Self {
        Self::new_with_status_code(StatusCode::FORBIDDEN)
    }

    pub fn new_no_auth() -> Self {
        Self::new_with_status_code(StatusCode::UNAUTHORIZED)
    }
}

macro_rules! match_rejection {
    ( $e: expr, $($rej: ty), +) => {
        $(
        match $e.downcast_ref::<$rej>() {
            Some(e) => {
                info!(e = ?e, "general bad params rejection incur");
                return into_json_response(StatusCode::BAD_REQUEST, &e.body_text());
            },
            _ => {}
        }
        ) +
    };
}

fn into_json_response(status_code: StatusCode, message: &str) -> Response {
    (
        status_code,
        serde_json::to_string(&json!({
            "code": status_code.as_u16(),
            "message": message
        }))
        // must not be failed
        .unwrap_or("{}".to_owned()),
    )
        .into_response()
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        match self {
            Self::InternalError(e) => {
                // well be returned when rejection matches
                match_rejection!(
                    e,
                    JsonRejection,
                    QueryRejection,
                    PathRejection,
                    MatchedPathRejection,
                    NestedPathRejection,
                    FormRejection,
                    RawFormRejection,
                    RawPathParamsRejection
                );
                error!(e = ?e, source_err = ?e.source(), "unprocessable error incur");
            }
            Self::AppError { code, message } => {
                debug!(code = ?code, message = message, "app error is returned");
                let message = message
                    .as_deref()
                    .unwrap_or(code.status_code.canonical_reason().unwrap_or(ERROR_UNKNOWN));
                return into_json_response(code.status_code, message);
            }
        }
        into_json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::INTERNAL_SERVER_ERROR
                .canonical_reason()
                .unwrap_or(ERROR_UNKNOWN),
        )
    }
}

impl<E> From<E> for ErrorResponse
where
    E: Into<anyhow::Error> + Debug,
{
    fn from(e: E) -> Self {
        Self::InternalError(e.into())
    }
}

pub fn new_fallback_response_handler() -> Router {
    Router::new().fallback(any(|| async {
        ErrorResponse::new_with_status_code(StatusCode::NOT_FOUND)
    }))
}


#[macro_export]
/// unhandled error now.
/// to handling when required
macro_rules! unhandled_error {
    ($result:expr, $msg:tt) => {
        match $result {
            Err(e) => {
                error!(e = ?e, $msg);
            },
            _ => {}
        }
        
    };
}
