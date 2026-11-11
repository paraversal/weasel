/// Backend description of a feature module.
///
/// The Tauri adapter exposes these through `app_features`; the frontend's own
/// manifests are the other half of the contract. Migrations are deliberately not part
/// of a feature: they are one global ordered sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    /// IPC command names, snake_case, exactly as registered.
    pub commands: &'static [&'static str],
    /// Event names emitted by this feature.
    pub events: &'static [&'static str],
}
