//! Shared test doubles. Not a dependency of shipped code.
//!
//! Services take `Arc<dyn Port>`, so these fakes work for any crate that depends on
//! `weasel-core`.

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};

use async_trait::async_trait;
use weasel_core::{
    CoreError, CoreResult,
    ports::{Clipboard, Clock, FileDialog, FileFilter},
};
use weasel_store::{OpenOptions, Store, open};

/// A migrated SQLite database in a temporary directory, deleted on drop.
pub struct TempDb {
    dir: tempfile::TempDir,
    store: Store,
}

impl TempDb {
    pub async fn new() -> Self {
        let dir = match tempfile::TempDir::new() {
            Ok(dir) => dir,
            Err(err) => panic!("temp dir: {err}"),
        };
        let db_path = dir.path().join("weasel.sqlite3");
        let backups_dir = dir.path().join("backups");
        let (store, _report) = match open(OpenOptions {
            db_path: &db_path,
            backups_dir: &backups_dir,
        })
        .await
        {
            Ok(pair) => pair,
            Err(err) => panic!("temp db: {err}"),
        };
        Self { dir, store }
    }

    /// Handle over the pool; cheap to clone.
    pub fn store(&self) -> Store {
        self.store.clone()
    }

    pub fn path(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }
}

/// Manually advanced clock.
#[derive(Debug)]
pub struct FixedClock {
    now_ms: AtomicI64,
}

impl FixedClock {
    pub fn new(now_ms: i64) -> Self {
        Self {
            now_ms: AtomicI64::new(now_ms),
        }
    }

    pub fn set(&self, now_ms: i64) {
        self.now_ms.store(now_ms, Ordering::SeqCst);
    }

    pub fn advance(&self, millis: i64) {
        self.now_ms.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

/// Clipboard that records writes and can fail the next write.
#[derive(Debug, Default)]
pub struct FakeClipboard {
    writes: Mutex<Vec<String>>,
    fail_next: AtomicBool,
}

impl FakeClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }

    pub fn writes(&self) -> Vec<String> {
        self.writes.lock().map(|w| w.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Clipboard for FakeClipboard {
    async fn write_text(&self, text: String) -> CoreResult<()> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(CoreError::InvalidState("clipboard unavailable".to_owned()));
        }
        self.writes
            .lock()
            .map_err(|_| CoreError::InvalidState("poisoned".to_owned()))?
            .push(text);
        Ok(())
    }
}

/// File dialog replaying a scripted queue of answers.
#[derive(Debug, Default)]
pub struct FakeFileDialog {
    answers: Mutex<VecDeque<Option<PathBuf>>>,
    seen_filters: Mutex<Vec<Vec<FileFilter>>>,
}

impl FakeFileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a selection.
    pub fn push(&self, path: impl Into<PathBuf>) {
        if let Ok(mut answers) = self.answers.lock() {
            answers.push_back(Some(path.into()));
        }
    }

    /// Queue a user cancellation.
    pub fn push_cancel(&self) {
        if let Ok(mut answers) = self.answers.lock() {
            answers.push_back(None);
        }
    }

    pub fn seen_filters(&self) -> Vec<Vec<FileFilter>> {
        self.seen_filters
            .lock()
            .map(|f| f.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl FileDialog for FakeFileDialog {
    async fn pick_file(&self, filters: &[FileFilter]) -> CoreResult<Option<PathBuf>> {
        if let Ok(mut seen) = self.seen_filters.lock() {
            seen.push(filters.to_vec());
        }
        let answer = self
            .answers
            .lock()
            .map_err(|_| CoreError::InvalidState("poisoned".to_owned()))?
            .pop_front();
        Ok(answer.flatten())
    }
}
