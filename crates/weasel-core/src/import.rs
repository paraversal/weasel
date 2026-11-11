//! Import domain types. Reading, mapping and reconciliation live in `weasel-import`;
//! this crate owns the shapes that persist and cross IPC.

use serde::{Deserialize, Serialize};

use crate::{
    email::EmailAddress,
    enums::{ContactSource, ContactStatus, ImportScope, ImportStatus, SourceKind},
};

/// Bump when the persisted plan shape changes. Old drafts are then unreadable by
/// design: they are previews, not user data.
pub const PLAN_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub row_count: u64,
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub conflicts: u64,
    pub unchanged: u64,
    pub invalid: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatch {
    pub id: i64,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub source_path: Option<String>,
    pub file_hash: Option<String>,
    pub sheet_name: Option<String>,
    pub scope: ImportScope,
    pub status: ImportStatus,
    pub created_at: i64,
    pub applied_at: Option<i64>,
    pub summary: ImportSummary,
    pub error: Option<String>,
}

/// One valid spreadsheet row, already cleaned. `row` is the 1-based sheet row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedRow {
    pub row: u32,
    pub first_name: String,
    pub last_name: String,
    pub email: EmailAddress,
    pub identity_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub row: u32,
    pub field: String,
    pub reason: String,
    pub identity_key: Option<String>,
}

/// The subset of a contact reconciliation needs. Never carries notes/metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactRef {
    pub id: i64,
    pub version: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub email_normalized: String,
    pub identity_key: Option<String>,
    pub status: ContactStatus,
    pub source: ContactSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSnapshot {
    /// Active and removed contacts only; archived people are outside the roster.
    pub contacts: Vec<ContactRef>,
    /// Every known address and its contact.
    pub history: Vec<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanAction {
    Add,
    UpdateDetails {
        target: ContactRef,
    },
    UpdateEmail {
        target: ContactRef,
    },
    Restore {
        target: ContactRef,
    },
    Noop {
        contact_id: i64,
    },
    Conflict {
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRow {
    pub input: NormalizedRow,
    pub action: PlanAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRemoval {
    pub contact: ContactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    DuplicateEmailInSource,
    AmbiguousHistory,
    AmbiguousIdentity,
    ManualContact,
    ContactMatchedTwice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub rows: Vec<u32>,
    pub kind: ConflictKind,
    pub candidates: Vec<ContactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanWarning {
    LargeRemoval {
        count: u64,
        active_import_contacts: u64,
    },
    SameFileAsBatch {
        batch_id: i64,
    },
    IgnoredTrailingRows {
        count: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub first_name: u32,
    pub last_name: u32,
    pub email: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub version: u32,
    pub scope: ImportScope,
    pub identity_strategy: String,
    pub mapping: ColumnMapping,
    pub rows: Vec<PlannedRow>,
    pub removals: Vec<PlannedRemoval>,
    pub conflicts: Vec<Conflict>,
    pub errors: Vec<RowError>,
    pub warnings: Vec<PlanWarning>,
    pub summary: ImportSummary,
}

/// A deliberate answer to one conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub row: u32,
    pub action: ConflictAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum ConflictAction {
    LinkContact { contact_id: i64 },
    CreateNew,
    Skip,
}

/// One persisted change. `expected_version` makes apply reject stale drafts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContactOp {
    Create {
        row: u32,
        first_name: String,
        last_name: String,
        email: EmailAddress,
        identity_key: Option<String>,
    },
    Modify {
        row: Option<u32>,
        contact_id: i64,
        expected_version: i64,
        first_name: Option<String>,
        last_name: Option<String>,
        email: Option<EmailAddress>,
        status: Option<ContactStatus>,
        identity_key: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportPhase {
    Reading,
    Matching,
    Applying,
}

/// Draft source metadata supplied by the reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewImportBatch {
    pub source_kind: SourceKind,
    pub source_name: String,
    pub source_path: Option<String>,
    pub file_hash: Option<String>,
    pub sheet_name: Option<String>,
    pub scope: ImportScope,
    pub plan: ImportPlan,
    pub summary: ImportSummary,
}
