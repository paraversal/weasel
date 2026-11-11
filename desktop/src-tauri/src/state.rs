//! Runtime state managed by Tauri.
//!
//! Boot failure is a *state*, not a crash: the window still opens and `app_health`
//! reports why the database is unavailable.
//!
//! Only the command layer reads this state; the boot path just installs it.

#![allow(dead_code)] // removed once the command layer lands

use std::sync::Arc;

use weasel_core::ports::{Clipboard, Clock, FileDialog};
use weasel_core::settings::SettingsService;
use weasel_platform::DataPaths;
use weasel_store::Store;

use crate::error::AppError;
use crate::events::EventBus;

/// Everything that needs a working database. Built once during setup.
pub struct Services {
    pub store: Store,
    pub settings: SettingsService,
    pub clock: Arc<dyn Clock>,
    pub clipboard: Arc<dyn Clipboard>,
    pub dialogs: Arc<dyn FileDialog>,
}

#[derive(Debug, Clone)]
pub struct BootError {
    pub message: String,
}

pub struct AppState {
    pub paths: DataPaths,
    pub boot: Result<Services, BootError>,
    pub events: EventBus,
}

impl AppState {
    pub fn services(&self) -> Result<&Services, AppError> {
        self.boot
            .as_ref()
            .map_err(|err| AppError::unavailable(err.message.clone()))
    }
}
