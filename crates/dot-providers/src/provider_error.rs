//! The error kinds the UI tells apart, and how HTTP failures map onto them.
//! Must not decide what the user is shown; the caller words the message.

use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("could not reach the model server: {0}")]
    Unreachable(String),
    #[error("the API key was rejected")]
    AuthFailed,
    #[error("the model was not found")]
    ModelNotFound,
    #[error("the provider is rate limiting requests")]
    RateLimited,
    #[error("the model server returned an error{}: {body}", status.map(|code| format!(" (HTTP {code})")).unwrap_or_default())]
    Other { status: Option<u16>, body: String },
}

pub(crate) fn error_for_status(status: StatusCode, body: String) -> ProviderError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthFailed,
        // Gemini's OpenAI endpoint answers a bad key with 400 instead of 401.
        StatusCode::BAD_REQUEST if body.contains("API key not valid") => ProviderError::AuthFailed,
        StatusCode::NOT_FOUND => ProviderError::ModelNotFound,
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited,
        _ => ProviderError::Other {
            status: Some(status.as_u16()),
            body,
        },
    }
}

pub(crate) fn error_for_transport(error: reqwest::Error) -> ProviderError {
    ProviderError::Unreachable(error.to_string())
}
