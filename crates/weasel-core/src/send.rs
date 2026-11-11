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

    /// Copies the current unsent audience. When `expected_count` is set, it must match
    /// the audience the user reviewed; otherwise nothing is copied or recorded.
    pub async fn prepare_send(
        &self,
        campaign_id: i64,
        expected_count: Option<u64>,
    ) -> CoreResult<PreparedSend> {
        let pending = self
            .repo
            .begin_batch(campaign_id, self.clock.now_ms())
            .await?;
        if let Some(expected) = expected_count {
            let actual = u64::try_from(pending.emails().len()).unwrap_or(u64::MAX);
            if actual != expected {
                drop(pending);
                return Err(CoreError::Conflict(
                    "The audience changed since you reviewed it. Review it again.".to_owned(),
                ));
            }
        }
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

    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use async_trait::async_trait;

    use crate::ports::{PendingSendBatch, SendRepository};

    use super::*;

    #[test]
    fn recipients_are_joined_with_a_single_space() {
        let emails = vec!["a@x.edu".to_owned(), "b@x.edu".to_owned()];
        assert_eq!(format_recipients(&emails), "a@x.edu b@x.edu");
        assert_eq!(format_recipients(&[]), "");
        assert_eq!(RECIPIENT_SEPARATOR, " ");
    }

    struct FixedTime;

    impl Clock for FixedTime {
        fn now_ms(&self) -> i64 {
            7
        }
    }

    #[derive(Default)]
    struct RecordingClipboard {
        writes: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Clipboard for RecordingClipboard {
        async fn write_text(&self, text: String) -> CoreResult<()> {
            self.writes.lock().unwrap().push(text);
            Ok(())
        }
    }

    struct Pending {
        batch: SendBatch,
        emails: Vec<String>,
        committed: Arc<AtomicBool>,
    }

    #[async_trait]
    impl PendingSendBatch for Pending {
        fn batch(&self) -> &SendBatch {
            &self.batch
        }

        fn emails(&self) -> &[String] {
            &self.emails
        }

        async fn commit(self: Box<Self>) -> CoreResult<()> {
            self.committed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FakeSends {
        emails: Vec<String>,
        committed: Arc<AtomicBool>,
    }

    #[async_trait]
    impl SendRepository for FakeSends {
        async fn begin_batch(
            &self,
            campaign_id: i64,
            now_ms: i64,
        ) -> CoreResult<Box<dyn PendingSendBatch>> {
            Ok(Box::new(Pending {
                batch: SendBatch {
                    id: 1,
                    campaign_id,
                    campaign_name: "Spring".to_owned(),
                    created_at: now_ms,
                    recipient_count: u64::try_from(self.emails.len()).unwrap_or_default(),
                    note: String::new(),
                    copied_at: Some(now_ms),
                },
                emails: self.emails.clone(),
                committed: self.committed.clone(),
            }))
        }

        async fn list(&self, _query: &SendBatchQuery) -> CoreResult<Page<SendBatch>> {
            unreachable!("not used by these tests")
        }

        async fn get(&self, _id: i64) -> CoreResult<SendBatch> {
            unreachable!("not used by these tests")
        }

        async fn recipients(
            &self,
            _id: i64,
            _search: Option<&str>,
            _page: &PageRequest,
        ) -> CoreResult<Page<SendRecord>> {
            unreachable!("not used by these tests")
        }

        async fn emails(&self, _id: i64) -> CoreResult<Vec<String>> {
            unreachable!("not used by these tests")
        }

        async fn mark_recopied(&self, _id: i64, _now_ms: i64) -> CoreResult<SendBatch> {
            unreachable!("not used by these tests")
        }

        async fn set_note(&self, _id: i64, _note: String, _now_ms: i64) -> CoreResult<SendBatch> {
            unreachable!("not used by these tests")
        }
    }

    fn service(
        emails: &[&str],
        committed: Arc<AtomicBool>,
        clipboard: Arc<RecordingClipboard>,
    ) -> SendService {
        SendService::new(
            Arc::new(FakeSends {
                emails: emails.iter().map(|email| (*email).to_owned()).collect(),
                committed,
            }),
            clipboard,
            Arc::new(FixedTime),
        )
    }

    #[tokio::test]
    async fn expected_count_matches_and_commits() {
        let committed = Arc::new(AtomicBool::new(false));
        let clipboard = Arc::new(RecordingClipboard::default());
        let service = service(
            &["a@x.edu", "b@x.edu"],
            committed.clone(),
            clipboard.clone(),
        );

        let prepared = service.prepare_send(1, Some(2)).await.unwrap();
        assert_eq!(
            prepared.emails,
            vec!["a@x.edu".to_owned(), "b@x.edu".to_owned()]
        );
        assert!(committed.load(Ordering::SeqCst));
        assert_eq!(
            clipboard.writes.lock().unwrap().as_slice(),
            ["a@x.edu b@x.edu"]
        );
    }

    #[tokio::test]
    async fn expected_count_mismatch_copies_nothing() {
        let committed = Arc::new(AtomicBool::new(false));
        let clipboard = Arc::new(RecordingClipboard::default());
        let service = service(
            &["a@x.edu", "b@x.edu"],
            committed.clone(),
            clipboard.clone(),
        );

        let err = service.prepare_send(1, Some(1)).await.unwrap_err();
        assert!(matches!(
            &err,
            CoreError::Conflict(message)
                if message == "The audience changed since you reviewed it. Review it again."
        ));
        assert!(!committed.load(Ordering::SeqCst));
        assert!(clipboard.writes.lock().unwrap().is_empty());
    }
}
