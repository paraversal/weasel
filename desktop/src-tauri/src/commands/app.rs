//! Application-level commands: identity, health, feature registry, data folder.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use weasel_core::FeatureDescriptor;

use crate::error::AppError;
use crate::state::AppState;

/// Version of the IPC surface, independent of the crate version. Bump on breaking
/// contract changes; the frontend refuses to run against an older backend.
pub const API_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub api_version: String,
    pub db_path: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppHealthDto {
    pub db_ok: bool,
    pub schema_version: Option<i64>,
    pub boot_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FeatureDescriptorDto {
    pub id: String,
    pub title: String,
    pub commands: Vec<String>,
    pub events: Vec<String>,
}

impl From<FeatureDescriptor> for FeatureDescriptorDto {
    fn from(descriptor: FeatureDescriptor) -> Self {
        Self {
            id: descriptor.id.to_owned(),
            title: descriptor.title.to_owned(),
            commands: descriptor.commands.iter().map(|c| (*c).to_owned()).collect(),
            events: descriptor.events.iter().map(|e| (*e).to_owned()).collect(),
        }
    }
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "app",
        title: "Application",
        commands: &[
            "app_info",
            "app_health",
            "app_features",
            "app_open_data_dir",
        ],
        events: &[],
    }
}

/// Identity and paths. Works even when boot failed, so a bug report can always include
/// the database path.
#[tauri::command]
#[specta::specta]
pub async fn app_info(app: AppHandle, state: State<'_, AppState>) -> Result<AppInfoDto, AppError> {
    let package = app.package_info();
    Ok(AppInfoDto {
        name: package.name.clone(),
        version: package.version.to_string(),
        tauri_version: tauri::VERSION.to_owned(),
        api_version: API_VERSION.to_owned(),
        db_path: state.paths.db_path.to_string_lossy().into_owned(),
        data_dir: state.paths.data_dir.to_string_lossy().into_owned(),
    })
}

/// Liveness of the database. Never fails: a broken database is reported as data.
#[tauri::command]
#[specta::specta]
pub async fn app_health(state: State<'_, AppState>) -> Result<AppHealthDto, AppError> {
    Ok(match &state.boot {
        Ok(services) => AppHealthDto {
            db_ok: services.store.ping().await.is_ok(),
            schema_version: services.store.schema_version().await.ok(),
            boot_error: None,
        },
        Err(err) => AppHealthDto {
            db_ok: false,
            schema_version: None,
            boot_error: Some(err.message.clone()),
        },
    })
}

#[tauri::command]
#[specta::specta]
pub async fn app_features() -> Result<Vec<FeatureDescriptorDto>, AppError> {
    Ok(crate::commands::feature_descriptors()
        .into_iter()
        .map(FeatureDescriptorDto::from)
        .collect())
}

/// Opens the data directory in the OS file manager, so "open data folder" works even
/// when the database is broken.
#[tauri::command]
#[specta::specta]
pub async fn app_open_data_dir(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    app.opener()
        .open_path(state.paths.data_dir.to_string_lossy().into_owned(), None::<String>)
        .map_err(AppError::internal)
}
