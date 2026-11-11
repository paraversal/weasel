//! Performance smoke, not a budget. Run with:
//! `cargo test --release -p weasel-store --test perf -- --ignored --nocapture perf_`
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use weasel_core::campaign::{AudienceFilter, CampaignService};
use weasel_core::contact::{ContactQuery, ContactService};
use weasel_core::enums::CampaignStatus;
use weasel_core::identity::NameIdentity;
use weasel_core::query::PageRequest;
use weasel_store::{SqliteCampaigns, SqliteContacts};
use weasel_testkit::builders::CampaignSeed;
use weasel_testkit::{FixedClock, TempDb};

#[tokio::test]
#[ignore = "perf smoke: 50k rows"]
async fn perf_contacts_list_50k() {
    let db = TempDb::new().await;
    let store = db.store();
    let clock = std::sync::Arc::new(FixedClock::new(1_700_000_000_000));
    let contacts = ContactService::new(
        std::sync::Arc::new(SqliteContacts::new(store.clone())),
        clock.clone(),
        std::sync::Arc::new(NameIdentity),
    );
    let campaigns = CampaignService::new(
        std::sync::Arc::new(SqliteCampaigns::new(store.clone())),
        clock.clone(),
    );

    let mut tx = store.begin_write().await.unwrap();
    for i in 0..50_000i64 {
        sqlx::query(
            "INSERT INTO contacts (first_name, last_name, email, email_normalized, \
             status, source, date_added, updated_at, version, notes) \
             VALUES (?, ?, ?, ?, 'active', 'import', 1, 1, 1, '')",
        )
        .bind(format!("First{i}"))
        .bind(format!("Last{i:05}"))
        .bind(format!("person{i}@example.edu"))
        .bind(format!("person{i}@example.edu"))
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    let first_page = ContactQuery {
        page: PageRequest {
            limit: 100,
            offset: 0,
        },
        ..ContactQuery::default()
    };
    let started = Instant::now();
    let page = contacts.list(&first_page).await.unwrap();
    let list_ms = started.elapsed().as_millis();
    assert_eq!(page.total, 50_000);
    assert_eq!(page.items.len(), 100);

    let search = ContactQuery {
        search: Some("Last00042".to_owned()),
        ..ContactQuery::default()
    };
    let started = Instant::now();
    let found = contacts.list(&search).await.unwrap();
    let search_ms = started.elapsed().as_millis();
    assert_eq!(found.total, 1);

    let campaign = campaigns
        .create(CampaignSeed::new("Perf").into_new_campaign())
        .await
        .unwrap();
    campaigns
        .set_status(campaign.id, campaign.version, CampaignStatus::Active)
        .await
        .unwrap();
    let started = Instant::now();
    let audience = campaigns
        .audience(
            campaign.id,
            AudienceFilter::Unsent,
            None,
            &PageRequest::default(),
        )
        .await
        .unwrap();
    let audience_ms = started.elapsed().as_millis();
    assert_eq!(audience.total, 50_000);

    println!(
        "perf_contacts_list_50k: first_page={list_ms}ms search={search_ms}ms audience={audience_ms}ms"
    );
}
