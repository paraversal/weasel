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
        match err {
            StoreError::Sqlx(sqlx::Error::RowNotFound) => CoreError::NotFound {
                entity: "record",
                id: "unknown".to_owned(),
            },
            StoreError::Sqlx(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                CoreError::Conflict(db.message().to_owned())
            }
            StoreError::NewerSchema { db, app } => CoreError::InvalidState(format!(
                "database schema v{db} is newer than this app supports (v{app})"
            )),
            other => CoreError::Internal(Box::new(other)),
        }
    }
}
