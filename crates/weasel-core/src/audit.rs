//! Audit trail shapes and the action vocabulary.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable action names. They are queried by the activity screen and by undo later.
pub mod actions {
    pub const CONTACT_CREATE: &str = "contact.create";
    pub const CONTACT_UPDATE: &str = "contact.update";
    pub const CONTACT_EMAIL_CHANGE: &str = "contact.email_change";
    pub const CONTACT_STATUS: &str = "contact.status";
    pub const CONTACT_DELETE: &str = "contact.delete";
    pub const CONTACT_IMPORT_CREATE: &str = "contact.import_create";
    pub const CONTACT_IMPORT_UPDATE: &str = "contact.import_update";

    pub const CAMPAIGN_CREATE: &str = "campaign.create";
    pub const CAMPAIGN_UPDATE: &str = "campaign.update";
    pub const CAMPAIGN_STATUS: &str = "campaign.status";
    pub const CAMPAIGN_DELETE: &str = "campaign.delete";

    pub const SEND_BATCH_CREATE: &str = "send_batch.create";
    pub const SEND_BATCH_RECOPY: &str = "send_batch.recopy";
    pub const SEND_BATCH_NOTE: &str = "send_batch.note";

    pub const IMPORT_PREVIEW: &str = "import.preview";
    pub const IMPORT_APPLY: &str = "import.apply";
    pub const IMPORT_DISCARD: &str = "import.discard";
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: i64,
    pub at: i64,
    pub actor: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub metadata: Value,
}

/// Write-side event; the repository assigns `id` and `at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub actor: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub metadata: Value,
}

impl AuditRecord {
    pub fn new(action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            actor: "local".to_owned(),
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            before: None,
            after: None,
            metadata: Value::Object(Default::default()),
        }
    }
}
