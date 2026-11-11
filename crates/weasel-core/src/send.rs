//! Send history types. A batch is one "copy recipients" action.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CoreError, CoreResult,
    enums::SendRecordStatus,
    limits,
    ports::{Clipboard, Clock, SendRepository},
    query::{Page, PageRequest},
};

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

/// Copying recipients is an application service because the clipboard is a port.
///
/// The clipboard is written before the batch commits: a clipboard failure records
/// nothing, and a commit failure after a successful copy leaves nothing behind, so
/// retrying is safe.
pub struct SendService {
    repo: Arc<dyn SendRepository>,
    clipboard: Arc<dyn Clipboard>,
    clock: Arc<dyn Clock>,
}

impl SendService {
    pub fn new(
        repo: Arc<dyn SendRepository>,
        clipboard: Arc<dyn Clipboard>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repo,
            clipboard,
            clock,
        }
    }

    pub async fn prepare_send(&self, campaign_id: i64) -> CoreResult<PreparedSend> {
        let pending = self
            .repo
            .begin_batch(campaign_id, self.clock.now_ms())
            .await?;
        if let Err(err) = self
            .clipboard
            .write_text(format_recipients(pending.emails()))
            .await
        {
            drop(pending);
            return Err(err);
        }
        let batch = pending.batch().clone();
        let emails = pending.emails().to_vec();
        pending.commit().await?;
        Ok(PreparedSend { batch, emails })
    }

    /// Copies the original snapshot again; edits to contacts do not rewrite history.
    pub async fn recopy(&self, batch_id: i64) -> CoreResult<PreparedSend> {
        let emails = self.repo.emails(batch_id).await?;
        self.clipboard
            .write_text(format_recipients(&emails))
            .await?;
        let batch = self
            .repo
            .mark_recopied(batch_id, self.clock.now_ms())
            .await?;
        Ok(PreparedSend { batch, emails })
    }

    pub async fn set_note(&self, batch_id: i64, note: String) -> CoreResult<SendBatch> {
        if note.chars().count() > limits::BATCH_NOTE_MAX {
            return Err(CoreError::validation(
                "note",
                format!("is longer than {} characters", limits::BATCH_NOTE_MAX),
            ));
        }
        self.repo
            .set_note(batch_id, note, self.clock.now_ms())
            .await
    }

    pub async fn list(&self, query: &SendBatchQuery) -> CoreResult<Page<SendBatch>> {
        self.repo.list(query).await
    }

    pub async fn get(&self, batch_id: i64) -> CoreResult<SendBatch> {
        self.repo.get(batch_id).await
    }

    pub async fn recipients(
        &self,
        batch_id: i64,
        search: Option<&str>,
        page: &PageRequest,
    ) -> CoreResult<Page<SendRecord>> {
        self.repo.recipients(batch_id, search, page).await
    }
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
