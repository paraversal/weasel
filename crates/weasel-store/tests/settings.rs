#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Settings round trip through the real database, including rollback on failure.

use std::{collections::BTreeMap, sync::Arc};

use serde_json::json;
use weasel_core::{
    CoreError,
    ports::SettingsRepository,
    settings::{KEY_FEATURE_FLAGS, KEY_THEME, SettingsPatch, SettingsService, Theme},
};
use weasel_store::SqliteSettings;
use weasel_testkit::{FixedClock, TempDb};

#[tokio::test]
async fn settings_round_trip_through_sqlite() {
    let db = TempDb::new().await;
    let repo = Arc::new(SqliteSettings::new(db.store()));
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let service = SettingsService::new(repo, clock.clone());

    let (updated, changed) = service
        .update(SettingsPatch {
            theme: Some(Theme::Dark),
            feature_flags: Some(BTreeMap::from([("beta".to_owned(), true)])),
        })
        .await
        .unwrap();
    assert_eq!(changed, vec![KEY_THEME, KEY_FEATURE_FLAGS]);

    let reloaded = SettingsService::new(
        Arc::new(SqliteSettings::new(db.store())),
        Arc::new(FixedClock::new(0)),
    )
    .get()
    .await
    .unwrap();
    assert_eq!(reloaded, updated);
    assert_eq!(reloaded.theme, Theme::Dark);
    assert!(reloaded.feature_flags["beta"]);

    let updated_at: i64 = sqlx::query_scalar("SELECT updated_at FROM settings WHERE key = 'theme'")
        .fetch_one(db.store().pool())
        .await
        .unwrap();
    assert_eq!(updated_at, 1_700_000_000_000);
}

#[tokio::test]
async fn save_rolls_back_when_a_statement_fails() {
    let db = TempDb::new().await;
    sqlx::query(
        "CREATE TRIGGER reject_boom BEFORE INSERT ON settings WHEN NEW.key = 'boom' \
         BEGIN SELECT RAISE(ABORT, 'boom rejected'); END",
    )
    .execute(db.store().pool())
    .await
    .unwrap();

    let repo = SqliteSettings::new(db.store());
    let err = repo
        .save(
            vec![("good".to_owned(), json!(1)), ("boom".to_owned(), json!(2))],
            1,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Internal(_)));

    let good: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'good'")
        .fetch_one(db.store().pool())
        .await
        .unwrap();
    assert_eq!(
        good, 0,
        "first insert must be rolled back with the failed one"
    );

    assert!(repo.load().await.unwrap().is_empty());
}
