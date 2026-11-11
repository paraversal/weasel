//! Command surface. Each feature module owns its commands, DTOs and descriptor.
//!
//! The three `@gen:` markers are maintained by `pnpm gen feature`; keep their exact
//! text and placement.

use weasel_core::FeatureDescriptor;

pub mod app;
pub mod settings;
// @gen:modules

/// Every feature registered in the backend. The developer page renders this list and
/// the binding-consistency test compares it with the generated TypeScript.
pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
    vec![
        app::descriptor(),
        settings::descriptor(),
        // @gen:descriptors
    ]
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            app::app_info,
            app::app_health,
            app::app_features,
            app::app_open_data_dir,
            settings::settings_get_all,
            settings::settings_update,
            // @gen:commands
        ])
        .events(tauri_specta::collect_events![crate::events::WeaselEvent])
}
