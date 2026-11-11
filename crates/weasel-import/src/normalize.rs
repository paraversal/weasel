//! Normalization: mapped cells become validated domain rows and row errors.

use weasel_core::{
    CoreError,
    email::EmailAddress,
    identity::{IdentityStrategy, clean_name},
    import::{ColumnMapping, NormalizedRow, RowError},
};

use crate::source::ParsedSource;

/// Validate every data row. Rows whose mapped cells are all blank are skipped.
///
/// A row produces at most one error: an unusable name or email. Email errors
/// carry the identity key when both names are valid, so a roster import can
/// protect that person from removal.
pub fn normalize(
    source: &ParsedSource,
    mapping: &ColumnMapping,
    identity: &dyn IdentityStrategy,
) -> (Vec<NormalizedRow>, Vec<RowError>) {
    let mut rows = Vec::new();
    let mut errors = Vec::new();

    for raw in &source.rows {
        let first_raw = cell(raw, mapping.first_name);
        let last_raw = cell(raw, mapping.last_name);
        let email_raw = cell(raw, mapping.email);

        if first_raw.trim().is_empty() && last_raw.trim().is_empty() && email_raw.trim().is_empty()
        {
            continue;
        }

        let first_name = clean_name(first_raw);
        if first_name.is_empty() {
            errors.push(RowError {
                row: raw.row,
                field: "firstName".to_owned(),
                reason: "The first name is empty.".to_owned(),
                identity_key: None,
            });
            continue;
        }
        let last_name = clean_name(last_raw);
        if last_name.is_empty() {
            errors.push(RowError {
                row: raw.row,
                field: "lastName".to_owned(),
                reason: "The last name is empty.".to_owned(),
                identity_key: None,
            });
            continue;
        }

        let identity_key = identity.key(&first_name, &last_name);
        match EmailAddress::parse(email_raw) {
            Ok(email) => rows.push(NormalizedRow {
                row: raw.row,
                first_name,
                last_name,
                email,
                identity_key,
            }),
            Err(CoreError::Validation { reason, .. }) => errors.push(RowError {
                row: raw.row,
                field: "email".to_owned(),
                reason,
                identity_key,
            }),
            Err(other) => errors.push(RowError {
                row: raw.row,
                field: "email".to_owned(),
                reason: other.to_string(),
                identity_key,
            }),
        }
    }

    (rows, errors)
}

fn cell(row: &crate::source::RawRow, column: u32) -> &str {
    row.cells
        .get(column as usize)
        .map(String::as_str)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::PathBuf;

    use weasel_core::{
        enums::SourceKind,
        identity::NameIdentity,
        import::ColumnMapping,
    };

    use super::*;
    use crate::source::{ParsedSource, RawRow};

    fn source(rows: Vec<(u32, Vec<&str>)>) -> ParsedSource {
        ParsedSource {
            kind: SourceKind::Csv,
            name: "roster.csv".to_owned(),
            path: PathBuf::from("roster.csv"),
            file_hash: "00".to_owned(),
            sheet_name: None,
            sheets: Vec::new(),
            header_row: 1,
            headers: vec!["VORNAME".to_owned(), "NACHNAME".to_owned(), "EMAIL".to_owned()],
            rows: rows
                .into_iter()
                .map(|(row, cells)| RawRow {
                    row,
                    cells: cells.into_iter().map(str::to_owned).collect(),
                })
                .collect(),
            ignored_trailing: 0,
        }
    }

    fn mapping() -> ColumnMapping {
        ColumnMapping {
            first_name: 0,
            last_name: 1,
            email: 2,
        }
    }

    #[test]
    fn normalizes_skips_blanks_and_reports_errors() {
        let source = source(vec![
            (2, vec!["  Ada ", "Lovelace", " Ada@Example.edu "]),
            (3, vec!["", "", ""]),
            (4, vec!["", "Baker", "bob@example.edu"]),
            (5, vec!["Cid", "", "cid@example.edu"]),
            (6, vec!["Dee", "Doe", "not-an-email"]),
            (7, vec!["Eve", "Evans", ""]),
        ]);
        let (rows, errors) = normalize(&source, &mapping(), &NameIdentity);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].first_name, "Ada");
        assert_eq!(rows[0].email.display, "Ada@Example.edu");
        assert_eq!(rows[0].email.normalized, "ada@example.edu");
        assert!(rows[0].identity_key.is_some());

        assert_eq!(errors.len(), 4);
        assert_eq!(errors[0].field, "firstName");
        assert_eq!(errors[1].field, "lastName");
        assert_eq!(errors[2].field, "email");
        assert_eq!(errors[2].row, 6);
        assert!(errors[2].identity_key.is_some());
        assert_eq!(errors[3].field, "email");
        assert_eq!(errors[3].row, 7);
        assert!(errors[3].identity_key.is_some());
    }
}
