use reqwest::{Response, StatusCode};
use thiserror::Error;

use crate::telemetry::redact_secrets;

use super::protocol::ErrorEnvelope;

#[derive(Debug, Error)]
pub enum DeepSeekError {
    #[error(
        "DeepSeek authentication failed (401); run `komari-call login deepseek` or check the configured key"
    )]
    Authentication,
    #[error("DeepSeek account balance is insufficient (402)")]
    InsufficientBalance,
    #[error("DeepSeek rate limit reached (429); wait before trying again")]
    RateLimited,
    #[error("DeepSeek rejected the request ({status}): {message}")]
    InvalidRequest { status: u16, message: String },
    #[error("DeepSeek service error ({status}): {message}")]
    Server { status: u16, message: String },
    #[error("DeepSeek HTTP error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("DeepSeek transport error: {0}")]
    Transport(String),
    #[error("DeepSeek stream protocol error: {0}")]
    Protocol(String),
}

pub async fn classify_response(response: Response, api_key: &str) -> DeepSeekError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    classify_status(status, &body, api_key)
}

pub fn classify_status(status: StatusCode, body: &str, api_key: &str) -> DeepSeekError {
    let message = provider_message(body, api_key);
    match status.as_u16() {
        400 | 422 => DeepSeekError::InvalidRequest {
            status: status.as_u16(),
            message,
        },
        401 => DeepSeekError::Authentication,
        402 => DeepSeekError::InsufficientBalance,
        429 => DeepSeekError::RateLimited,
        500..=599 => DeepSeekError::Server {
            status: status.as_u16(),
            message,
        },
        _ => DeepSeekError::Http {
            status: status.as_u16(),
            message,
        },
    }
}

pub fn transport(error: reqwest::Error, api_key: &str) -> DeepSeekError {
    DeepSeekError::Transport(redact_secrets(&error.to_string(), &[api_key]))
}

fn provider_message(body: &str, api_key: &str) -> String {
    let parsed = serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error)
        .and_then(|error| error.message)
        .unwrap_or_else(|| "request failed without a provider message".into());
    let redacted = redact_secrets(&parsed, &[api_key]);
    redacted.chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_auth_rate_limit_and_server_errors() {
        assert!(matches!(
            classify_status(StatusCode::UNAUTHORIZED, "{}", "secret"),
            DeepSeekError::Authentication
        ));
        assert!(matches!(
            classify_status(StatusCode::TOO_MANY_REQUESTS, "{}", "secret"),
            DeepSeekError::RateLimited
        ));
        assert!(matches!(
            classify_status(StatusCode::SERVICE_UNAVAILABLE, "{}", "secret"),
            DeepSeekError::Server { status: 503, .. }
        ));
    }

    #[test]
    fn redacts_key_from_provider_message() {
        let error = classify_status(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"bad key sk-private"}}"#,
            "sk-private",
        );
        let rendered = error.to_string();
        assert!(!rendered.contains("sk-private"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
