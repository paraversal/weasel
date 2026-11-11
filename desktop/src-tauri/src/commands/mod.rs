//! Command surface. Each feature module owns its commands, DTOs and descriptor.
//!
//! The three `@gen:` markers are maintained by `pnpm gen feature`; keep their exact
//! text and placement.

use weasel_core::FeatureDescriptor;

pub mod activity;
pub mod app;
pub mod campaigns;
pub mod contacts;
pub mod dto;
pub mod imports;
pub mod send_batches;
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
        campaigns::descriptor(),
        contacts::descriptor(),
        imports::descriptor(),
        send_batches::descriptor(),
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
            campaigns::campaigns_list,
            campaigns::campaigns_get,
            campaigns::campaigns_create,
            campaigns::campaigns_update,
            campaigns::campaigns_set_status,
            campaigns::campaigns_delete,
            campaigns::campaigns_stats,
            campaigns::campaigns_audience,
            campaigns::campaigns_prepare_send,
            contacts::contacts_list,
            contacts::contacts_get,
            contacts::contacts_create,
            contacts::contacts_update,
            contacts::contacts_set_status,
            contacts::contacts_delete,
            contacts::contacts_stats,
            imports::imports_pick_source,
            imports::imports_inspect,
            imports::imports_preview,
            imports::imports_apply,
            imports::imports_discard,
            imports::imports_list,
            imports::imports_get,
            imports::imports_plan_rows,
            send_batches::send_batches_list,
            send_batches::send_batches_get,
            send_batches::send_batches_recipients,
            send_batches::send_batches_recopy,
            send_batches::send_batches_set_note,
            settings::settings_get_all,
            settings::settings_update,
            // @gen:commands
        ])
        .events(tauri_specta::collect_events![crate::events::WeaselEvent])
}
