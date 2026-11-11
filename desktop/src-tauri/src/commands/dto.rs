//! Shared wire types and command plumbing.
//!
//! DTO enums are generated rather than hand-written so their serde spelling cannot
//! drift from the core enum's. Request and response structs stay separate: specta
//! otherwise emits `_Serialize`/`_Deserialize` twins for every type used in both
//! directions.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use weasel_core::CoreError;
use weasel_core::campaign::AudienceFilter;
use weasel_core::contact::ContactSort;
use weasel_core::enums::{
    CampaignStatus, ContactSource, ContactStatus, EmailChangeReason, ImportScope, ImportStatus,
    SendRecordStatus, SourceKind,
};
use weasel_core::import::{ConflictKind, ImportPhase};
use weasel_core::query::{Page, PageRequest};

use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::AppState;

macro_rules! dto_enum_with {
    ($dto:ident <=> $core:ty, $rename:literal { $($variant:ident),+ $(,)? }) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type,
        )]
        #[serde(rename_all = $rename)]
        pub enum $dto {
            $($variant),+
        }

        impl From<$core> for $dto {
            fn from(value: $core) -> Self {
                match value {
                    $(<$core>::$variant => Self::$variant),+
                }
            }
        }

        impl From<$dto> for $core {
            fn from(value: $dto) -> Self {
                match value {
                    $($dto::$variant => Self::$variant),+
                }
            }
        }
    };
}

/// Generates a snake_case DTO enum mirroring a `string_enum!` core enum.
macro_rules! dto_enum {
    ($dto:ident <=> $core:path { $($variant:ident),+ $(,)? }) => {
        dto_enum_with!($dto <=> $core, "snake_case" { $($variant),+ });
    };
}

/// Generates a camelCase DTO enum for core enums declared with `rename_all = "camelCase"`.
macro_rules! dto_enum_camel {
    ($dto:ident <=> $core:path { $($variant:ident),+ $(,)? }) => {
        dto_enum_with!($dto <=> $core, "camelCase" { $($variant),+ });
    };
}

dto_enum!(ContactStatusDto <=> ContactStatus { Active, Removed, Archived });
dto_enum!(ContactSourceDto <=> ContactSource { Import, Manual, Api });
dto_enum_camel!(ContactSortDto <=> ContactSort { LastNameAsc, LastNameDesc, AddedDesc, EmailAsc });
dto_enum!(
    EmailChangeReasonDto <=> EmailChangeReason { Import, Manual, Merge, Correction }
);
dto_enum!(CampaignStatusDto <=> CampaignStatus { Draft, Active, Archived });
dto_enum_camel!(AudienceFilterDto <=> AudienceFilter { Sent, Unsent, All });
dto_enum!(
    SendRecordStatusDto <=> SendRecordStatus { Created, Copied, Sent, Failed, Skipped }
);
dto_enum!(ImportStatusDto <=> ImportStatus { Draft, Applied, Failed, Discarded });
dto_enum!(ImportScopeDto <=> ImportScope { Roster, Delta });
dto_enum!(SourceKindDto <=> SourceKind { Xlsx, Xls, Ods, Csv, Manual });
dto_enum_camel!(
    ConflictKindDto <=> ConflictKind {
        DuplicateEmailInSource,
        AmbiguousHistory,
        AmbiguousIdentity,
        ManualContact,
        ContactMatchedTwice,
    }
);
dto_enum_camel!(ImportPhaseDto <=> ImportPhase { Reading, Matching, Applying });

/// A page of wire DTOs. `total` is an `i64` because JavaScript numbers are doubles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

impl<T, U> From<Page<U>> for PageDto<T>
where
    T: From<U>,
{
    fn from(page: Page<U>) -> Self {
        Self {
            items: page.items.into_iter().map(T::from).collect(),
            total: i64::try_from(page.total).unwrap_or(i64::MAX),
            offset: i64::from(page.offset),
            limit: i64::from(page.limit),
        }
    }
}

/// Applies the page contract: missing/zero means the default, values above the cap
/// are clamped.
pub fn page(limit: Option<u32>, offset: Option<u32>) -> PageRequest {
    PageRequest::new(limit.unwrap_or(0), offset.unwrap_or(0)).normalized()
}

/// Trims a search term; blank means "no filter"; very long terms are user errors.
pub fn search_term(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let term = raw.trim();
    if term.is_empty() {
        return Ok(None);
    }
    if term.chars().count() > 200 {
        return Err(AppError::from(CoreError::validation(
            "search",
            "must be at most 200 characters",
        )));
    }
    Ok(Some(term.to_owned()))
}

/// A write result plus the events to emit after its transaction committed.
#[derive(Debug)]
pub struct Emitted<T> {
    pub value: T,
    pub events: Vec<DomainEvent>,
}

impl<T> Emitted<T> {
    #[allow(dead_code)] // first used by multi-event commands in M3
    pub fn new(value: T, events: Vec<DomainEvent>) -> Self {
        Self { value, events }
    }

    pub fn one(value: T, event: DomainEvent) -> Self {
        Self {
            value,
            events: vec![event],
        }
    }

    pub fn read(value: T) -> Self {
        Self {
            value,
            events: Vec::new(),
        }
    }
}

/// Emits queued events in order and returns the value. An error result carries no
/// events, so this is only reached after a successful commit.
pub fn finish<T>(app: &AppHandle, state: &AppState, emitted: Emitted<T>) -> T {
    for event in emitted.events {
        state.events.emit(app, event);
    }
    emitted.value
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use serde::Serialize;
    use serde_json::json;
    use weasel_core::campaign::AudienceFilter;
    use weasel_core::contact::ContactSort;
    use weasel_core::enums::{
        CampaignStatus, ContactSource, ContactStatus, EmailChangeReason, ImportScope, ImportStatus,
        SendRecordStatus, SourceKind,
    };
    use weasel_core::import::{ConflictKind, ImportPhase};

    use crate::events::DomainEvent;

    use super::*;

    fn same<D: Serialize, C: Serialize>(dto: D, core: C) {
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            serde_json::to_value(core).unwrap()
        );
    }

    #[test]
    fn every_enum_spelling_matches_the_domain() {
        same(ContactStatusDto::Active, ContactStatus::Active);
        same(ContactStatusDto::Removed, ContactStatus::Removed);
        same(ContactStatusDto::Archived, ContactStatus::Archived);

        same(ContactSourceDto::Import, ContactSource::Import);
        same(ContactSourceDto::Manual, ContactSource::Manual);
        same(ContactSourceDto::Api, ContactSource::Api);

        same(ContactSortDto::LastNameAsc, ContactSort::LastNameAsc);
        same(ContactSortDto::LastNameDesc, ContactSort::LastNameDesc);
        same(ContactSortDto::AddedDesc, ContactSort::AddedDesc);
        same(ContactSortDto::EmailAsc, ContactSort::EmailAsc);

        same(EmailChangeReasonDto::Import, EmailChangeReason::Import);
        same(EmailChangeReasonDto::Manual, EmailChangeReason::Manual);
        same(EmailChangeReasonDto::Merge, EmailChangeReason::Merge);
        same(
            EmailChangeReasonDto::Correction,
            EmailChangeReason::Correction,
        );

        same(CampaignStatusDto::Draft, CampaignStatus::Draft);
        same(CampaignStatusDto::Active, CampaignStatus::Active);
        same(CampaignStatusDto::Archived, CampaignStatus::Archived);

        same(AudienceFilterDto::Sent, AudienceFilter::Sent);
        same(AudienceFilterDto::Unsent, AudienceFilter::Unsent);
        same(AudienceFilterDto::All, AudienceFilter::All);

        same(SendRecordStatusDto::Created, SendRecordStatus::Created);
        same(SendRecordStatusDto::Copied, SendRecordStatus::Copied);
        same(SendRecordStatusDto::Sent, SendRecordStatus::Sent);
        same(SendRecordStatusDto::Failed, SendRecordStatus::Failed);
        same(SendRecordStatusDto::Skipped, SendRecordStatus::Skipped);

        same(ImportStatusDto::Draft, ImportStatus::Draft);
        same(ImportStatusDto::Applied, ImportStatus::Applied);
        same(ImportStatusDto::Failed, ImportStatus::Failed);
        same(ImportStatusDto::Discarded, ImportStatus::Discarded);

        same(ImportScopeDto::Roster, ImportScope::Roster);
        same(ImportScopeDto::Delta, ImportScope::Delta);

        same(SourceKindDto::Xlsx, SourceKind::Xlsx);
        same(SourceKindDto::Xls, SourceKind::Xls);
        same(SourceKindDto::Ods, SourceKind::Ods);
        same(SourceKindDto::Csv, SourceKind::Csv);
        same(SourceKindDto::Manual, SourceKind::Manual);

        same(
            ConflictKindDto::DuplicateEmailInSource,
            ConflictKind::DuplicateEmailInSource,
        );
        same(
            ConflictKindDto::AmbiguousHistory,
            ConflictKind::AmbiguousHistory,
        );
        same(
            ConflictKindDto::AmbiguousIdentity,
            ConflictKind::AmbiguousIdentity,
        );
        same(ConflictKindDto::ManualContact, ConflictKind::ManualContact);
        same(
            ConflictKindDto::ContactMatchedTwice,
            ConflictKind::ContactMatchedTwice,
        );

        same(ImportPhaseDto::Reading, ImportPhase::Reading);
        same(ImportPhaseDto::Matching, ImportPhase::Matching);
        same(ImportPhaseDto::Applying, ImportPhase::Applying);
    }

    #[test]
    fn import_progress_uses_camel_case_fields() {
        let event = DomainEvent::ImportProgress {
            progress_id: "import-1".to_owned(),
            phase: ImportPhaseDto::Matching,
            processed: 3,
            total: 10,
        };
        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["type"], json!("importProgress"));
        assert_eq!(value["progressId"], json!("import-1"));
        assert_eq!(value["phase"], json!("matching"));
        assert_eq!(value["processed"], json!(3));
        assert_eq!(value["total"], json!(10));
        assert!(value.get("progress_id").is_none());
    }

    #[test]
    fn unit_variants_have_only_a_type_tag() {
        assert_eq!(
            serde_json::to_value(DomainEvent::ContactsChanged {}).unwrap(),
            json!({ "type": "contactsChanged" })
        );
    }

    #[test]
    fn page_dto_keeps_totals_and_clamps_input() {
        let request = page(Some(1000), None);
        assert_eq!(request.limit, 500);
        assert_eq!(page(None, None).limit, 100);

        let page = Page::new(vec![1u64, 2], 9, request);
        let dto: PageDto<u64> = page.into();
        assert_eq!(dto.items, vec![1, 2]);
        assert_eq!(dto.total, 9);
        assert_eq!(dto.limit, 500);
    }

    #[test]
    fn search_terms_are_trimmed_and_bounded() {
        assert_eq!(search_term(None).unwrap(), None);
        assert_eq!(search_term(Some("   ".to_owned())).unwrap(), None);
        assert_eq!(
            search_term(Some("  Ada  ".to_owned())).unwrap(),
            Some("Ada".to_owned())
        );

        let error = search_term(Some("x".repeat(201))).unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Validation);
        assert_eq!(error.details.unwrap()["field"], json!("search"));
    }
}
