//! Transport error: the only error shape that crosses the IPC boundary.
//!
//! `CoreError` is mapped here once, so command implementations only ever return
//! `AppError` and the frontend only ever handles `AppError`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use specta::Type;
use weasel_core::CoreError;

/// Machine-readable error class. The frontend switches on this, never on `message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Validation,
    NotFound,
    Conflict,
    InvalidState,
    Io,
    /// Reserved by the import plan; part of the published wire contract.
    Import,
    #[allow(dead_code)]
    Cancelled,
    Internal,
}

/// What a rejected `invoke` carries.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    /// Safe for display. Never contains SQL, file contents or personally identifying data.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub retryable: bool,
}

impl AppError {
    /// Boot/setup failure: no database, so every data command fails this way.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidState,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    /// Unexpected failure. The full error is logged under a short reference that is
    /// echoed to the user, so a bug report maps to exactly one log line.
    pub fn internal(err: impl std::fmt::Display) -> Self {
        let log_id = next_log_id();
        tracing::error!(
            log_id = %log_id,
            error = %redact_emails(&err.to_string()),
            "internal error"
        );
        Self {
            code: ErrorCode::Internal,
            message: format!("Unexpected error (ref {log_id})"),
            details: Some(serde_json::json!({ "logId": log_id })),
            retryable: false,
        }
    }
}

impl From<CoreError> for AppError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::Validation { field, reason } => Self {
                code: ErrorCode::Validation,
                message: format!("{field}: {reason}"),
                details: Some(serde_json::json!({ "field": field, "reason": reason })),
                retryable: false,
            },
            CoreError::NotFound { entity, id } => Self {
                code: ErrorCode::NotFound,
                message: format!("{entity} not found"),
                details: Some(serde_json::json!({ "entity": entity, "id": id })),
                retryable: false,
            },
            CoreError::Conflict(message) => Self {
                code: ErrorCode::Conflict,
                message,
                details: None,
                retryable: false,
            },
            CoreError::InvalidState(message) => Self {
                code: ErrorCode::InvalidState,
                message,
                details: None,
                retryable: false,
            },
            CoreError::Io(message) => Self {
                code: ErrorCode::Io,
                message,
                details: None,
                retryable: true,
            },
            CoreError::Import { row, reason } => Self {
                code: ErrorCode::Import,
                message: reason,
                details: Some(serde_json::json!({ "row": row })),
                retryable: false,
            },
            CoreError::Internal(source) => Self::internal(source),
        }
    }
}

static LOG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_log_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let seq = LOG_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}", base36(now), base36(seq))
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Replace the local part of anything that looks like an email address before logging.
///
/// Deliberately regex-free: whitespace-delimited tokens with exactly one `@` and a dot
/// in the domain are masked. Addresses are the main personal data this app handles.
pub fn redact_emails(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut start = 0;
    for (index, ch) in input.char_indices() {
        if ch.is_whitespace() {
            push_redacted(&input[start..index], &mut out);
            out.push(ch);
            start = index + ch.len_utf8();
        }
    }
    push_redacted(&input[start..], &mut out);
    out
}

fn push_redacted(token: &str, out: &mut String) {
    if let Some(at) = token.find('@') {
        let local = &token[..at];
        let domain = &token[at + 1..];
        if !local.is_empty() && domain.contains('.') && !domain.contains('@') {
            out.push_str("***@");
            out.push_str(domain);
            return;
        }
    }
    out.push_str(token);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn redacts_only_addresses() {
        assert_eq!(
            redact_emails("send to alice@example.com failed"),
            "send to ***@example.com failed"
        );
        assert_eq!(redact_emails("no addresses here"), "no addresses here");
        assert_eq!(redact_emails("a@b"), "a@b");
    }

    #[test]
    fn validation_maps_field_details() {
        let error = AppError::from(CoreError::validation("theme", "unknown value"));
        assert_eq!(error.code, ErrorCode::Validation);
        assert!(!error.retryable);
        assert_eq!(
            error.details,
            Some(serde_json::json!({ "field": "theme", "reason": "unknown value" }))
        );
    }

    #[test]
    fn every_core_error_maps_to_the_documented_row() {
        let cases = [
            (
                CoreError::validation("theme", "unknown value"),
                ErrorCode::Validation,
                false,
            ),
            (
                CoreError::NotFound {
                    entity: "contact",
                    id: "42".to_string(),
                },
                ErrorCode::NotFound,
                false,
            ),
            (
                CoreError::Conflict("already sent".to_string()),
                ErrorCode::Conflict,
                false,
            ),
            (
                CoreError::InvalidState("no campaign".to_string()),
                ErrorCode::InvalidState,
                false,
            ),
            (CoreError::io("disk full"), ErrorCode::Io, true),
            (
                CoreError::Import {
                    row: Some(7),
                    reason: "email is empty".to_owned(),
                },
                ErrorCode::Import,
                false,
            ),
            (
                CoreError::Internal(Box::new(std::io::Error::other("boom"))),
                ErrorCode::Internal,
                false,
            ),
        ];
        for (core, code, retryable) in cases {
            let mapped = AppError::from(core);
            assert_eq!(mapped.code, code);
            assert_eq!(mapped.retryable, retryable);
        }
    }

    #[test]
    fn internal_errors_carry_a_log_id_and_hide_the_cause() {
        let mapped = AppError::from(CoreError::Internal(Box::new(std::io::Error::other(
            "secret detail",
        ))));
        let details = mapped.details.expect("internal errors carry details");
        assert!(details.get("logId").and_then(|id| id.as_str()).is_some());
        assert!(!mapped.message.contains("secret detail"));
    }

    #[test]
    fn io_is_retryable() {
        let error = AppError::from(CoreError::io("disk full"));
        assert_eq!(error.code, ErrorCode::Io);
        assert!(error.retryable);
    }
}
