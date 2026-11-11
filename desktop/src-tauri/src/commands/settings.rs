//! Settings commands and their DTOs. Domain types never derive `specta::Type`; the
//! conversions live here so the wire shape can drift from the domain deliberately.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::FeatureDescriptor;
use weasel_core::settings::{Settings, SettingsPatch, SettingsService, Theme};

use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ThemeDto {
    #[default]
    System,
    Light,
    Dark,
}

impl From<Theme> for ThemeDto {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::System => Self::System,
            Theme::Light => Self::Light,
            Theme::Dark => Self::Dark,
        }
    }
}

impl From<ThemeDto> for Theme {
    fn from(theme: ThemeDto) -> Self {
        match theme {
            ThemeDto::System => Self::System,
            ThemeDto::Light => Self::Light,
            ThemeDto::Dark => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: ThemeDto,
    pub feature_flags: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatchDto {
    pub theme: Option<ThemeDto>,
    pub feature_flags: Option<BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateRequest {
    pub patch: SettingsPatchDto,
}

impl From<Settings> for SettingsDto {
    fn from(settings: Settings) -> Self {
        Self {
            theme: settings.theme.into(),
            feature_flags: settings.feature_flags,
        }
    }
}

impl From<SettingsPatchDto> for SettingsPatch {
    fn from(patch: SettingsPatchDto) -> Self {
        Self {
            theme: patch.theme.map(Into::into),
            feature_flags: patch.feature_flags,
        }
    }
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "settings",
        title: "Settings",
        commands: &["settings_get_all", "settings_update"],
        events: &["settingsChanged"],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn settings_get_all(state: State<'_, AppState>) -> Result<SettingsDto, AppError> {
    let services = state.services()?;
    load(&services.settings).await
}

#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SettingsUpdateRequest,
) -> Result<SettingsDto, AppError> {
    let services = state.services()?;
    let (settings, changed) = apply(&services.settings, request).await?;
    if !changed.is_empty() {
        state.events.emit(
            &app,
            DomainEvent::SettingsChanged {
                keys: changed.iter().map(|key| (*key).to_owned()).collect(),
            },
        );
    }
    Ok(settings)
}

async fn load(settings: &SettingsService) -> Result<SettingsDto, AppError> {
    Ok(SettingsDto::from(settings.get().await?))
}

/// Pure command body: no `State`, no `AppHandle`, so it is testable against `TempDb`.
async fn apply(
    settings: &SettingsService,
    request: SettingsUpdateRequest,
) -> Result<(SettingsDto, Vec<&'static str>), AppError> {
    let (next, changed) = settings.update(request.patch.into()).await?;
    Ok((SettingsDto::from(next), changed))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use weasel_core::ports::Clock;
    use weasel_core::settings::{KEY_FEATURE_FLAGS, KEY_THEME};
    use weasel_store::SqliteSettings;
    use weasel_testkit::{FixedClock, TempDb};

    use super::*;

    async fn service() -> (TempDb, SettingsService, Arc<dyn Clock>) {
        let db = TempDb::new().await;
        let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(1_700_000_000_000));
        let settings = SettingsService::new(
            Arc::new(SqliteSettings::new(db.store())),
            Arc::clone(&clock),
        );
        (db, settings, clock)
    }

    fn update(theme: Option<ThemeDto>) -> SettingsUpdateRequest {
        SettingsUpdateRequest {
            patch: SettingsPatchDto {
                theme,
                feature_flags: None,
            },
        }
    }

    #[tokio::test]
    async fn update_then_get_reflects_the_change() {
        let (_db, settings, _clock) = service().await;

        assert_eq!(load(&settings).await.unwrap().theme, ThemeDto::System);

        let (next, changed) = apply(&settings, update(Some(ThemeDto::Dark)))
            .await
            .unwrap();
        assert_eq!(next.theme, ThemeDto::Dark);
        assert_eq!(changed, vec![KEY_THEME]);
        assert_eq!(load(&settings).await.unwrap().theme, ThemeDto::Dark);
    }

    #[tokio::test]
    async fn unchanged_update_reports_no_keys() {
        let (_db, settings, _clock) = service().await;
        apply(&settings, update(Some(ThemeDto::Light)))
            .await
            .unwrap();
        let (_next, changed) = apply(&settings, update(Some(ThemeDto::Light)))
            .await
            .unwrap();
        assert!(changed.is_empty());
    }

    #[tokio::test]
    async fn invalid_flag_id_is_rejected_before_touching_storage() {
        let (_db, settings, _clock) = service().await;
        let request = SettingsUpdateRequest {
            patch: SettingsPatchDto {
                theme: None,
                feature_flags: Some(BTreeMap::from([("Bad Id".to_string(), true)])),
            },
        };
        let error = apply(&settings, request).await.unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Validation);
        assert!(load(&settings).await.unwrap().feature_flags.is_empty());
        assert_eq!(KEY_FEATURE_FLAGS, "featureFlags");
    }
}
