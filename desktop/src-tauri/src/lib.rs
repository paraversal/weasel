//! Library entry point: plugin composition, boot sequence, command wiring.

mod adapters;
mod bindings;
mod commands;
mod error;
mod events;
mod state;

pub use bindings::export as export_bindings;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
use weasel_core::ports::{Clock, SettingsRepository};
use weasel_core::settings::SettingsService;
use weasel_platform::{DataPaths, SystemClock};
use weasel_store::{OpenOptions, SqliteSettings};

use crate::adapters::{TauriClipboard, TauriFileDialog};
use crate::events::EventBus;
use crate::state::{AppState, BootError, Services};

pub fn run() {
    let ipc = commands::builder();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            let paths = resolve_paths(app.handle());
            // Registered here rather than on the builder: the file target must point at
            // the resolved data directory, which needs `app.path()`.
            app.handle().plugin(log_plugin(&paths.logs_dir))?;
            ipc.mount_events(app);
            boot(app.handle(), paths);
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // No logger is guaranteed this late, so this is the only unconditional output.
        eprintln!("weasel: failed to start: {err}");
        std::process::exit(1);
    }
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn log_plugin(logs_dir: &std::path::Path) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Folder {
                path: logs_dir.to_path_buf(),
                file_name: Some("weasel".to_string()),
            }),
        ])
        .max_file_size(5_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .build()
}

/// Opens the database and composes the services. Never panics and never aborts startup:
/// a broken database leaves the app usable for diagnostics.
fn boot(app: &AppHandle, paths: DataPaths) {
    if let Err(err) = paths.ensure() {
        tracing::error!(error = %err, dir = %paths.data_dir.display(), "could not create data directories");
    }
    tracing::info!(data_dir = %paths.data_dir.display(), db = %paths.db_path.display(), "data paths resolved");

    let boot = open_services(app, &paths);
    app.manage(AppState {
        paths,
        boot,
        events: EventBus::new(),
    });
}

fn resolve_paths(app: &AppHandle) -> DataPaths {
    let app_data = app.path().app_data_dir().unwrap_or_else(|err| {
        tracing::warn!(error = %err, "no app data dir; falling back to the temp dir");
        std::env::temp_dir().join("weasel")
    });
    let override_dir = std::env::var_os("WEASEL_DATA_DIR").map(PathBuf::from);
    if override_dir.is_some() {
        tracing::info!("WEASEL_DATA_DIR overrides the app data dir");
    }
    DataPaths::resolve(&app_data, override_dir.as_deref(), cfg!(debug_assertions))
}

fn open_services(app: &AppHandle, paths: &DataPaths) -> Result<Services, BootError> {
    let opened = tauri::async_runtime::block_on(weasel_store::open(OpenOptions {
        db_path: &paths.db_path,
        backups_dir: &paths.backups_dir,
    }));

    let (store, report) = match opened {
        Ok(opened) => opened,
        Err(err) => {
            let message = err.to_string();
            tracing::error!(
                error = %error::redact_emails(&message),
                db = %paths.db_path.display(),
                "database failed to open"
            );
            return Err(BootError { message });
        }
    };

    tracing::info!(
        schema_version = report.schema_version,
        created = report.created,
        backup = ?report.backup,
        "database ready"
    );

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let settings = SettingsService::new(
        Arc::new(SqliteSettings::new(store.clone())) as Arc<dyn SettingsRepository>,
        Arc::clone(&clock),
    );

    Ok(Services {
        store,
        settings,
        clock,
        clipboard: Arc::new(TauriClipboard::new(app.clone())),
        dialogs: Arc::new(TauriFileDialog::new(app.clone())),
    })
}
