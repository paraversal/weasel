//! Source readers against committed fixtures and generated workbooks.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use rust_xlsxwriter::Workbook;
use tempfile::TempDir;
use weasel_core::{CoreError, enums::SourceKind, identity::NameIdentity, import::ColumnMapping};
use weasel_import::{
    ReadOptions,
    mapping::{resolve, suggest},
    normalize::normalize,
    read_source,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> weasel_import::ParsedSource {
    read_source(&fixture(name), &ReadOptions::default()).unwrap()
}

fn cells(source: &weasel_import::ParsedSource) -> Vec<Vec<&str>> {
    source
        .rows
        .iter()
        .map(|row| row.cells.iter().map(String::as_str).collect())
        .collect()
}

#[test]
fn comma_fixture_is_read_with_headers_and_rows() {
    let source = read("comma.csv");
    assert_eq!(source.kind, SourceKind::Csv);
    assert_eq!(source.name, "comma.csv");
    assert_eq!(source.header_row, 1);
    assert_eq!(source.headers, ["VORNAME", "NACHNAME", "EMAIL"]);
    assert_eq!(
        cells(&source),
        vec![
            vec!["Ada", "Lovelace", "ada@example.edu"],
            vec!["Bob", "Baker", "bob@example.edu"],
        ]
    );
    assert_eq!(source.rows[0].row, 2);
    assert_eq!(source.rows[1].row, 3);
    assert_eq!(source.ignored_trailing, 0);
    assert_eq!(source.file_hash.len(), 64);
    assert!(source.sheet_name.is_none());
}

#[test]
fn semicolon_with_bom_and_footer() {
    let source = read("semicolon_bom.csv");
    assert_eq!(source.headers, ["VORNAME", "NACHNAME", "EMAIL"]);
    assert_eq!(
        cells(&source),
        vec![
            vec!["Ada", "Lovelace", "ada@example.edu"],
            vec!["Bob", "Baker", "bob@example.edu"],
        ]
    );
    assert_eq!(source.ignored_trailing, 1);
    assert_eq!(
        source.rows[1].row, 4,
        "blank lines still advance row numbers"
    );
}

#[test]
fn tab_fixture_sniffs_the_tab() {
    let source = read("tab.csv");
    assert_eq!(source.headers, ["VORNAME", "NACHNAME", "EMAIL"]);
    assert_eq!(source.rows.len(), 2);
    assert_eq!(source.rows[0].cells[2], "ada@example.edu");
}

#[test]
fn trailing_footer_is_ignored_but_counted() {
    let source = read("footer.csv");
    assert_eq!(source.rows.len(), 2);
    assert_eq!(source.ignored_trailing, 1);
}

#[test]
fn whitespace_case_duplicates_and_bad_emails_survive_to_normalization() {
    let source = read("variations.csv");
    assert_eq!(source.headers, ["Vorname", "Nachname", "E-Mail"]);
    assert_eq!(source.rows.len(), 4);

    let suggested = suggest(&source.headers);
    assert_eq!(
        suggested,
        weasel_import::SuggestedMapping {
            first_name: Some(0),
            last_name: Some(1),
            email: Some(2),
        }
    );
    let width = source
        .rows
        .iter()
        .map(|row| row.cells.len())
        .chain([source.headers.len()])
        .max()
        .unwrap();
    let mapping = resolve(None, suggested, width).unwrap();
    let (rows, errors) = normalize(&source, &mapping, &NameIdentity);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].email.normalized, "ada@example.edu");
    assert_eq!(rows[0].first_name, "ada");
    assert_eq!(rows[0].last_name, "LOVELACE");
    assert_eq!(rows[1].row, 3);
    assert_eq!(rows[2].row, 4);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "email");
    assert_eq!(errors[0].row, 5);
}

#[test]
fn non_utf8_csv_is_refused_with_advice() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("latin1.csv");
    std::fs::write(
        &path,
        b"VORNAME;NACHNAME;EMAIL\nJos\xe9;M\xfcller;j@example.edu\n",
    )
    .unwrap();
    let err = read_source(&path, &ReadOptions::default()).unwrap_err();
    match err {
        CoreError::Import { reason, .. } => {
            assert!(reason.contains("Save the file as CSV UTF-8"), "{reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn size_and_row_limits_carry_numbers() {
    let path = fixture("comma.csv");
    let err = read_source(
        &path,
        &ReadOptions {
            max_bytes: 10,
            ..ReadOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, CoreError::Import { reason, .. } if reason.contains("limit")),
        "size limit"
    );

    let err = read_source(
        &path,
        &ReadOptions {
            max_rows: 1,
            ..ReadOptions::default()
        },
    )
    .unwrap_err();
    match err {
        CoreError::Import { reason, .. } => {
            assert!(reason.contains('2') && reason.contains('1'), "{reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unknown_extension_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roster.txt");
    std::fs::write(&path, "VORNAME,NACHNAME,EMAIL\n").unwrap();
    let err = read_source(&path, &ReadOptions::default()).unwrap_err();
    assert!(matches!(err, CoreError::Import { .. }));
}

fn workbook(path: &Path) {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Roster").unwrap();
    sheet.write_string(0, 0, "Roster 2026").unwrap();
    for (column, header) in ["Vorname", "Nachname", "E-Mail-Adresse"].iter().enumerate() {
        sheet
            .write_string(1, u16::try_from(column).unwrap(), *header)
            .unwrap();
    }
    sheet.write_number(2, 0, 12345.0).unwrap();
    sheet.write_string(2, 1, "Lovelace").unwrap();
    sheet.write_string(2, 2, "ada@example.edu").unwrap();
    sheet.write_string(3, 0, "Bob").unwrap();
    sheet.write_string(3, 1, "Baker").unwrap();
    sheet.write_number(3, 2, 12.5).unwrap();
    sheet.write_string(4, 0, "Fusszeile").unwrap();

    let second = book.add_worksheet();
    second.set_name("Second").unwrap();
    second.write_string(0, 0, "VORNAME").unwrap();
    second.write_string(0, 1, "NACHNAME").unwrap();
    second.write_string(0, 2, "EMAIL").unwrap();
    book.save(path).unwrap();
}

#[test]
fn xlsx_detects_header_on_row_two_and_formats_numbers() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roster.xlsx");
    workbook(&path);

    let source = read_source(&path, &ReadOptions::default()).unwrap();
    assert_eq!(source.kind, SourceKind::Xlsx);
    assert_eq!(source.sheet_name.as_deref(), Some("Roster"));
    assert_eq!(source.sheets, ["Roster", "Second"]);
    assert_eq!(source.header_row, 2);
    assert_eq!(source.headers, ["Vorname", "Nachname", "E-Mail-Adresse"]);
    assert_eq!(source.rows.len(), 2);
    assert_eq!(source.ignored_trailing, 1);
    assert_eq!(source.rows[0].row, 3);
    assert_eq!(source.rows[0].cells[0], "12345", "integral floats lose .0");
    assert_eq!(source.rows[1].cells[2], "12.5");
}

#[test]
fn sheet_selection_and_unknown_sheet() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roster.xlsx");
    workbook(&path);

    let source = read_source(
        &path,
        &ReadOptions {
            sheet: Some("Second".to_owned()),
            ..ReadOptions::default()
        },
    )
    .unwrap();
    assert_eq!(source.sheet_name.as_deref(), Some("Second"));
    assert_eq!(source.headers, ["VORNAME", "NACHNAME", "EMAIL"]);
    assert!(source.rows.is_empty());

    let err = read_source(
        &path,
        &ReadOptions {
            sheet: Some("Missing".to_owned()),
            ..ReadOptions::default()
        },
    )
    .unwrap_err();
    match err {
        CoreError::Import { reason, .. } => {
            assert!(
                reason.contains("Missing") && reason.contains("Roster"),
                "{reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn ods_fixture_is_read_through_calamine() {
    let source = read("roster.ods");
    assert_eq!(source.kind, SourceKind::Ods);
    assert_eq!(source.sheet_name.as_deref(), Some("Roster"));
    assert_eq!(source.headers, ["VORNAME", "NACHNAME", "EMAIL"]);
    assert_eq!(
        cells(&source),
        vec![
            vec!["Ada", "Lovelace", "ada@example.edu"],
            vec!["Bob", "Baker", "bob@example.edu"],
        ]
    );
}

#[test]
fn xls_dispatch_reaches_the_sheet_reader() {
    // No tiny fixtures were available for the legacy binary format; this proves the
    // extension dispatch and the reader's error mapping.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy.xls");
    std::fs::write(&path, b"not a real workbook").unwrap();
    let err = read_source(&path, &ReadOptions::default()).unwrap_err();
    assert!(matches!(err, CoreError::Import { .. }), "got {err:?}");
}

#[test]
fn mapping_suggests_german_headers() {
    let source = read("variations.csv");
    let suggested = suggest(&source.headers);
    assert_eq!(suggested.first_name, Some(0));
    assert_eq!(suggested.last_name, Some(1));
    assert_eq!(suggested.email, Some(2));
    let mapping = resolve(None, suggested, source.headers.len()).unwrap();
    assert_eq!(
        mapping,
        ColumnMapping {
            first_name: 0,
            last_name: 1,
            email: 2
        }
    );
}
