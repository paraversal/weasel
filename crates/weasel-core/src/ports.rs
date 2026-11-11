use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::CoreResult;

/// Wall-clock access. Production uses the OS clock; tests use a fixed clock.
pub trait Clock: Send + Sync {
    /// Unix milliseconds in UTC.
    fn now_ms(&self) -> i64;
}

/// Write-only clipboard access. Sending a campaign copies addresses here.
#[async_trait]
pub trait Clipboard: Send + Sync {
    async fn write_text(&self, text: String) -> CoreResult<()>;
}

/// Filter shown in a native open-file dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Native file picker. Implementations must not block the async runtime.
#[async_trait]
pub trait FileDialog: Send + Sync {
    async fn pick_file(&self, filters: &[FileFilter]) -> CoreResult<Option<PathBuf>>;
}

/// Persistence port for application settings.
///
/// `save` must write all entries in one transaction: either every entry is stored or
/// none is.
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> CoreResult<Vec<(String, Value)>>;
    async fn save(&self, entries: Vec<(String, Value)>, now_ms: i64) -> CoreResult<()>;
}
