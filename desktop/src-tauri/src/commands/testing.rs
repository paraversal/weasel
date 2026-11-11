//! Real services over a temporary database, with scripted ports.

#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)] // shared by every command test module

use std::sync::Arc;

use weasel_core::campaign::{Campaign, NewCampaign};
use weasel_core::contact::{Contact, NewContact};
use weasel_core::ports::{Clipboard, Clock, FileDialog};
use weasel_testkit::builders::{CampaignSeed, ContactSeed};
use weasel_testkit::{FakeClipboard, FakeFileDialog, FixedClock, TempDb};

use crate::compose;
use crate::state::{PickedSources, Services};

pub struct Harness {
    pub services: Services,
    pub clock: Arc<FixedClock>,
    pub clipboard: Arc<FakeClipboard>,
    pub dialogs: Arc<FakeFileDialog>,
    pub picked: PickedSources,
    /// Declared last so services drop before the temporary directory disappears.
    pub db: TempDb,
}

pub async fn harness() -> Harness {
    let db = TempDb::new().await;
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let clipboard = Arc::new(FakeClipboard::new());
    let dialogs = Arc::new(FakeFileDialog::new());

    let clock_port: Arc<dyn Clock> = clock.clone();
    let clipboard_port: Arc<dyn Clipboard> = clipboard.clone();
    let dialogs_port: Arc<dyn FileDialog> = dialogs.clone();
    let services = compose(db.store(), clock_port, clipboard_port, dialogs_port);

    Harness {
        services,
        clock,
        clipboard,
        dialogs,
        picked: PickedSources::default(),
        db,
    }
}

pub async fn seed_contact(harness: &Harness, first: &str, last: &str, email: &str) -> Contact {
    harness
        .services
        .contacts
        .create_manual(ContactSeed::new(first, last, email).into_new_contact())
        .await
        .unwrap()
}

pub async fn seed_campaign(harness: &Harness, name: &str) -> Campaign {
    harness
        .services
        .campaigns
        .create(CampaignSeed::new(name).into_new_campaign())
        .await
        .unwrap()
}

pub fn manual(first: &str, last: &str, email: &str) -> NewContact {
    ContactSeed::new(first, last, email).into_new_contact()
}

pub fn campaign(name: &str) -> NewCampaign {
    CampaignSeed::new(name).into_new_campaign()
}
