//! Backend → frontend event channel.
//!
//! One channel (`weasel-event`) carrying an envelope with a monotonic revision.
//! Emitting is fire-and-forget and must happen only after the causing transaction
//! committed.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::AppHandle;
use tauri_specta::Event;

use crate::error::AppError;

/// Wire contract: the enum is exported to TypeScript in full, so a variant can exist
/// before the feature that emits it lands.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DomainEvent {
    SettingsChanged { keys: Vec<String> },
    #[allow(dead_code)] // emitted by command wrappers once a feature has a failing path
    OperationFailed { operation: String, error: AppError },
    // Later exec plans add: ContactsChanged, CampaignsChanged, SendBatchesChanged,
    // ImportsChanged, ImportProgress, ImportFinished.
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct WeaselEvent {
    pub revision: u64,
    pub event: DomainEvent,
}

#[derive(Debug, Default)]
pub struct EventBus {
    revision: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Revisions start at 1 and never repeat, so the frontend can drop stale payloads.
    pub fn next_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Call only after the transaction that caused the change has committed.
    pub fn emit(&self, app: &AppHandle, event: DomainEvent) {
        let payload = WeaselEvent {
            revision: self.next_revision(),
            event,
        };
        if let Err(err) = payload.emit(app) {
            tracing::warn!(error = %err, "failed to emit weasel event");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn revisions_are_strictly_increasing_across_threads() {
        let bus = Arc::new(EventBus::new());
        let mut handles = Vec::new();
        for _ in 0..4 {
            let bus = Arc::clone(&bus);
            handles.push(std::thread::spawn(move || {
                (0..100).map(|_| bus.next_revision()).collect::<Vec<_>>()
            }));
        }
        let mut seen: Vec<u64> = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect();
        seen.sort_unstable();
        assert_eq!(seen.len(), 400);
        assert_eq!(seen.first().copied(), Some(1));
        assert_eq!(seen.last().copied(), Some(400));
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
