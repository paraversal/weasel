//! Command surface. Each feature module owns its commands, DTOs and descriptor.
//!
//! The three `@gen:` markers are maintained by `pnpm gen feature`; keep their exact
//! text and placement.

use weasel_core::FeatureDescriptor;

pub mod activity;
pub mod app;
pub mod contacts;
pub mod dto;
pub mod settings;
#[cfg(test)]
pub mod testing;
// @gen:modules

/// Every feature registered in the backend. The developer page renders this list and
/// the binding-consistency test compares it with the generated TypeScript.
pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
    vec![
        activity::descriptor(),
        app::descriptor(),
        contacts::descriptor(),
        settings::descriptor(),
        // @gen:descriptors
    ]
}

/// Commands resolve to `{ status: "ok"; data } | { status: "error"; error }` so the
/// frontend has one exhaustive path instead of a `try/catch` per call site.
pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
        .typ::<dto::PageDto<app::AppInfoDto>>()
        .commands(tauri_specta::collect_commands![
            activity::activity_list,
            activity::activity_recent,
            app::app_info,
            app::app_health,
            app::app_features,
            app::app_open_data_dir,
            contacts::contacts_list,
            contacts::contacts_get,
            contacts::contacts_create,
            contacts::contacts_update,
            contacts::contacts_set_status,
            contacts::contacts_delete,
            contacts::contacts_stats,
            settings::settings_get_all,
            settings::settings_update,
            // @gen:commands
        ])
        .events(tauri_specta::collect_events![crate::events::WeaselEvent])
}
