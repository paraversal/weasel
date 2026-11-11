//! Command surface. Each feature module owns its commands, DTOs and descriptor.

use weasel_core::FeatureDescriptor;

// @gen:modules

/// Every feature registered in the backend. The developer page renders this list and
/// the binding-consistency test compares it with the generated TypeScript.
#[allow(dead_code)] // consumed by `app_features` in M5
pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
    vec![
        // @gen:descriptors
    ]
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().events(tauri_specta::collect_events![
        crate::events::WeaselEvent
    ])
}
