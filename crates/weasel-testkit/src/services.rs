//! Fully composed services over a temporary database.

use std::sync::Arc;

use weasel_core::campaign::CampaignService;
use weasel_core::contact::ContactService;
use weasel_core::email_check::EmailCheckService;
use weasel_core::identity::{NAME_IDENTITY_ID, NameIdentity};
use weasel_core::ports::AuditRepository;
use weasel_core::send::SendService;
use weasel_import::service::ImportService;
use weasel_store::{
    SqliteAudit, SqliteCampaigns, SqliteContacts, SqliteEmailChecks, SqliteImports, SqliteSends,
    Store,
};

use crate::{FakeClipboard, FixedClock, TempDb};

/// Every domain service wired to the same temp database.
pub struct TestServices {
    pub store: Store,
    pub clock: Arc<FixedClock>,
    pub clipboard: Arc<FakeClipboard>,
    pub contacts: ContactService,
    pub campaigns: CampaignService,
    pub sends: SendService,
    pub imports: ImportService,
    pub email_checks: EmailCheckService,
    pub audit: Arc<dyn AuditRepository>,
}

impl TestServices {
    pub fn new(db: &TempDb) -> Self {
        let store = db.store();
        let clock = Arc::new(FixedClock::new(1_700_000_000_000));
        let clipboard = Arc::new(FakeClipboard::new());
        let identity = Arc::new(NameIdentity);

        let contacts = ContactService::new(
            Arc::new(SqliteContacts::new(store.clone())),
            clock.clone(),
            identity.clone(),
        );
        let campaigns =
            CampaignService::new(Arc::new(SqliteCampaigns::new(store.clone())), clock.clone());
        let sends = SendService::new(
            Arc::new(SqliteSends::new(store.clone())),
            clipboard.clone(),
            clock.clone(),
        );
        let imports = ImportService::new(
            Arc::new(SqliteImports::new(store.clone())),
            clock.clone(),
            identity,
        );
        let email_checks = EmailCheckService::new(
            Arc::new(SqliteEmailChecks::new(store.clone())),
            clock.clone(),
        );
        let audit: Arc<dyn AuditRepository> = Arc::new(SqliteAudit::new(store.clone()));

        Self {
            store,
            clock,
            clipboard,
            contacts,
            campaigns,
            sends,
            imports,
            email_checks,
            audit,
        }
    }

    pub fn identity_id(&self) -> &'static str {
        NAME_IDENTITY_ID
    }
}
