//! Import commands: pick a file, inspect it, preview, apply, discard and review.
//!
//! Reads are only allowed for paths the native dialog returned in this session. The
//! allowlist lives in [`PickedSources`] and holds canonical paths, so a symlink swap
//! after picking cannot redirect a read.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::CoreError;
use weasel_core::FeatureDescriptor;
use weasel_core::enums::ImportStatus;
use weasel_core::import::{
    ColumnMapping, Conflict, ConflictAction, ConflictResolution, ImportBatch, ImportPhase,
    ImportSummary, PlanAction, PlanWarning, PlannedRow, RowError,
};
use weasel_core::ports::{FileFilter, ProgressSink};
use weasel_import::mapping::{SuggestedMapping, suggest};
use weasel_import::service::PreviewOptions;
use weasel_import::source::{
    ParsedSource, ReadOptions, SUPPORTED_EXTENSIONS, kind_for, read_source as read_parsed,
};
use weasel_platform::fs::validate_source_path;

use crate::commands::contacts::ContactDto;
use crate::commands::dto::{
    ConflictKindDto, Emitted, ImportScopeDto, ImportStatusDto, PageDto, SourceKindDto, finish, page,
};
use crate::commands::email_checks::{EmailHoldDto, holds_for};
use crate::error::AppError;
use crate::events::{DomainEvent, EventEmitter};
use crate::state::{AppState, PickedSources, Services};

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "imports",
        title: "Imports",
        commands: &[
            "imports_pick_source",
            "imports_inspect",
            "imports_preview",
            "imports_apply",
            "imports_discard",
            "imports_list",
            "imports_get",
            "imports_plan_rows",
        ],
        events: &["importsChanged", "importProgress", "importFinished"],
    }
}

// ---------------------------------------------------------------------------
// Wire types

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileDto {
    pub path: String,
    pub name: String,
    pub kind: SourceKindDto,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedMappingDto {
    #[serde(default)]
    pub first_name: Option<u32>,
    #[serde(default)]
    pub last_name: Option<u32>,
    #[serde(default)]
    pub email: Option<u32>,
}

impl From<SuggestedMapping> for SuggestedMappingDto {
    fn from(mapping: SuggestedMapping) -> Self {
        Self {
            first_name: mapping.first_name,
            last_name: mapping.last_name,
            email: mapping.email,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SampleRowDto {
    pub row: u32,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceInspectionDto {
    pub name: String,
    pub kind: SourceKindDto,
    pub sheets: Vec<String>,
    pub sheet_name: Option<String>,
    pub header_row: u32,
    pub headers: Vec<String>,
    pub suggested_mapping: SuggestedMappingDto,
    pub sample_rows: Vec<SampleRowDto>,
    pub row_count: u64,
    pub ignored_trailing: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummaryDto {
    pub row_count: u64,
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub conflicts: u64,
    pub unchanged: u64,
    pub invalid: u64,
}

impl From<ImportSummary> for ImportSummaryDto {
    fn from(summary: ImportSummary) -> Self {
        Self {
            row_count: summary.row_count,
            added: summary.added,
            updated: summary.updated,
            removed: summary.removed,
            conflicts: summary.conflicts,
            unchanged: summary.unchanged,
            invalid: summary.invalid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchDto {
    pub id: i64,
    pub source_kind: SourceKindDto,
    pub source_name: String,
    pub sheet_name: Option<String>,
    pub scope: ImportScopeDto,
    pub status: ImportStatusDto,
    pub created_at: i64,
    pub applied_at: Option<i64>,
    pub summary: ImportSummaryDto,
    pub error: Option<String>,
}

impl From<ImportBatch> for ImportBatchDto {
    fn from(batch: ImportBatch) -> Self {
        Self {
            id: batch.id,
            source_kind: batch.source_kind.into(),
            source_name: batch.source_name,
            sheet_name: batch.sheet_name,
            scope: batch.scope.into(),
            status: batch.status.into(),
            created_at: batch.created_at,
            applied_at: batch.applied_at,
            summary: batch.summary.into(),
            error: batch.error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContactRefDto {
    pub id: i64,
    pub version: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub status: crate::commands::dto::ContactStatusDto,
    pub source: crate::commands::dto::ContactSourceDto,
}

impl From<weasel_core::import::ContactRef> for ContactRefDto {
    fn from(contact: weasel_core::import::ContactRef) -> Self {
        Self {
            id: contact.id,
            version: contact.version,
            first_name: contact.first_name,
            last_name: contact.last_name,
            email: contact.email,
            status: contact.status.into(),
            source: contact.source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlanActionDto {
    Add,
    UpdateDetails { target: ContactRefDto },
    UpdateEmail { target: ContactRefDto },
    Restore { target: ContactRefDto },
    Noop { contact_id: i64 },
    Conflict { index: u32 },
}

impl From<PlanAction> for PlanActionDto {
    fn from(action: PlanAction) -> Self {
        match action {
            PlanAction::Add => Self::Add,
            PlanAction::UpdateDetails { target } => Self::UpdateDetails {
                target: target.into(),
            },
            PlanAction::UpdateEmail { target } => Self::UpdateEmail {
                target: target.into(),
            },
            PlanAction::Restore { target } => Self::Restore {
                target: target.into(),
            },
            PlanAction::Noop { contact_id } => Self::Noop { contact_id },
            PlanAction::Conflict { index } => Self::Conflict {
                index: u32::try_from(index).unwrap_or(u32::MAX),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRowDto {
    pub row: u32,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub action: PlanActionDto,
    /// The incoming address is on hold.
    pub email_hold: Option<EmailHoldDto>,
    /// For an email update: the hold on the address being replaced, if any.
    pub previous_hold: Option<EmailHoldDto>,
}

impl From<PlannedRow> for PlannedRowDto {
    fn from(planned: PlannedRow) -> Self {
        Self {
            row: planned.input.row,
            first_name: planned.input.first_name,
            last_name: planned.input.last_name,
            email: planned.input.email.display,
            action: planned.action.into(),
            email_hold: None,
            previous_hold: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDto {
    pub rows: Vec<u32>,
    pub kind: ConflictKindDto,
    pub candidates: Vec<ContactRefDto>,
}

impl From<Conflict> for ConflictDto {
    fn from(conflict: Conflict) -> Self {
        Self {
            rows: conflict.rows,
            kind: conflict.kind.into(),
            candidates: conflict
                .candidates
                .into_iter()
                .map(ContactRefDto::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RowErrorDto {
    pub row: u32,
    pub field: String,
    pub reason: String,
}

impl From<RowError> for RowErrorDto {
    fn from(error: RowError) -> Self {
        Self {
            row: error.row,
            field: error.field,
            reason: error.reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlanWarningDto {
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

impl From<PlanWarning> for PlanWarningDto {
    fn from(warning: PlanWarning) -> Self {
        match warning {
            PlanWarning::LargeRemoval {
                count,
                active_import_contacts,
            } => Self::LargeRemoval {
                count,
                active_import_contacts,
            },
            PlanWarning::SameFileAsBatch { batch_id } => Self::SameFileAsBatch { batch_id },
            PlanWarning::IgnoredTrailingRows { count } => Self::IgnoredTrailingRows { count },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMappingDto {
    pub first_name: u32,
    pub last_name: u32,
    pub email: u32,
}

impl From<ColumnMapping> for ColumnMappingDto {
    fn from(mapping: ColumnMapping) -> Self {
        Self {
            first_name: mapping.first_name,
            last_name: mapping.last_name,
            email: mapping.email,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewDto {
    pub batch: ImportBatchDto,
    pub headers: Vec<String>,
    pub header_row: u32,
    pub suggested_mapping: SuggestedMappingDto,
    pub mapping: ColumnMappingDto,
    pub summary: ImportSummaryDto,
    pub sample: Vec<PlannedRowDto>,
    pub conflicts: Vec<ConflictDto>,
    pub errors: Vec<RowErrorDto>,
    pub warnings: Vec<PlanWarningDto>,
    pub removals: Vec<ContactRefDto>,
    /// Every row whose incoming address is on hold.
    pub on_hold: Vec<PlannedRowDto>,
    /// Email updates that replace an address on hold with one that is not.
    pub released: Vec<PlannedRowDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchDetailDto {
    pub batch: ImportBatchDto,
    pub members: PageDto<ContactDto>,
}

// ---------------------------------------------------------------------------
// Requests

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportInspectRequest {
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMappingInputDto {
    pub first_name: u32,
    pub last_name: u32,
    pub email: u32,
}

impl From<ColumnMappingInputDto> for ColumnMapping {
    fn from(mapping: ColumnMappingInputDto) -> Self {
        Self {
            first_name: mapping.first_name,
            last_name: mapping.last_name,
            email: mapping.email,
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewRequest {
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default)]
    pub mapping: Option<ColumnMappingInputDto>,
    pub scope: ImportScopeDto,
    #[serde(default)]
    pub progress_id: Option<String>,
    #[serde(default)]
    pub replaces_batch_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConflictActionDto {
    LinkContact,
    CreateNew,
    Skip,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionDto {
    pub row: u32,
    pub action: ConflictActionDto,
    #[serde(default)]
    pub contact_id: Option<i64>,
}

impl TryFrom<ConflictResolutionDto> for ConflictResolution {
    type Error = CoreError;

    fn try_from(resolution: ConflictResolutionDto) -> Result<Self, Self::Error> {
        let action = match (resolution.action, resolution.contact_id) {
            (ConflictActionDto::LinkContact, Some(contact_id)) => {
                ConflictAction::LinkContact { contact_id }
            }
            (ConflictActionDto::LinkContact, None) => {
                return Err(CoreError::validation(
                    "resolutions",
                    "Choose a contact for the linked row.",
                ));
            }
            (ConflictActionDto::CreateNew, None) => ConflictAction::CreateNew,
            (ConflictActionDto::Skip, None) => ConflictAction::Skip,
            (_, Some(_)) => {
                return Err(CoreError::validation(
                    "resolutions",
                    "A contact can only be linked with the linkContact action.",
                ));
            }
        };
        Ok(Self {
            row: resolution.row,
            action,
        })
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplyRequest {
    pub batch_id: i64,
    pub resolutions: Vec<ConflictResolutionDto>,
    #[serde(default)]
    pub progress_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportDiscardRequest {
    pub batch_id: i64,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportListRequest {
    #[serde(default)]
    pub status: Option<ImportStatusDto>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportGetRequest {
    pub batch_id: i64,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlanRowsRequest {
    pub batch_id: i64,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

// ---------------------------------------------------------------------------
// Commands

#[tauri::command]
#[specta::specta]
pub async fn imports_pick_source(
    state: State<'_, AppState>,
) -> Result<Option<SourceFileDto>, AppError> {
    pick_source(state.services()?, &state.picked_sources).await
}

#[tauri::command]
#[specta::specta]
pub async fn imports_inspect(
    state: State<'_, AppState>,
    request: ImportInspectRequest,
) -> Result<SourceInspectionDto, AppError> {
    inspect(state.services()?, &state.picked_sources, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn imports_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportPreviewRequest,
) -> Result<ImportPreviewDto, AppError> {
    let events = crate::events::AppEmitter {
        app: &app,
        bus: &state.events,
    };
    let emitted = preview(state.services()?, &state.picked_sources, &events, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn imports_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportApplyRequest,
) -> Result<ImportBatchDto, AppError> {
    let events = crate::events::AppEmitter {
        app: &app,
        bus: &state.events,
    };
    let emitted = apply(state.services()?, &events, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn imports_discard(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportDiscardRequest,
) -> Result<ImportBatchDto, AppError> {
    let emitted = discard(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn imports_list(
    state: State<'_, AppState>,
    request: ImportListRequest,
) -> Result<PageDto<ImportBatchDto>, AppError> {
    list(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn imports_get(
    state: State<'_, AppState>,
    request: ImportGetRequest,
) -> Result<ImportBatchDetailDto, AppError> {
    get(state.services()?, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn imports_plan_rows(
    state: State<'_, AppState>,
    request: ImportPlanRowsRequest,
) -> Result<PageDto<PlannedRowDto>, AppError> {
    plan_rows(state.services()?, request).await
}

// ---------------------------------------------------------------------------
// Bodies

pub(crate) async fn pick_source(
    services: &Services,
    picked: &PickedSources,
) -> Result<Option<SourceFileDto>, AppError> {
    let filter = FileFilter {
        name: "Spreadsheets".to_owned(),
        extensions: SUPPORTED_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
    };
    let Some(file) = services.dialogs.pick_file(&[filter]).await? else {
        return Ok(None);
    };
    register_source(picked, &file).map(Some)
}

/// Allowlist a file the user chose natively (picker or drop) and describe it. Only the
/// native layer may call this: the webview must never be able to add paths.
pub(crate) fn register_source(
    picked: &PickedSources,
    file: &Path,
) -> Result<SourceFileDto, AppError> {
    let canonical = validate_source_path(file, SUPPORTED_EXTENSIONS)?;
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CoreError::validation("path", "The file name is not valid UTF-8."))?
        .to_owned();
    let bytes = std::fs::metadata(&canonical).map_err(CoreError::io)?.len();
    let kind = kind_for(&canonical)?;

    picked
        .lock()
        .map_err(|_| CoreError::InvalidState("The file picker lock was poisoned.".to_owned()))?
        .insert(canonical.clone());

    Ok(SourceFileDto {
        path: canonical.display().to_string(),
        name,
        kind: kind.into(),
        bytes,
    })
}

/// What a native file drop on the window turns into: the first supported file is
/// allowlisted and announced as `SourceDropped`; otherwise `SourceRejected` says why.
pub(crate) fn dropped_event(picked: &PickedSources, paths: &[PathBuf]) -> Option<DomainEvent> {
    let first = paths.first()?;
    let mut rejection = None;
    for path in paths {
        match register_source(picked, path) {
            Ok(source) => return Some(DomainEvent::SourceDropped { source }),
            Err(err) => {
                rejection.get_or_insert(err);
            }
        }
    }
    let name = first
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let reason = rejection.map(|err| err.message).unwrap_or_default();
    Some(DomainEvent::SourceRejected { name, reason })
}

pub(crate) async fn inspect(
    _services: &Services,
    picked: &PickedSources,
    request: ImportInspectRequest,
) -> Result<SourceInspectionDto, AppError> {
    let path = ensure_picked(picked, Path::new(&request.path))?;
    let source = read_source(path, request.sheet).await?;
    let suggested = suggest(&source.headers);
    let sample_rows = source
        .rows
        .iter()
        .take(10)
        .map(|row| SampleRowDto {
            row: row.row,
            cells: row.cells.clone(),
        })
        .collect();
    Ok(SourceInspectionDto {
        name: source.name,
        kind: source.kind.into(),
        sheets: source.sheets,
        sheet_name: source.sheet_name,
        header_row: source.header_row,
        headers: source.headers,
        suggested_mapping: suggested.into(),
        sample_rows,
        row_count: u64::try_from(source.rows.len()).unwrap_or(u64::MAX),
        ignored_trailing: source.ignored_trailing,
    })
}

pub(crate) async fn preview(
    services: &Services,
    picked: &PickedSources,
    events: &dyn EventEmitter,
    request: ImportPreviewRequest,
) -> Result<Emitted<ImportPreviewDto>, AppError> {
    let progress_id = validate_progress_id(request.progress_id)?;
    let path = ensure_picked(picked, Path::new(&request.path))?;
    validate_source_path(&path, SUPPORTED_EXTENSIONS)?;

    if let Some(progress_id) = progress_id.as_ref() {
        events.emit(DomainEvent::ImportProgress {
            progress_id: progress_id.clone(),
            phase: ImportPhase::Reading.into(),
            processed: 0,
            total: 1,
        });
    }

    let source = read_source(path, request.sheet).await?;
    let options = PreviewOptions {
        mapping: request.mapping.map(ColumnMapping::from),
        scope: request.scope.into(),
    };

    let progress = ProgressEvents {
        events,
        progress_id,
    };
    let preview = services
        .imports
        .preview(&source, options, &progress)
        .await?;

    if let Some(batch_id) = request.replaces_batch_id {
        match services.imports.discard(batch_id).await {
            Ok(_) | Err(CoreError::InvalidState(_)) => {}
            Err(err) => {
                tracing::warn!(batch_id, error = %err, "could not replace the previous preview");
            }
        }
    }

    let (on_hold, released) = held_rows(services, preview.batch.id).await?;
    let dto = ImportPreviewDto {
        batch: preview.batch.into(),
        headers: preview.headers,
        header_row: preview.header_row,
        suggested_mapping: preview.suggested_mapping.into(),
        mapping: preview.mapping.into(),
        summary: preview.summary.into(),
        sample: rows_with_holds(services, preview.sample).await?,
        conflicts: preview
            .conflicts
            .into_iter()
            .map(ConflictDto::from)
            .collect(),
        errors: preview.errors.into_iter().map(RowErrorDto::from).collect(),
        warnings: preview
            .warnings
            .into_iter()
            .map(PlanWarningDto::from)
            .collect(),
        removals: preview
            .removals
            .into_iter()
            .map(|removal| removal.contact.into())
            .collect(),
        on_hold,
        released,
    };
    Ok(Emitted::one(dto, DomainEvent::ImportsChanged {}))
}

pub(crate) async fn apply(
    services: &Services,
    events: &dyn EventEmitter,
    request: ImportApplyRequest,
) -> Result<Emitted<ImportBatchDto>, AppError> {
    let progress_id = validate_progress_id(request.progress_id)?;
    let resolutions = request
        .resolutions
        .into_iter()
        .map(ConflictResolution::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(progress_id) = progress_id.as_ref() {
        events.emit(DomainEvent::ImportProgress {
            progress_id: progress_id.clone(),
            phase: ImportPhase::Applying.into(),
            processed: 0,
            total: 1,
        });
    }

    let batch = services
        .imports
        .apply(request.batch_id, resolutions)
        .await?;

    if let Some(progress_id) = progress_id {
        events.emit(DomainEvent::ImportProgress {
            progress_id,
            phase: ImportPhase::Applying.into(),
            processed: 1,
            total: 1,
        });
    }

    Ok(Emitted::new(
        batch.into(),
        vec![
            DomainEvent::ImportsChanged {},
            DomainEvent::ContactsChanged {},
            DomainEvent::ImportFinished {
                batch_id: request.batch_id,
                status: ImportStatus::Applied.into(),
            },
        ],
    ))
}

pub(crate) async fn discard(
    services: &Services,
    request: ImportDiscardRequest,
) -> Result<Emitted<ImportBatchDto>, AppError> {
    let batch = services.imports.discard(request.batch_id).await?;
    Ok(Emitted::new(
        batch.into(),
        vec![
            DomainEvent::ImportsChanged {},
            DomainEvent::ImportFinished {
                batch_id: request.batch_id,
                status: ImportStatus::Discarded.into(),
            },
        ],
    ))
}

pub(crate) async fn list(
    services: &Services,
    request: ImportListRequest,
) -> Result<PageDto<ImportBatchDto>, AppError> {
    let page = page(request.limit, request.offset);
    let status = request.status.map(ImportStatus::from);
    let batches = services.imports.list(status, &page).await?;
    Ok(batches.into())
}

pub(crate) async fn get(
    services: &Services,
    request: ImportGetRequest,
) -> Result<ImportBatchDetailDto, AppError> {
    let page = page(request.limit, request.offset);
    let batch = services.imports.get(request.batch_id).await?;
    let members = services.imports.members(request.batch_id, &page).await?;
    Ok(ImportBatchDetailDto {
        batch: batch.into(),
        members: crate::commands::contacts::page_with_holds(services, members).await?,
    })
}

pub(crate) async fn plan_rows(
    services: &Services,
    request: ImportPlanRowsRequest,
) -> Result<PageDto<PlannedRowDto>, AppError> {
    let page = page(request.limit, request.offset);
    let rows = services.imports.plan_rows(request.batch_id, &page).await?;
    Ok(PageDto {
        total: i64::try_from(rows.total).unwrap_or(i64::MAX),
        offset: i64::from(rows.offset),
        limit: i64::from(rows.limit),
        items: rows_with_holds(services, rows.items).await?,
    })
}

/// Planned rows with their email holds: the incoming address, and for email updates
/// the address it replaces.
async fn rows_with_holds(
    services: &Services,
    rows: Vec<PlannedRow>,
) -> Result<Vec<PlannedRowDto>, AppError> {
    let mut addresses = Vec::with_capacity(rows.len());
    for row in &rows {
        addresses.push(row.input.email.normalized.clone());
        if let PlanAction::UpdateEmail { target } = &row.action {
            addresses.push(target.email.to_lowercase());
        }
    }
    let holds = holds_for(services, addresses).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let email_hold = holds.get(&row.input.email.normalized).cloned();
            let previous_hold = match &row.action {
                PlanAction::UpdateEmail { target } => {
                    holds.get(&target.email.to_lowercase()).cloned()
                }
                _ => None,
            };
            PlannedRowDto {
                email_hold,
                previous_hold,
                ..row.into()
            }
        })
        .collect())
}

/// Every planned row whose new address is on hold, and every email update that
/// releases a hold. Reads the whole stored plan page by page.
async fn held_rows(
    services: &Services,
    batch_id: i64,
) -> Result<(Vec<PlannedRowDto>, Vec<PlannedRowDto>), AppError> {
    let mut on_hold = Vec::new();
    let mut released = Vec::new();
    let mut offset = 0;
    loop {
        let page = page(Some(u32::MAX), Some(offset));
        let rows = services.imports.plan_rows(batch_id, &page).await?;
        let fetched = u32::try_from(rows.items.len()).unwrap_or(u32::MAX);
        let total = rows.total;
        for row in rows_with_holds(services, rows.items).await? {
            if row.email_hold.is_some() {
                on_hold.push(row);
            } else if row.previous_hold.is_some() {
                released.push(row);
            }
        }
        offset = offset.saturating_add(fetched);
        if fetched == 0 || u64::from(offset) >= total {
            break;
        }
    }
    Ok((on_hold, released))
}

// ---------------------------------------------------------------------------
// Helpers

fn ensure_picked(picked: &PickedSources, path: &Path) -> Result<PathBuf, AppError> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| CoreError::validation("path", "Choose the file again"))?;
    let allowed = picked
        .lock()
        .map_err(|_| CoreError::InvalidState("The file picker lock was poisoned.".to_owned()))?;
    if !allowed.contains(&canonical) {
        return Err(CoreError::validation("path", "Choose the file again").into());
    }
    Ok(canonical)
}

async fn read_source(path: PathBuf, sheet: Option<String>) -> Result<ParsedSource, AppError> {
    tokio::task::spawn_blocking(move || {
        read_parsed(
            &path,
            &ReadOptions {
                sheet,
                ..ReadOptions::default()
            },
        )
    })
    .await
    .map_err(AppError::internal)?
    .map_err(AppError::from)
}

fn validate_progress_id(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(progress_id) = raw else {
        return Ok(None);
    };
    let valid = !progress_id.is_empty()
        && progress_id.len() <= 64
        && progress_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    if !valid {
        return Err(
            CoreError::validation("progressId", "must be 1-64 letters, digits or hyphens").into(),
        );
    }
    Ok(Some(progress_id))
}

/// Emits one progress event per report when the client supplied a correlation id.
struct ProgressEvents<'a> {
    events: &'a dyn EventEmitter,
    progress_id: Option<String>,
}

impl ProgressSink for ProgressEvents<'_> {
    fn report(&self, phase: ImportPhase, processed: u64, total: u64) {
        if let Some(progress_id) = self.progress_id.as_ref() {
            self.events.emit(DomainEvent::ImportProgress {
                progress_id: progress_id.clone(),
                phase: phase.into(),
                processed,
                total,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Mutex;

    use weasel_core::contact::ContactQuery;
    use weasel_import::mapping::suggest;

    use crate::commands::dto::ImportPhaseDto;
    use crate::commands::testing::harness;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<DomainEvent>>,
    }

    impl Recorder {
        fn take(&self) -> Vec<DomainEvent> {
            std::mem::take(&mut *self.events.lock().unwrap())
        }
    }

    impl EventEmitter for Recorder {
        fn emit(&self, event: DomainEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn write_csv(harness: &crate::commands::testing::Harness, name: &str, body: &str) -> PathBuf {
        let path = harness.db.path().join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    async fn pick(harness: &crate::commands::testing::Harness, path: &Path) -> SourceFileDto {
        harness.dialogs.push(path);
        pick_source(&harness.services, &harness.picked)
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn a_dropped_spreadsheet_is_allowlisted_and_announced() {
        let harness = harness().await;
        let csv = write_csv(
            &harness,
            "dropped.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let notes = write_csv(&harness, "notes.txt", "not a sheet");

        let event = dropped_event(&harness.picked, &[notes.clone(), csv.clone()]).unwrap();
        let DomainEvent::SourceDropped { source } = event else {
            panic!("expected SourceDropped, got {event:?}");
        };
        assert_eq!(source.name, "dropped.csv");
        let inspection = inspect(
            &harness.services,
            &harness.picked,
            ImportInspectRequest {
                path: source.path,
                sheet: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(inspection.row_count, 1);
    }

    #[tokio::test]
    async fn an_unsupported_drop_is_rejected_and_not_allowlisted() {
        let harness = harness().await;
        let notes = write_csv(&harness, "notes.txt", "not a sheet");
        let event = dropped_event(&harness.picked, std::slice::from_ref(&notes)).unwrap();
        let DomainEvent::SourceRejected { name, reason } = event else {
            panic!("expected SourceRejected, got {event:?}");
        };
        assert_eq!(name, "notes.txt");
        assert!(!reason.is_empty());
        assert!(ensure_picked(&harness.picked, &notes).is_err());
        assert!(dropped_event(&harness.picked, &[]).is_none());
    }

    #[tokio::test]
    async fn only_files_from_the_dialog_can_be_read() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let err = inspect(
            &harness.services,
            &harness.picked,
            ImportInspectRequest {
                path: path.display().to_string(),
                sheet: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Validation);
        assert_eq!(
            err.details
                .and_then(|details| details["field"].as_str().map(str::to_owned)),
            Some("path".to_owned())
        );

        let file = pick(&harness, &path).await;
        assert_eq!(file.name, "roster.csv");
        assert_eq!(file.kind, SourceKindDto::Csv);
        assert!(file.bytes > 0);
        assert_eq!(harness.dialogs.seen_filters()[0][0].name, "Spreadsheets");
        assert_eq!(
            harness.dialogs.seen_filters()[0][0].extensions,
            SUPPORTED_EXTENSIONS
                .iter()
                .map(|extension| (*extension).to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn a_cancelled_pick_is_null() {
        let harness = harness().await;
        harness.dialogs.push_cancel();
        assert!(
            pick_source(&harness.services, &harness.picked)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn inspect_reports_headers_without_creating_a_draft() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "incomplete.csv",
            "First Name,Last Name\nAda,Lovelace\nGrace,Hopper\n",
        );
        let picked = pick(&harness, &path).await;
        let inspection = inspect(
            &harness.services,
            &harness.picked,
            ImportInspectRequest {
                path: picked.path,
                sheet: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(inspection.headers, ["First Name", "Last Name"]);
        assert_eq!(inspection.suggested_mapping.first_name, Some(0));
        assert_eq!(inspection.suggested_mapping.last_name, Some(1));
        assert_eq!(inspection.suggested_mapping.email, None);
        assert_eq!(inspection.sample_rows.len(), 2);
        assert_eq!(inspection.row_count, 2);
        assert_eq!(harness.services.imports.count_drafts().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn preview_creates_a_draft_and_announces_it() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let preview = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: Some(ColumnMappingInputDto {
                    first_name: 0,
                    last_name: 1,
                    email: 2,
                }),
                scope: ImportScopeDto::Roster,
                progress_id: Some("preview-1".to_owned()),
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(preview.value.batch.status, ImportStatusDto::Draft);
        assert_eq!(preview.value.summary.added, 1);
        assert_eq!(harness.services.imports.count_drafts().await.unwrap(), 1);
        assert_eq!(preview.events, vec![DomainEvent::ImportsChanged {}]);
        let events = recorder.take();
        assert!(matches!(
            events.first(),
            Some(DomainEvent::ImportProgress {
                phase: ImportPhaseDto::Reading,
                processed: 0,
                total: 1,
                ..
            })
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            DomainEvent::ImportProgress {
                phase: ImportPhaseDto::Matching,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn replacing_a_preview_discards_the_old_draft() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let first = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path.clone(),
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        let second = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: Some(first.value.batch.id),
            },
        )
        .await
        .unwrap();
        assert_ne!(first.value.batch.id, second.value.batch.id);
        assert_eq!(harness.services.imports.count_drafts().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn unresolved_conflicts_block_apply_before_events() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "dupes.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let preview = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(preview.value.conflicts.len(), 1);
        let err = apply(
            &harness.services,
            &recorder,
            ImportApplyRequest {
                batch_id: preview.value.batch.id,
                resolutions: Vec::new(),
                progress_id: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::InvalidState);
        assert!(recorder.take().is_empty());
        assert_eq!(harness.services.imports.count_drafts().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn apply_emits_import_then_contacts_then_finished() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let preview = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        let applied = apply(
            &harness.services,
            &recorder,
            ImportApplyRequest {
                batch_id: preview.value.batch.id,
                resolutions: Vec::new(),
                progress_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(applied.value.status, ImportStatusDto::Applied);
        assert_eq!(
            applied.events,
            vec![
                DomainEvent::ImportsChanged {},
                DomainEvent::ContactsChanged {},
                DomainEvent::ImportFinished {
                    batch_id: preview.value.batch.id,
                    status: ImportStatusDto::Applied,
                },
            ]
        );
    }

    #[tokio::test]
    async fn link_contact_requires_a_contact() {
        let err = ConflictResolution::try_from(ConflictResolutionDto {
            row: 2,
            action: ConflictActionDto::LinkContact,
            contact_id: None,
        })
        .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Validation {
                field: "resolutions",
                ..
            }
        ));
        let err = ConflictResolution::try_from(ConflictResolutionDto {
            row: 2,
            action: ConflictActionDto::Skip,
            contact_id: Some(4),
        })
        .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Validation {
                field: "resolutions",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn plan_rows_are_only_available_for_a_draft() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let preview = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        let batch_id = preview.value.batch.id;
        discard(&harness.services, ImportDiscardRequest { batch_id })
            .await
            .unwrap();
        let err = plan_rows(
            &harness.services,
            ImportPlanRowsRequest {
                batch_id,
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn re_previewing_the_same_file_warns_and_finds_nothing_to_do() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let first = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path.clone(),
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        apply(
            &harness.services,
            &recorder,
            ImportApplyRequest {
                batch_id: first.value.batch.id,
                resolutions: Vec::new(),
                progress_id: None,
            },
        )
        .await
        .unwrap();
        let again = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(again.value.summary.added, 0);
        assert_eq!(again.value.summary.updated, 0);
        assert_eq!(again.value.summary.removed, 0);
        assert_eq!(again.value.summary.unchanged, 1);
        assert!(matches!(
            again.value.warnings.as_slice(),
            [PlanWarningDto::SameFileAsBatch { batch_id }] if *batch_id == first.value.batch.id
        ));
    }

    #[tokio::test]
    async fn members_and_rows_page_through_the_stored_plan() {
        let harness = harness().await;
        let path = write_csv(
            &harness,
            "roster.csv",
            "First Name,Last Name,Email\nAda,Lovelace,ada@example.edu\nGrace,Hopper,grace@example.edu\n",
        );
        let picked = pick(&harness, &path).await;
        let recorder = Recorder::default();
        let preview = preview(
            &harness.services,
            &harness.picked,
            &recorder,
            ImportPreviewRequest {
                path: picked.path,
                sheet: None,
                mapping: None,
                scope: ImportScopeDto::Roster,
                progress_id: None,
                replaces_batch_id: None,
            },
        )
        .await
        .unwrap();
        apply(
            &harness.services,
            &recorder,
            ImportApplyRequest {
                batch_id: preview.value.batch.id,
                resolutions: Vec::new(),
                progress_id: None,
            },
        )
        .await
        .unwrap();

        let detail = get(
            &harness.services,
            ImportGetRequest {
                batch_id: preview.value.batch.id,
                limit: Some(1),
                offset: Some(0),
            },
        )
        .await
        .unwrap();
        assert_eq!(detail.members.total, 2);
        assert_eq!(detail.members.items.len(), 1);

        let rows = plan_rows(
            &harness.services,
            ImportPlanRowsRequest {
                batch_id: preview.value.batch.id,
                limit: Some(1),
                offset: Some(1),
            },
        )
        .await
        .unwrap();
        assert_eq!(rows.total, 2);
        assert_eq!(rows.items.len(), 1);
        assert_eq!(rows.items[0].row, 3);

        let contacts = harness
            .services
            .contacts
            .list(&ContactQuery::default())
            .await
            .unwrap();
        assert_eq!(contacts.total, 2);
    }

    #[test]
    fn suggestions_come_from_the_shared_header_analyzer() {
        let headers = vec![
            "E-Mail".to_owned(),
            "Vorname".to_owned(),
            "Nachname".to_owned(),
        ];
        assert_eq!(suggest(&headers).email, Some(0));
    }
}
