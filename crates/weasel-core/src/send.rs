//! Send history types. A batch is one "copy recipients" action.

use serde::{Deserialize, Serialize};

use crate::{enums::SendRecordStatus, query::PageRequest};

/// Separator between addresses when the batch is copied to the clipboard.
pub const RECIPIENT_SEPARATOR: &str = " ";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBatch {
    pub id: i64,
    pub campaign_id: i64,
    pub campaign_name: String,
    pub created_at: i64,
    pub recipient_count: u64,
    pub note: String,
    pub copied_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRecord {
    pub id: i64,
    pub batch_id: i64,
    pub contact_id: Option<i64>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub status: SendRecordStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBatchQuery {
    pub campaign_id: Option<i64>,
    /// Matches the note, campaign name, or a recipient's name/email.
    pub search: Option<String>,
    pub page: PageRequest,
}

/// The batch plus the addresses that were copied, in record order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedSend {
    pub batch: SendBatch,
    pub emails: Vec<String>,
}

/// Join addresses for the clipboard. See `RECIPIENT_SEPARATOR` for why it is a space.
pub fn format_recipients(emails: &[String]) -> String {
    emails.join(RECIPIENT_SEPARATOR)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn recipients_are_joined_with_a_single_space() {
        let emails = vec!["a@x.edu".to_owned(), "b@x.edu".to_owned()];
        assert_eq!(format_recipients(&emails), "a@x.edu b@x.edu");
        assert_eq!(format_recipients(&[]), "");
        assert_eq!(RECIPIENT_SEPARATOR, " ");
    }
}
