use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::{
    audit::AuditEvent,
    campaign::{
        AudienceFilter, AudienceMember, Campaign, CampaignPatch, CampaignQuery, CampaignStats,
        NewCampaign,
    },
    contact::{
        Contact, ContactChanges, ContactDetail, ContactQuery, ContactStats, NewContactRecord,
    },
    email::EmailAddress,
    email_check::{EmailFlag, EmailHold, EmailVerdict, FlagReason, InvalidRule, RulePreview},
    enums::{CampaignStatus, ContactStatus, EmailChangeReason, ImportStatus},
    error::CoreResult,
    import::{
        ContactOp, ImportBatch, ImportPhase, ImportPlan, ImportSummary, NewImportBatch,
        ReconcileSnapshot,
    },
    query::{Page, PageRequest},
    send::{SendBatch, SendBatchQuery, SendRecord},
};

/// Wall-clock access. Production uses the OS clock; tests use a fixed clock.
pub trait Clock: Send + Sync {
    /// Unix milliseconds in UTC.
    fn now_ms(&self) -> i64;
}

/// Write-only clipboard access. Sending a campaign copies addresses here.
#[async_trait]
pub trait Clipboard: Send + Sync {
    async fn write_text(&self, text: String) -> CoreResult<()>;
}

/// Filter shown in a native open-file dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Native file picker. Implementations must not block the async runtime.
#[async_trait]
pub trait FileDialog: Send + Sync {
    async fn pick_file(&self, filters: &[FileFilter]) -> CoreResult<Option<PathBuf>>;
}

/// Persistence port for application settings.
///
/// `save` must write all entries in one transaction: either every entry is stored or
/// none is.
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    async fn load(&self) -> CoreResult<Vec<(String, Value)>>;
    async fn save(&self, entries: Vec<(String, Value)>, now_ms: i64) -> CoreResult<()>;
}

/// Contacts, including their email history and campaign receipts.
///
/// Every write method writes its own audit rows in the same transaction as the change.
#[async_trait]
pub trait ContactRepository: Send + Sync {
    async fn list(&self, query: &ContactQuery) -> CoreResult<Page<Contact>>;
    async fn get(&self, id: i64) -> CoreResult<Contact>;
    async fn detail(&self, id: i64) -> CoreResult<ContactDetail>;
    async fn stats(&self) -> CoreResult<ContactStats>;
    async fn count_active(&self) -> CoreResult<u64>;
    async fn create(&self, contact: NewContactRecord, now_ms: i64) -> CoreResult<Contact>;
    /// Rejects a stale `expected_version` with `Conflict`; email changes also append
    /// history and require `reason`.
    async fn update(
        &self,
        id: i64,
        expected_version: i64,
        changes: ContactChanges,
        reason: EmailChangeReason,
        now_ms: i64,
    ) -> CoreResult<Contact>;
    async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: ContactStatus,
        now_ms: i64,
    ) -> CoreResult<Contact>;
    /// Soft-deletes: sets `removed` and keeps the address occupied.
    async fn delete(&self, id: i64, expected_version: i64, now_ms: i64) -> CoreResult<()>;
}

#[async_trait]
pub trait CampaignRepository: Send + Sync {
    async fn list(&self, query: &CampaignQuery) -> CoreResult<Page<Campaign>>;
    async fn get(&self, id: i64) -> CoreResult<Campaign>;
    async fn create(&self, campaign: NewCampaign, now_ms: i64) -> CoreResult<Campaign>;
    async fn update(&self, id: i64, patch: CampaignPatch, now_ms: i64) -> CoreResult<Campaign>;
    async fn set_status(
        &self,
        id: i64,
        expected_version: i64,
        status: CampaignStatus,
        now_ms: i64,
    ) -> CoreResult<Campaign>;
    async fn delete(&self, id: i64, expected_version: i64, now_ms: i64) -> CoreResult<()>;
    async fn stats(&self, id: i64) -> CoreResult<CampaignStats>;
    /// Stats for a page of campaigns in a fixed number of queries. Unknown ids map to
    /// zeroed stats; callers only pass ids they just read.
    async fn stats_for(&self, ids: &[i64]) -> CoreResult<HashMap<i64, CampaignStats>>;
    async fn audience(
        &self,
        id: i64,
        filter: AudienceFilter,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<AudienceMember>>;
}

/// A batch being prepared. It owns the write transaction; dropping it rolls back, so a
/// failed clipboard write cannot leave a "sent" batch behind.
#[async_trait]
pub trait PendingSendBatch: Send {
    fn batch(&self) -> &SendBatch;
    fn emails(&self) -> &[String];
    async fn commit(self: Box<Self>) -> CoreResult<()>;
}

#[async_trait]
pub trait SendRepository: Send + Sync {
    async fn begin_batch(
        &self,
        campaign_id: i64,
        now_ms: i64,
    ) -> CoreResult<Box<dyn PendingSendBatch>>;
    async fn list(&self, query: &SendBatchQuery) -> CoreResult<Page<SendBatch>>;
    async fn get(&self, id: i64) -> CoreResult<SendBatch>;
    async fn recipients(
        &self,
        id: i64,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<SendRecord>>;
    /// Addresses in record-id order, ready for the clipboard.
    async fn emails(&self, id: i64) -> CoreResult<Vec<String>>;
    async fn mark_recopied(&self, id: i64, now_ms: i64) -> CoreResult<SendBatch>;
    async fn set_note(&self, id: i64, note: String, now_ms: i64) -> CoreResult<SendBatch>;
}

/// Read/write access used by the import planner and applier.
#[async_trait]
pub trait ImportRepository: Send + Sync {
    /// Active and removed contacts plus their address history.
    async fn snapshot(&self) -> CoreResult<ReconcileSnapshot>;
    async fn create_draft(&self, draft: NewImportBatch, now_ms: i64) -> CoreResult<ImportBatch>;
    async fn get(&self, id: i64) -> CoreResult<ImportBatch>;
    async fn plan(&self, id: i64) -> CoreResult<ImportPlan>;
    async fn list(
        &self,
        status: Option<ImportStatus>,
        page: &PageRequest,
    ) -> CoreResult<Page<ImportBatch>>;
    /// Contacts whose `import_batch_id` is `id`.
    async fn members(&self, id: i64, page: &PageRequest) -> CoreResult<Page<Contact>>;
    async fn last_applied_with_hash(&self, hash: &str) -> CoreResult<Option<i64>>;
    /// Applies the ops atomically. Fails with `Conflict` if a contact version moved.
    async fn apply(
        &self,
        id: i64,
        ops: Vec<ContactOp>,
        summary: ImportSummary,
        now_ms: i64,
    ) -> CoreResult<ImportBatch>;
    async fn discard(&self, id: i64, now_ms: i64) -> CoreResult<ImportBatch>;
    async fn count_drafts(&self) -> CoreResult<u64>;
}

/// Invalid-address flags and rules. Every write records an audit row.
#[async_trait]
pub trait EmailCheckRepository: Send + Sync {
    async fn rules(&self) -> CoreResult<Vec<InvalidRule>>;
    /// `Conflict` when the same pattern already exists.
    async fn add_rule(&self, pattern: &str, now_ms: i64) -> CoreResult<InvalidRule>;
    async fn delete_rule(&self, id: i64, now_ms: i64) -> CoreResult<()>;
    async fn preview_rule(&self, pattern: &str) -> CoreResult<RulePreview>;
    async fn flags(&self) -> CoreResult<Vec<EmailFlag>>;
    async fn set_flag(
        &self,
        email: &EmailAddress,
        verdict: EmailVerdict,
        reason: FlagReason,
        now_ms: i64,
    ) -> CoreResult<()>;
    /// Drop the flag; if a rule still matches, store a `Valid` override instead.
    async fn mark_valid(&self, email: &EmailAddress, now_ms: i64) -> CoreResult<()>;
    async fn clear_flag(&self, email: &EmailAddress, now_ms: i64) -> CoreResult<()>;
    async fn holds(&self, normalized: &[String]) -> CoreResult<HashMap<String, EmailHold>>;
    async fn set_bounced(
        &self,
        record_id: i64,
        bounced: bool,
        now_ms: i64,
    ) -> CoreResult<SendRecord>;
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
        page: &PageRequest,
    ) -> CoreResult<Page<AuditEvent>>;
    async fn recent(&self, page: &PageRequest) -> CoreResult<Page<AuditEvent>>;
}

/// Progress for long imports. Implementations must not block.
pub trait ProgressSink: Send + Sync {
    fn report(&self, phase: ImportPhase, processed: u64, total: u64);
}

/// Discards progress.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn report(&self, _phase: ImportPhase, _processed: u64, _total: u64) {}
}
