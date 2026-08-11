use reqwest::{Response, StatusCode};
use thiserror::Error;

use crate::{provider::openai_compatible::protocol::ErrorEnvelope, telemetry::redact_secrets};

#[derive(Debug, Error)]
pub enum OpenCodeGoError {
    #[error(
        "OpenCode Go authentication failed (401); run `komari-call login opencode-go` or check the configured key"
    )]
    Authentication,
    #[error("OpenCode Go subscription quota or balance is insufficient (402)")]
    InsufficientBalance,
    #[error("OpenCode Go rate limit reached (429); wait before trying again")]
    RateLimited,
    #[error("OpenCode Go rejected the request ({status}): {message}")]
    InvalidRequest { status: u16, message: String },
    #[error("OpenCode Go service error ({status}): {message}")]
    Server { status: u16, message: String },
    #[error("OpenCode Go HTTP error ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("OpenCode Go transport error: {0}")]
    Transport(String),
}

pub async fn classify_response(response: Response, api_key: &str) -> OpenCodeGoError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    classify_status(status, &body, api_key)
}

pub fn classify_status(status: StatusCode, body: &str, api_key: &str) -> OpenCodeGoError {
    let message = provider_message(body, api_key);
    match status.as_u16() {
        400 | 422 => OpenCodeGoError::InvalidRequest {
            status: status.as_u16(),
            message,
        },
        401 => OpenCodeGoError::Authentication,
        402 => OpenCodeGoError::InsufficientBalance,
        429 => OpenCodeGoError::RateLimited,
        500..=599 => OpenCodeGoError::Server {
            status: status.as_u16(),
            message,
        },
        _ => OpenCodeGoError::Http {
            status: status.as_u16(),
            message,
        },
    }
}

pub fn transport(error: reqwest::Error, api_key: &str) -> OpenCodeGoError {
    OpenCodeGoError::Transport(redact_secrets(&error.to_string(), &[api_key]))
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
    fn classifies_auth_balance_rate_limit_and_server_errors() {
        assert!(matches!(
            classify_status(StatusCode::UNAUTHORIZED, "{}", "secret"),
            OpenCodeGoError::Authentication
        ));
        assert!(matches!(
            classify_status(StatusCode::PAYMENT_REQUIRED, "{}", "secret"),
            OpenCodeGoError::InsufficientBalance
        ));
        assert!(matches!(
            classify_status(StatusCode::TOO_MANY_REQUESTS, "{}", "secret"),
            OpenCodeGoError::RateLimited
        ));
        assert!(matches!(
            classify_status(StatusCode::SERVICE_UNAVAILABLE, "{}", "secret"),
            OpenCodeGoError::Server { status: 503, .. }
        ));
    }

    #[test]
    fn redacts_and_limits_provider_messages() {
        let body = format!(
            "{{\"error\":{{\"message\":\"bad key sk-private {}\"}}}}",
            "x".repeat(500)
        );
        let rendered = classify_status(StatusCode::BAD_REQUEST, &body, "sk-private").to_string();
        assert!(!rendered.contains("sk-private"));
        assert!(rendered.contains("[REDACTED]"));
        let message = rendered.split_once(": ").unwrap().1;
        assert_eq!(message.chars().count(), 400);
    }
}
