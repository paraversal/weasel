//! Runtime state managed by Tauri.
//!
//! Boot failure is a *state*, not a crash: the window still opens and `app_health`
//! reports why the database is unavailable.

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use weasel_core::campaign::CampaignService;
use weasel_core::contact::ContactService;
use weasel_core::email_check::EmailCheckService;
use weasel_core::ports::{AuditRepository, Clipboard, Clock, FileDialog};
use weasel_core::send::SendService;
use weasel_core::settings::SettingsService;
use weasel_import::service::ImportService;
use weasel_platform::DataPaths;
use weasel_store::Store;

use crate::error::AppError;
use crate::events::EventBus;

/// Canonical paths the native file picker returned during this process.
///
/// The webview may only read sources it selected, so imports reject every other path.
pub type PickedSources = Mutex<HashSet<PathBuf>>;

/// Everything that needs a working database. Built once during setup.
pub struct Services {
    pub store: Store,
    pub settings: SettingsService,
    pub contacts: ContactService,
    pub imports: ImportService,
    pub campaigns: CampaignService,
    pub sends: SendService,
    pub email_checks: EmailCheckService,
    pub audit: Arc<dyn AuditRepository>,
    // Ports are composed here once so features only add commands.
    #[allow(dead_code)] // first read by the backup command in M4
    pub clock: Arc<dyn Clock>,
    #[allow(dead_code)] // SendService owns the clipboard; this keeps the composition explicit
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
    pub picked_sources: PickedSources,
}

impl AppState {
    pub fn services(&self) -> Result<&Services, AppError> {
        self.boot
            .as_ref()
            .map_err(|err| AppError::unavailable(err.message.clone()))
    }
}
