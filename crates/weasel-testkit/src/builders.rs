//! Fixture builders shared by integration tests and the seed binary.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use weasel_core::campaign::NewCampaign;
use weasel_core::contact::NewContact;
use weasel_core::enums::SourceKind;
use weasel_import::{ParsedSource, RawRow};

/// A manual or import contact without the database concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactSeed {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub notes: String,
}

impl ContactSeed {
    pub fn new(first_name: &str, last_name: &str, email: &str) -> Self {
        Self {
            first_name: first_name.to_owned(),
            last_name: last_name.to_owned(),
            email: email.to_owned(),
            notes: String::new(),
        }
    }

    pub fn with_notes(mut self, notes: &str) -> Self {
        self.notes = notes.to_owned();
        self
    }

    pub fn into_new_contact(self) -> NewContact {
        NewContact {
            first_name: self.first_name,
            last_name: self.last_name,
            email: self.email,
            notes: self.notes,
        }
    }
}

/// A campaign fixture with the same defaults a fresh draft has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignSeed {
    pub name: String,
    pub description: String,
    pub subject: String,
    pub body: String,
}

impl CampaignSeed {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            description: String::new(),
            subject: format!("{name} subject"),
            body: format!("{name} body"),
        }
    }

    pub fn into_new_campaign(self) -> NewCampaign {
        NewCampaign {
            name: self.name,
            description: self.description,
            subject: self.subject,
            body: self.body,
        }
    }
}

/// Builds an in-memory CSV-shaped source. The hash is deterministic for the rows.
pub fn parsed_source(rows: &[(&str, &str, &str)]) -> ParsedSource {
    parsed_source_named("fixture.csv", rows)
}

pub fn parsed_source_named(name: &str, rows: &[(&str, &str, &str)]) -> ParsedSource {
    let owned: Vec<(String, String, String)> = rows
        .iter()
        .map(|(first, last, email)| ((*first).to_owned(), (*last).to_owned(), (*email).to_owned()))
        .collect();
    parsed_source_owned(name, &owned)
}

pub fn parsed_source_owned(name: &str, rows: &[(String, String, String)]) -> ParsedSource {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    rows.hash(&mut hasher);
    let file_hash = format!("{:016x}", hasher.finish());

    ParsedSource {
        kind: SourceKind::Csv,
        name: name.to_owned(),
        path: PathBuf::from(format!("/tmp/{name}")),
        file_hash,
        sheet_name: None,
        sheets: Vec::new(),
        header_row: 1,
        headers: vec![
            "VORNAME".to_owned(),
            "NACHNAME".to_owned(),
            "EMAIL".to_owned(),
        ],
        rows: rows
            .iter()
            .enumerate()
            .map(|(index, (first, last, email))| RawRow {
                row: index as u32 + 2,
                cells: vec![first.clone(), last.clone(), email.clone()],
            })
            .collect(),
        ignored_trailing: 0,
    }
}
