//! Preview, apply and review import batches.

use std::sync::Arc;

use weasel_core::enums::{ContactStatus, ImportScope, ImportStatus};
use weasel_core::identity::IdentityStrategy;
use weasel_core::import::{
    ColumnMapping, Conflict, ConflictResolution, ImportBatch, ImportPlan, ImportSummary,
    NewImportBatch, PLAN_VERSION, PlanWarning, PlannedRow, RowError,
};
use weasel_core::ports::{Clock, ImportRepository, ProgressSink};
use weasel_core::query::{Page, PageRequest};
use weasel_core::{CoreError, CoreResult};

use crate::mapping::{self, SuggestedMapping};
use crate::normalize::normalize;
use crate::reconcile;
use crate::source::ParsedSource;

/// The user's explicit choices for one preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewOptions {
    pub mapping: Option<ColumnMapping>,
    pub scope: ImportScope,
}

/// Everything the review screen needs after reading and planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub batch: ImportBatch,
    pub headers: Vec<String>,
    pub header_row: u32,
    pub suggested_mapping: SuggestedMapping,
    pub mapping: ColumnMapping,
    pub summary: ImportSummary,
    pub sample: Vec<PlannedRow>,
    pub conflicts: Vec<Conflict>,
    pub errors: Vec<RowError>,
    pub warnings: Vec<PlanWarning>,
}

pub struct ImportService {
    imports: Arc<dyn ImportRepository>,
    clock: Arc<dyn Clock>,
    identity: Arc<dyn IdentityStrategy>,
}

impl ImportService {
    pub fn new(
        imports: Arc<dyn ImportRepository>,
        clock: Arc<dyn Clock>,
        identity: Arc<dyn IdentityStrategy>,
    ) -> Self {
        Self {
            imports,
            clock,
            identity,
        }
    }

    pub async fn count_drafts(&self) -> CoreResult<u64> {
        self.imports.count_drafts().await
    }

    pub async fn preview(
        &self,
        source: &ParsedSource,
        options: PreviewOptions,
        progress: &dyn ProgressSink,
    ) -> CoreResult<ImportPreview> {
        let suggested_mapping = mapping::suggest(&source.headers);
        let width = source
            .rows
            .iter()
            .map(|row| row.cells.len())
            .chain(std::iter::once(source.headers.len()))
            .max()
            .unwrap_or(0);
        let mapping = mapping::resolve(options.mapping, suggested_mapping, width)?;

        let (rows, errors) = normalize(source, &mapping, self.identity.as_ref());
        let snapshot = self.imports.snapshot().await?;
        let mut plan = reconcile::plan(
            &rows,
            &errors,
            &snapshot,
            options.scope,
            &mapping,
            self.identity.id(),
            progress,
        )?;

        if let Some(batch_id) = self
            .imports
            .last_applied_with_hash(&source.file_hash)
            .await?
        {
            plan.warnings
                .push(PlanWarning::SameFileAsBatch { batch_id });
        }

        let now_ms = self.clock.now_ms();
        let draft = NewImportBatch {
            source_kind: source.kind,
            source_name: source.name.clone(),
            source_path: Some(source.path.display().to_string()),
            file_hash: Some(source.file_hash.clone()),
            sheet_name: source.sheet_name.clone(),
            scope: options.scope,
            summary: plan.summary.clone(),
            plan: plan.clone(),
        };
        let batch = self.imports.create_draft(draft, now_ms).await?;

        let sample = plan.rows.iter().take(100).cloned().collect();
        Ok(ImportPreview {
            batch,
            headers: source.headers.clone(),
            header_row: source.header_row,
            suggested_mapping,
            mapping,
            summary: plan.summary,
            sample,
            conflicts: plan.conflicts,
            errors: plan.errors,
            warnings: plan.warnings,
        })
    }

    pub async fn apply(
        &self,
        batch_id: i64,
        resolutions: Vec<ConflictResolution>,
    ) -> CoreResult<ImportBatch> {
        let batch = self.imports.get(batch_id).await?;
        if batch.status != ImportStatus::Draft {
            return Err(CoreError::InvalidState(
                "Only draft imports can be applied".to_owned(),
            ));
        }
        let plan = self.imports.plan(batch_id).await?;
        if plan.version != PLAN_VERSION {
            return Err(CoreError::InvalidState("Preview the file again".to_owned()));
        }
        let ops = reconcile::resolve(&plan, &resolutions)?;
        let summary = summary_for(&plan, &ops);
        let now_ms = self.clock.now_ms();
        self.imports.apply(batch_id, ops, summary, now_ms).await
    }

    pub async fn discard(&self, batch_id: i64) -> CoreResult<ImportBatch> {
        self.imports.discard(batch_id, self.clock.now_ms()).await
    }

    pub async fn list(
        &self,
        status: Option<ImportStatus>,
        page: &PageRequest,
    ) -> CoreResult<Page<ImportBatch>> {
        self.imports.list(status, page).await
    }

    pub async fn get(&self, batch_id: i64) -> CoreResult<ImportBatch> {
        self.imports.get(batch_id).await
    }

    pub async fn members(
        &self,
        batch_id: i64,
        page: &PageRequest,
    ) -> CoreResult<Page<weasel_core::contact::Contact>> {
        self.imports.members(batch_id, page).await
    }

    /// Pages the stored plan so review can resume where it stopped.
    pub async fn plan_rows(
        &self,
        batch_id: i64,
        page: &PageRequest,
    ) -> CoreResult<Page<PlannedRow>> {
        let plan = self.imports.plan(batch_id).await?;
        let total = plan.rows.len() as u64;
        let start = (page.offset as usize).min(plan.rows.len());
        let end = start
            .saturating_add(page.limit as usize)
            .min(plan.rows.len());
        Ok(Page {
            items: plan.rows[start..end].to_vec(),
            total,
            limit: page.limit,
            offset: page.offset,
        })
    }
}

fn summary_for(plan: &ImportPlan, ops: &[weasel_core::import::ContactOp]) -> ImportSummary {
    let mut added = 0u64;
    let mut updated = 0u64;
    let mut removed = 0u64;
    for op in ops {
        match op {
            weasel_core::import::ContactOp::Create { .. } => added += 1,
            weasel_core::import::ContactOp::Modify { status, .. } => {
                if *status == Some(ContactStatus::Removed) {
                    removed += 1;
                } else {
                    updated += 1;
                }
            }
        }
    }
    ImportSummary {
        row_count: plan.rows.len() as u64,
        added,
        updated,
        removed,
        conflicts: plan.conflicts.len() as u64,
        unchanged: plan
            .rows
            .iter()
            .filter(|row| matches!(row.action, weasel_core::import::PlanAction::Noop { .. }))
            .count() as u64,
        invalid: plan.errors.len() as u64,
    }
}
