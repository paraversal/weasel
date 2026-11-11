//! Invalid-address commands: wildcard rules, per-address verdicts and bounces.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use weasel_core::FeatureDescriptor;
use weasel_core::email_check::{
    EmailFlag, EmailHold, EmailVerdict, FlagReason, InvalidRule, RulePreview,
};

use crate::commands::dto::{Emitted, finish};
use crate::commands::send_batches::SendRecordDto;
use crate::error::AppError;
use crate::events::DomainEvent;
use crate::state::{AppState, Services};

/// Why an address is on hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EmailHoldDto {
    Manual,
    Bounced,
    Rule { pattern: String },
}

impl From<EmailHold> for EmailHoldDto {
    fn from(hold: EmailHold) -> Self {
        match hold {
            EmailHold::Manual => Self::Manual,
            EmailHold::Bounced => Self::Bounced,
            EmailHold::Rule { pattern } => Self::Rule { pattern },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InvalidRuleDto {
    pub id: i64,
    pub pattern: String,
    pub created_at: i64,
    pub matches: u64,
}

impl From<InvalidRule> for InvalidRuleDto {
    fn from(rule: InvalidRule) -> Self {
        Self {
            id: rule.id,
            pattern: rule.pattern,
            created_at: rule.created_at,
            matches: rule.matches,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RulePreviewDto {
    pub pattern: String,
    pub matches: u64,
    pub examples: Vec<String>,
}

impl From<RulePreview> for RulePreviewDto {
    fn from(preview: RulePreview) -> Self {
        Self {
            pattern: preview.pattern,
            matches: preview.matches,
            examples: preview.examples,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmailFlagDto {
    pub email: String,
    /// `false` means "marked valid": an override for a matching rule.
    pub invalid: bool,
    pub bounced: bool,
    pub created_at: i64,
    pub contacts: u64,
}

impl From<EmailFlag> for EmailFlagDto {
    fn from(flag: EmailFlag) -> Self {
        Self {
            email: flag.email,
            invalid: flag.verdict == EmailVerdict::Invalid,
            bounced: flag.reason == FlagReason::Bounced,
            created_at: flag.created_at,
            contacts: flag.contacts,
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmailRuleRequest {
    pub pattern: String,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmailRuleDeleteRequest {
    pub id: i64,
}

/// `Invalid` holds the address; `Valid` releases it (overriding rules); `Default`
/// forgets any verdict so rules apply again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EmailMarkDto {
    Invalid,
    Valid,
    Default,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmailMarkRequest {
    pub email: String,
    pub mark: EmailMarkDto,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SendRecordBounceRequest {
    pub record_id: i64,
    pub bounced: bool,
}

pub fn descriptor() -> FeatureDescriptor {
    FeatureDescriptor {
        id: "email_checks",
        title: "Invalid addresses",
        commands: &[
            "email_rules_list",
            "email_rules_add",
            "email_rules_delete",
            "email_rules_preview",
            "email_flags_list",
            "email_mark",
            "send_records_set_bounced",
        ],
        events: &["emailChecksChanged"],
    }
}

#[tauri::command]
#[specta::specta]
pub async fn email_rules_list(state: State<'_, AppState>) -> Result<Vec<InvalidRuleDto>, AppError> {
    let rules = state.services()?.email_checks.rules().await?;
    Ok(rules.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn email_rules_add(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmailRuleRequest,
) -> Result<InvalidRuleDto, AppError> {
    let emitted = add_rule(state.services()?, request).await?;
    Ok(finish(&app, &state, emitted))
}

#[tauri::command]
#[specta::specta]
pub async fn email_rules_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmailRuleDeleteRequest,
) -> Result<(), AppError> {
    state
        .services()?
        .email_checks
        .delete_rule(request.id)
        .await?;
    finish(&app, &state, changed(()));
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn email_rules_preview(
    state: State<'_, AppState>,
    request: EmailRuleRequest,
) -> Result<RulePreviewDto, AppError> {
    let preview = state
        .services()?
        .email_checks
        .preview_rule(&request.pattern)
        .await?;
    Ok(preview.into())
}

#[tauri::command]
#[specta::specta]
pub async fn email_flags_list(state: State<'_, AppState>) -> Result<Vec<EmailFlagDto>, AppError> {
    let flags = state.services()?.email_checks.flags().await?;
    Ok(flags.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn email_mark(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmailMarkRequest,
) -> Result<(), AppError> {
    let emitted = mark(state.services()?, request).await?;
    finish(&app, &state, emitted);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn send_records_set_bounced(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SendRecordBounceRequest,
) -> Result<SendRecordDto, AppError> {
    let record = state
        .services()?
        .email_checks
        .set_bounced(request.record_id, request.bounced)
        .await?;
    Ok(finish(&app, &state, changed(record.into())))
}

// ---------------------------------------------------------------------------
// Bodies

pub(crate) async fn add_rule(
    services: &Services,
    request: EmailRuleRequest,
) -> Result<Emitted<InvalidRuleDto>, AppError> {
    let rule = services.email_checks.add_rule(&request.pattern).await?;
    Ok(changed(rule.into()))
}

pub(crate) async fn mark(
    services: &Services,
    request: EmailMarkRequest,
) -> Result<Emitted<()>, AppError> {
    let checks = &services.email_checks;
    match request.mark {
        EmailMarkDto::Invalid => checks.mark_invalid(&request.email).await?,
        EmailMarkDto::Valid => checks.mark_valid(&request.email).await?,
        EmailMarkDto::Default => checks.clear_flag(&request.email).await?,
    }
    Ok(changed(()))
}

/// Holds for a set of normalized addresses, as DTOs.
pub(crate) async fn holds_for(
    services: &Services,
    normalized: Vec<String>,
) -> Result<HashMap<String, EmailHoldDto>, AppError> {
    let holds = services.email_checks.holds(&normalized).await?;
    Ok(holds
        .into_iter()
        .map(|(address, hold)| (address, hold.into()))
        .collect())
}

/// Any verdict, rule or bounce change can move people in or out of every list.
fn changed<T>(value: T) -> Emitted<T> {
    Emitted::one(value, DomainEvent::EmailChecksChanged {})
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::commands::testing::harness;

    #[tokio::test]
    async fn marks_round_trip_and_announce_a_change() {
        let harness = harness().await;
        let services = &harness.services;
        let emitted = add_rule(
            services,
            EmailRuleRequest {
                pattern: "*@temp.example.org".to_owned(),
            },
        )
        .await
        .unwrap();
        assert_eq!(emitted.events, vec![DomainEvent::EmailChecksChanged {}]);

        let holds = holds_for(services, vec!["ada@temp.example.org".to_owned()])
            .await
            .unwrap();
        assert_eq!(
            holds.get("ada@temp.example.org"),
            Some(&EmailHoldDto::Rule {
                pattern: "*@temp.example.org".to_owned()
            })
        );

        mark(
            services,
            EmailMarkRequest {
                email: "Ada@temp.example.org".to_owned(),
                mark: EmailMarkDto::Valid,
            },
        )
        .await
        .unwrap();
        let holds = holds_for(services, vec!["ada@temp.example.org".to_owned()])
            .await
            .unwrap();
        assert!(holds.is_empty());
    }

    #[tokio::test]
    async fn a_bad_pattern_is_a_validation_error() {
        let harness = harness().await;
        let error = add_rule(
            &harness.services,
            EmailRuleRequest {
                pattern: "*".to_owned(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Validation);
    }
}
