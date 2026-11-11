use weasel_core::CoreError;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("database schema v{db} is newer than this app supports (v{app})")]
    NewerSchema { db: i64, app: i64 },
    #[error("i/o: {0}")]
    Io(String),
}

impl StoreError {
    pub fn io(err: impl std::fmt::Display) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<StoreError> for CoreError {
    fn from(err: StoreError) -> Self {
        match &err {
            StoreError::Sqlx(sqlx::Error::RowNotFound) => CoreError::NotFound {
                entity: "record",
                id: "unknown".to_owned(),
            },
            StoreError::Sqlx(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                unique_conflict(db.message())
            }
            StoreError::NewerSchema { db, app } => CoreError::InvalidState(format!(
                "database schema v{db} is newer than this app supports (v{app})"
            )),
            _ => CoreError::Internal(Box::new(err)),
        }
    }
}

/// SQLite reports the violated index in the message. Only the email conflict is
/// actionable by a person; the rest mean the caller skipped a lookup.
fn unique_conflict(message: &str) -> CoreError {
    if message.contains("ux_contacts_email_live") || message.contains("contacts.email_normalized") {
        CoreError::Conflict("Another contact already uses this email".to_owned())
    } else if message.contains("ux_email_history_open") {
        CoreError::Conflict("This contact already has an open email history row".to_owned())
    } else {
        CoreError::Conflict("A record with these values already exists".to_owned())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn email_uniqueness_is_explained() {
        let err = unique_conflict(
            "UNIQUE constraint failed: contacts.email_normalized (code 2067 SQLITE_CONSTRAINT_UNIQUE)",
        );
        assert!(matches!(err, CoreError::Conflict(message) if message.contains("email")));
        let err = unique_conflict("UNIQUE constraint failed: send_records.batch_id");
        assert!(matches!(err, CoreError::Conflict(message) if message.contains("already exists")));
    }
}
