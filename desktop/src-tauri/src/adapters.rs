//! Tauri implementations of the core ports.
//!
//! Plugin errors never escape: they become `CoreError::Io`, so the UI can offer a
//! retry without knowing which plugin failed.

use std::path::PathBuf;

use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use weasel_core::CoreError;
use weasel_core::ports::{Clipboard, FileDialog, FileFilter};

pub struct TauriClipboard {
    app: AppHandle,
}

impl TauriClipboard {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl Clipboard for TauriClipboard {
    async fn write_text(&self, text: String) -> weasel_core::CoreResult<()> {
        self.app
            .clipboard()
            .write_text(text)
            .map_err(CoreError::io)
    }
}

pub struct TauriFileDialog {
    app: AppHandle,
}

impl TauriFileDialog {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl FileDialog for TauriFileDialog {
    async fn pick_file(&self, filters: &[FileFilter]) -> weasel_core::CoreResult<Option<PathBuf>> {
        let app = self.app.clone();
        let filters = filters.to_vec();
        // The dialog API blocks the calling thread; keep it off the async runtime.
        tokio::task::spawn_blocking(move || {
            let mut dialog = app.dialog().file();
            for filter in &filters {
                let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
                dialog = dialog.add_filter(&filter.name, &extensions);
            }
            match dialog.blocking_pick_file() {
                Some(file) => file.into_path().map(Some).map_err(CoreError::io),
                None => Ok(None),
            }
        })
        .await
        .map_err(CoreError::internal)?
    }
}
