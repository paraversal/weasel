use std::fmt;

/// The single error type every application service returns.
///
/// Adapters convert their own errors into this type, so the Tauri layer only maps
/// `CoreError` to its transport error.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid {field}: {reason}")]
    Validation { field: &'static str, reason: String },
    #[error("{entity} {id} not found")]
    NotFound { entity: &'static str, id: String },
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    InvalidState(String),
    #[error("i/o: {0}")]
    Io(String),
    #[error(transparent)]
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

impl CoreError {
    pub fn validation(field: &'static str, reason: impl Into<String>) -> Self {
        Self::Validation {
            field,
            reason: reason.into(),
        }
    }

    pub fn io(err: impl fmt::Display) -> Self {
        Self::Io(err.to_string())
    }

    pub fn internal(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Internal(Box::new(err))
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
