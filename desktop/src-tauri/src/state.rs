//! Runtime state managed by Tauri.
//!
//! Boot failure is a *state*, not a crash: the window still opens and `app_health`
//! reports why the database is unavailable.

use std::sync::Arc;

use weasel_core::campaign::CampaignService;
use weasel_core::contact::ContactService;
use weasel_core::ports::{AuditRepository, Clipboard, Clock, FileDialog};
use weasel_core::send::SendService;
use weasel_core::settings::SettingsService;
use weasel_import::service::ImportService;
use weasel_platform::DataPaths;
use weasel_store::Store;

use crate::error::AppError;
use crate::events::EventBus;

/// Everything that needs a working database. Built once during setup.
pub struct Services {
    pub store: Store,
    pub settings: SettingsService,
    pub contacts: ContactService,
    pub imports: ImportService,
    // TODO(04-exec): commands start consuming these.
    #[allow(dead_code)]
    pub campaigns: CampaignService,
    #[allow(dead_code)]
    pub sends: SendService,
    #[allow(dead_code)]
    pub audit: Arc<dyn AuditRepository>,
    // Ports are composed here once so features only add commands.
    #[allow(dead_code)]
    pub clock: Arc<dyn Clock>,
    #[allow(dead_code)]
    pub clipboard: Arc<dyn Clipboard>,
    #[allow(dead_code)]
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
