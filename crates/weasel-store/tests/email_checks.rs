//! Invalid addresses: rules, manual flags, bounces, and how they shape sends.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use weasel_core::{
    CoreError,
    campaign::{AudienceFilter, NewCampaign},
    contact::{Contact, ContactPatch, ContactQuery, NewContact},
    email_check::{EmailHold, EmailVerdict},
    enums::{CampaignStatus, SendRecordStatus},
    query::PageRequest,
};
use weasel_testkit::{TempDb, services::TestServices};

async fn contact(services: &TestServices, first: &str, email: &str) -> Contact {
    services
        .contacts
        .create_manual(NewContact {
            first_name: first.to_owned(),
            last_name: "Test".to_owned(),
            email: email.to_owned(),
            notes: String::new(),
        })
        .await
        .unwrap()
}

async fn active_campaign(services: &TestServices) -> i64 {
    let campaign = services
        .campaigns
        .create(NewCampaign {
            name: "Welcome".to_owned(),
            description: String::new(),
            subject: String::new(),
            body: String::new(),
        })
        .await
        .unwrap();
    services
        .campaigns
        .set_status(campaign.id, campaign.version, CampaignStatus::Active)
        .await
        .unwrap()
        .id
}

async fn held_names(services: &TestServices) -> Vec<String> {
    services
        .contacts
        .list(&ContactQuery {
            on_hold: true,
            ..ContactQuery::default()
        })
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|c| c.first_name)
        .collect()
}

#[tokio::test]
async fn a_rule_holds_matching_people_out_of_the_next_send() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    contact(&services, "Ada", "Ada@Temp.Uni.edu").await;
    contact(&services, "Bob", "bob@uni.edu").await;
    let campaign_id = active_campaign(&services).await;

    let preview = services
        .email_checks
        .preview_rule(" *@TEMP.uni.edu ")
        .await
        .unwrap();
    assert_eq!(preview.pattern, "*@temp.uni.edu");
    assert_eq!(preview.matches, 1);
    assert_eq!(preview.examples, vec!["Ada@Temp.Uni.edu".to_owned()]);

    let rule = services
        .email_checks
        .add_rule("*@temp.uni.edu")
        .await
        .unwrap();
    assert_eq!(rule.matches, 1);
    assert_eq!(held_names(&services).await, vec!["Ada"]);

    let stats = services.campaigns.stats(campaign_id).await.unwrap();
    assert_eq!((stats.unsent, stats.on_hold, stats.total), (1, 1, 2));

    let unsent = services
        .campaigns
        .audience(
            campaign_id,
            AudienceFilter::Unsent,
            None,
            &PageRequest::default(),
        )
        .await
        .unwrap();
    let flags: Vec<(String, bool)> = unsent
        .items
        .iter()
        .map(|m| (m.first_name.clone(), m.on_hold))
        .collect();
    assert_eq!(
        flags,
        vec![("Ada".to_owned(), true), ("Bob".to_owned(), false)]
    );

    let prepared = services
        .sends
        .prepare_send(campaign_id, Some(1))
        .await
        .unwrap();
    assert_eq!(prepared.emails, vec!["bob@uni.edu".to_owned()]);

    let error = services
        .sends
        .prepare_send(campaign_id, None)
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidState(_)));
}

#[tokio::test]
async fn a_corrected_address_releases_the_hold() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    let ada = contact(&services, "Ada", "ada@temp.uni.edu").await;
    let campaign_id = active_campaign(&services).await;
    services
        .email_checks
        .add_rule("*@temp.uni.edu")
        .await
        .unwrap();
    assert_eq!(held_names(&services).await, vec!["Ada"]);

    services
        .contacts
        .update(
            ada.id,
            ContactPatch {
                version: ada.version,
                email: Some("ada@uni.edu".to_owned()),
                ..ContactPatch::default()
            },
        )
        .await
        .unwrap();

    assert!(held_names(&services).await.is_empty());
    let stats = services.campaigns.stats(campaign_id).await.unwrap();
    assert_eq!((stats.unsent, stats.on_hold), (1, 0));
}

#[tokio::test]
async fn marking_valid_overrides_a_rule_and_clearing_restores_it() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    contact(&services, "Ada", "ada@temp.uni.edu").await;
    contact(&services, "Bob", "bob@uni.edu").await;
    services
        .email_checks
        .add_rule("*@temp.uni.edu")
        .await
        .unwrap();

    services
        .email_checks
        .mark_valid("ADA@temp.uni.edu")
        .await
        .unwrap();
    assert!(held_names(&services).await.is_empty());
    let flags = services.email_checks.flags().await.unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].verdict, EmailVerdict::Valid);
    assert_eq!(flags[0].contacts, 1);

    services
        .email_checks
        .clear_flag("ada@temp.uni.edu")
        .await
        .unwrap();
    assert_eq!(held_names(&services).await, vec!["Ada"]);

    services
        .email_checks
        .mark_invalid("bob@uni.edu")
        .await
        .unwrap();
    let holds = services
        .email_checks
        .holds(&[
            "ada@temp.uni.edu".to_owned(),
            "bob@uni.edu".to_owned(),
            "cid@uni.edu".to_owned(),
        ])
        .await
        .unwrap();
    assert_eq!(
        holds.get("ada@temp.uni.edu"),
        Some(&EmailHold::Rule {
            pattern: "*@temp.uni.edu".to_owned()
        })
    );
    assert_eq!(holds.get("bob@uni.edu"), Some(&EmailHold::Manual));
    assert!(!holds.contains_key("cid@uni.edu"));

    // Without a rule, "valid" drops the flag instead of storing an override.
    services
        .email_checks
        .mark_valid("bob@uni.edu")
        .await
        .unwrap();
    assert!(
        services
            .email_checks
            .flags()
            .await
            .unwrap()
            .iter()
            .all(|flag| flag.email_normalized != "bob@uni.edu")
    );
}

#[tokio::test]
async fn a_bounce_reopens_the_person_once_their_address_is_fixed() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    let bob = contact(&services, "Bob", "bob@uni.edu").await;
    let campaign_id = active_campaign(&services).await;
    let prepared = services
        .sends
        .prepare_send(campaign_id, None)
        .await
        .unwrap();
    let record = services
        .sends
        .recipients(prepared.batch.id, None, &PageRequest::default())
        .await
        .unwrap()
        .items
        .remove(0);

    let bounced = services
        .email_checks
        .set_bounced(record.id, true)
        .await
        .unwrap();
    assert_eq!(bounced.status, SendRecordStatus::Failed);
    let stats = services.campaigns.stats(campaign_id).await.unwrap();
    assert_eq!(
        (stats.sent, stats.bounced, stats.unsent, stats.on_hold),
        (0, 1, 0, 1)
    );
    assert_eq!(held_names(&services).await, vec!["Bob"]);

    let sent = services
        .campaigns
        .audience(
            campaign_id,
            AudienceFilter::Sent,
            None,
            &PageRequest::default(),
        )
        .await
        .unwrap();
    assert!(sent.items[0].bounced);
    assert_eq!(sent.items[0].record_id, Some(record.id));

    services
        .contacts
        .update(
            bob.id,
            ContactPatch {
                version: bob.version,
                email: Some("bob.b@uni.edu".to_owned()),
                ..ContactPatch::default()
            },
        )
        .await
        .unwrap();
    let again = services
        .sends
        .prepare_send(campaign_id, Some(1))
        .await
        .unwrap();
    assert_eq!(again.emails, vec!["bob.b@uni.edu".to_owned()]);
}

#[tokio::test]
async fn undoing_a_bounce_restores_the_record_and_drops_its_flag() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    contact(&services, "Bob", "bob@uni.edu").await;
    let campaign_id = active_campaign(&services).await;
    let prepared = services
        .sends
        .prepare_send(campaign_id, None)
        .await
        .unwrap();
    let record = services
        .sends
        .recipients(prepared.batch.id, None, &PageRequest::default())
        .await
        .unwrap()
        .items
        .remove(0);

    services
        .email_checks
        .set_bounced(record.id, true)
        .await
        .unwrap();
    let restored = services
        .email_checks
        .set_bounced(record.id, false)
        .await
        .unwrap();
    assert_eq!(restored.status, SendRecordStatus::Copied);
    assert!(services.email_checks.flags().await.unwrap().is_empty());
    let stats = services.campaigns.stats(campaign_id).await.unwrap();
    assert_eq!((stats.sent, stats.bounced, stats.unsent), (1, 0, 0));
}

#[tokio::test]
async fn rules_are_validated_and_unique() {
    let db = TempDb::new().await;
    let services = TestServices::new(&db);
    let error = services.email_checks.add_rule("*@*").await.unwrap_err();
    assert!(matches!(error, CoreError::Validation { .. }));

    let rule = services.email_checks.add_rule("*@temp.org").await.unwrap();
    let error = services
        .email_checks
        .add_rule("*@TEMP.org")
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::Conflict(_)));

    services.email_checks.delete_rule(rule.id).await.unwrap();
    assert!(services.email_checks.rules().await.unwrap().is_empty());
    let error = services
        .email_checks
        .delete_rule(rule.id)
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::NotFound { .. }));
}
