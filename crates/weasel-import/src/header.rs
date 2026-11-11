//! Header detection: find the row that names the three fields.
//!
//! Legacy exports put a title row above the header, so detection is independent
//! of row 1. Synonyms are case-, space-, dash- and umlaut-insensitive.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::source::RawRow;

pub(crate) const FIRST: &[&str] = &[
    "vorname",
    "first name",
    "firstname",
    "given name",
    "forename",
];
pub(crate) const LAST: &[&str] = &[
    "nachname",
    "last name",
    "lastname",
    "surname",
    "family name",
    "name",
];
pub(crate) const EMAIL: &[&str] = &["email", "e-mail", "mail", "email address", "e-mail-adresse"];

/// The result of looking at the first rows of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderAnalysis {
    /// Index into the raw row slice where headers live.
    pub header_index: usize,
    /// Index of the first data row.
    pub data_start: usize,
    /// 1-based row number of the header, or 1 when nothing was detected.
    pub header_row: u32,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    First,
    Last,
    Email,
}

/// Detect the header row among the first ten rows. A row needs at least two
/// recognized fields. With no match the first row is treated as the header.
pub fn analyze(rows: &[RawRow]) -> HeaderAnalysis {
    for (index, row) in rows.iter().take(10).enumerate() {
        let matched = matched_fields(&row.cells);
        if matched.len() >= 2 {
            return HeaderAnalysis {
                header_index: index,
                data_start: index + 1,
                header_row: row.row,
                headers: row.cells.clone(),
            };
        }
    }
    let headers = rows
        .first()
        .map(|row| row.cells.clone())
        .unwrap_or_default();
    HeaderAnalysis {
        header_index: 0,
        data_start: usize::from(!rows.is_empty()),
        header_row: 1,
        headers,
    }
}

fn matched_fields(cells: &[String]) -> Vec<Field> {
    let mut fields = Vec::new();
    for cell in cells {
        let folded = fold(cell);
        if !fields.contains(&Field::First) && FIRST.iter().any(|synonym| fold(synonym) == folded) {
            fields.push(Field::First);
        } else if !fields.contains(&Field::Last)
            && LAST.iter().any(|synonym| fold(synonym) == folded)
        {
            fields.push(Field::Last);
        } else if !fields.contains(&Field::Email)
            && EMAIL.iter().any(|synonym| fold(synonym) == folded)
        {
            fields.push(Field::Email);
        }
    }
    fields
}

/// Lowercase, fold German umlauts, strip combining marks and separators.
pub fn fold(raw: &str) -> String {
    let mut replaced = String::with_capacity(raw.len());
    for letter in raw.chars() {
        match letter {
            'ä' | 'Ä' => replaced.push_str("ae"),
            'ö' | 'Ö' => replaced.push_str("oe"),
            'ü' | 'Ü' => replaced.push_str("ue"),
            'ß' => replaced.push_str("ss"),
            other => replaced.extend(other.to_lowercase()),
        }
    }
    replaced
        .nfd()
        .filter(|letter| !is_combining_mark(*letter))
        .filter(|letter| letter.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn folds_case_separators_and_umlauts() {
        assert_eq!(fold("E-Mail-Adresse"), "emailadresse");
        assert_eq!(fold("Vorname"), "vorname");
        assert_eq!(fold("  FIRST_NAME "), "firstname");
        assert_eq!(fold("Nachname"), "nachname");
        assert_eq!(fold("Müller"), "mueller");
    }
}
