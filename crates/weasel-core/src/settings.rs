//! The application settings service: the template every later service copies.
//!
//! Storage is a flat key/value table; each key holds a JSON value. Unknown keys are
//! ignored, missing keys fall back to defaults, and a stored value that fails to
//! deserialize falls back to its default with a warning — reading settings must never
//! take the app down.

use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::{CoreError, CoreResult},
    ports::{Clock, SettingsRepository},
};

/// Storage key for the theme.
pub const KEY_THEME: &str = "theme";
/// Storage key for the feature-flag map.
pub const KEY_FEATURE_FLAGS: &str = "featureFlags";
/// Storage key for what the UI calls the people in the address book.
pub const KEY_AUDIENCE_TERM: &str = "audienceTerm";
/// Longest accepted audience term, per form.
pub const AUDIENCE_TERM_MAX: usize = 40;

/// Colour scheme selection. `System` follows the OS preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// The word the UI uses for one person and for many ("freshie" / "freshies").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceTerm {
    pub singular: String,
    pub plural: String,
}

impl Default for AudienceTerm {
    fn default() -> Self {
        Self {
            singular: "freshie".to_owned(),
            plural: "freshies".to_owned(),
        }
    }
}

impl AudienceTerm {
    /// Trimmed copy, or a `Validation` error for an empty or overlong form.
    pub fn cleaned(&self) -> CoreResult<Self> {
        let clean = |raw: &str| -> CoreResult<String> {
            let value = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                return Err(CoreError::validation("audienceTerm", "must not be empty"));
            }
            if value.chars().count() > AUDIENCE_TERM_MAX {
                return Err(CoreError::validation(
                    "audienceTerm",
                    format!("must be at most {AUDIENCE_TERM_MAX} characters"),
                ));
            }
            Ok(value)
        };
        Ok(Self {
            singular: clean(&self.singular)?,
            plural: clean(&self.plural)?,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub feature_flags: BTreeMap<String, bool>,
    pub audience_term: AudienceTerm,
}

/// A partial update. Absent fields keep their stored value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub theme: Option<Theme>,
    pub feature_flags: Option<BTreeMap<String, bool>>,
    pub audience_term: Option<AudienceTerm>,
}

impl SettingsPatch {
    /// Reject flag ids that are not `[a-z0-9-]{1,64}`.
    pub fn validate(&self) -> CoreResult<()> {
        if let Some(flags) = &self.feature_flags {
            for id in flags.keys() {
                if !is_valid_flag_id(id) {
                    return Err(CoreError::validation(
                        "featureFlags",
                        format!("invalid flag id {id:?}: expected [a-z0-9-]{{1,64}}"),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn is_valid_flag_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Clone)]
pub struct SettingsService {
    repo: Arc<dyn SettingsRepository>,
    clock: Arc<dyn Clock>,
}

impl SettingsService {
    pub fn new(repo: Arc<dyn SettingsRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn get(&self) -> CoreResult<Settings> {
        Ok(decode(self.repo.load().await?))
    }

    /// Apply `patch`, persist only changed keys, and report which keys changed.
    pub async fn update(&self, patch: SettingsPatch) -> CoreResult<(Settings, Vec<&'static str>)> {
        patch.validate()?;
        let current = self.get().await?;
        let mut next = current.clone();
        let mut changed: Vec<&'static str> = Vec::new();

        if let Some(theme) = patch.theme
            && next.theme != theme
        {
            next.theme = theme;
            changed.push(KEY_THEME);
        }

        if let Some(term) = patch.audience_term {
            let term = term.cleaned()?;
            if next.audience_term != term {
                next.audience_term = term;
                changed.push(KEY_AUDIENCE_TERM);
            }
        }

        if let Some(flags) = patch.feature_flags {
            let mut merged = next.feature_flags.clone();
            merge_flags(&mut merged, flags);
            if merged != next.feature_flags {
                next.feature_flags = merged;
                changed.push(KEY_FEATURE_FLAGS);
            }
        }

        if changed.is_empty() {
            return Ok((current, changed));
        }

        let mut entries = Vec::with_capacity(changed.len());
        for key in &changed {
            let value = match *key {
                KEY_THEME => to_value(&next.theme)?,
                KEY_FEATURE_FLAGS => to_value(&next.feature_flags)?,
                KEY_AUDIENCE_TERM => to_value(&next.audience_term)?,
                other => {
                    return Err(CoreError::internal(std::io::Error::other(format!(
                        "unhandled settings key {other}"
                    ))));
                }
            };
            entries.push(((*key).to_owned(), value));
        }
        self.repo.save(entries, self.clock.now_ms()).await?;
        Ok((next, changed))
    }
}

fn merge_flags(into: &mut BTreeMap<String, bool>, patch: BTreeMap<String, bool>) {
    for (key, value) in patch {
        into.insert(key, value);
    }
}

fn to_value<T: Serialize>(value: &T) -> CoreResult<Value> {
    serde_json::to_value(value).map_err(CoreError::internal)
}

fn decode(entries: Vec<(String, Value)>) -> Settings {
    let mut settings = Settings::default();
    for (key, value) in entries {
        match key.as_str() {
            KEY_THEME => match serde_json::from_value(value) {
                Ok(theme) => settings.theme = theme,
                Err(err) => tracing::warn!(%err, "ignoring malformed setting `theme`"),
            },
            KEY_FEATURE_FLAGS => match serde_json::from_value(value) {
                Ok(flags) => settings.feature_flags = flags,
                Err(err) => tracing::warn!(%err, "ignoring malformed setting `featureFlags`"),
            },
            KEY_AUDIENCE_TERM => match serde_json::from_value::<AudienceTerm>(value)
                .map_err(|err| err.to_string())
                .and_then(|term| term.cleaned().map_err(|err| err.to_string()))
            {
                Ok(term) => settings.audience_term = term,
                Err(err) => tracing::warn!(%err, "ignoring malformed setting `audienceTerm`"),
            },
            other => tracing::debug!(key = other, "ignoring unknown setting"),
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeRepo {
        rows: Mutex<Vec<(String, Value)>>,
        saves: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for FakeRepo {
        async fn load(&self) -> CoreResult<Vec<(String, Value)>> {
            Ok(self.rows.lock().unwrap().clone())
        }

        async fn save(&self, entries: Vec<(String, Value)>, _now_ms: i64) -> CoreResult<()> {
            *self.saves.lock().unwrap() += 1;
            let mut rows = self.rows.lock().unwrap();
            for (key, value) in entries {
                match rows.iter_mut().find(|(k, _)| *k == key) {
                    Some(row) => row.1 = value,
                    None => rows.push((key, value)),
                }
            }
            Ok(())
        }
    }

    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    fn service(repo: Arc<FakeRepo>) -> SettingsService {
        SettingsService::new(repo, Arc::new(FixedClock(1_700_000_000_000)))
    }

    #[tokio::test]
    async fn audience_term_is_trimmed_validated_and_persisted() {
        let repo = Arc::new(FakeRepo::default());
        let svc = service(repo.clone());
        assert_eq!(svc.get().await.unwrap().audience_term.plural, "freshies");

        let (next, changed) = svc
            .update(SettingsPatch {
                audience_term: Some(AudienceTerm {
                    singular: "  member ".to_owned(),
                    plural: "club   members".to_owned(),
                }),
                ..SettingsPatch::default()
            })
            .await
            .unwrap();
        assert_eq!(changed, vec![KEY_AUDIENCE_TERM]);
        assert_eq!(next.audience_term.singular, "member");
        assert_eq!(next.audience_term.plural, "club members");
        assert_eq!(svc.get().await.unwrap().audience_term, next.audience_term);

        let err = svc
            .update(SettingsPatch {
                audience_term: Some(AudienceTerm {
                    singular: " ".to_owned(),
                    plural: "x".to_owned(),
                }),
                ..SettingsPatch::default()
            })
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Validation {
                field: "audienceTerm",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn defaults_on_empty_storage() {
        let repo = Arc::new(FakeRepo::default());
        let settings = service(repo).get().await.unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.theme, Theme::System);
    }

    #[tokio::test]
    async fn ignores_unknown_and_malformed_values() {
        let repo = Arc::new(FakeRepo {
            rows: Mutex::new(vec![
                ("mystery".into(), Value::from(1)),
                ("theme".into(), Value::from("chartreuse")),
                (
                    "featureFlags".into(),
                    serde_json::json!({ "ok-flag": true }),
                ),
            ]),
            saves: Mutex::new(0),
        });
        let settings = service(repo).get().await.unwrap();
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.feature_flags.len(), 1);
    }

    #[tokio::test]
    async fn update_merges_flags_and_reports_changes() {
        let repo = Arc::new(FakeRepo::default());
        service(repo.clone())
            .update(SettingsPatch {
                theme: Some(Theme::Dark),
                feature_flags: Some(BTreeMap::from([("alpha".to_owned(), true)])),
                audience_term: None,
            })
            .await
            .unwrap();

        let (settings, changed) = service(repo.clone())
            .update(SettingsPatch {
                theme: None,
                feature_flags: Some(BTreeMap::from([("beta".to_owned(), true)])),
                audience_term: None,
            })
            .await
            .unwrap();

        assert_eq!(changed, vec![KEY_FEATURE_FLAGS]);
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.feature_flags.len(), 2);
        assert!(settings.feature_flags["beta"]);
        assert_eq!(*repo.saves.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn no_op_update_does_not_write() {
        let repo = Arc::new(FakeRepo::default());
        let (settings, changed) = service(repo.clone())
            .update(SettingsPatch {
                theme: Some(Theme::System),
                feature_flags: None,
                audience_term: None,
            })
            .await
            .unwrap();
        assert!(changed.is_empty());
        assert_eq!(settings, Settings::default());
        assert_eq!(*repo.saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn rejects_invalid_flag_ids() {
        let repo = Arc::new(FakeRepo::default());
        let err = service(repo.clone())
            .update(SettingsPatch {
                theme: None,
                feature_flags: Some(BTreeMap::from([("Not Valid".to_owned(), true)])),
                audience_term: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Validation {
                field: "featureFlags",
                ..
            }
        ));
        assert_eq!(*repo.saves.lock().unwrap(), 0);
    }
}
