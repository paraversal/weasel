//! CSV reading: UTF-8 only, delimiter sniffing, tolerant row widths.

use weasel_core::{CoreError, CoreResult, enums::SourceKind};

use crate::source::{RawRow, RawTable, ReadOptions, SourceReader};

/// Byte-order mark tolerated at the start of a UTF-8 file.
const BOM: &[u8] = b"\xef\xbb\xbf";

pub struct CsvReader;

impl SourceReader for CsvReader {
    fn kinds(&self) -> &[SourceKind] {
        &[SourceKind::Csv]
    }

    fn read(&self, bytes: &[u8], _opts: &ReadOptions) -> CoreResult<RawTable> {
        let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
        let text = std::str::from_utf8(bytes).map_err(|_| CoreError::Import {
            row: None,
            reason: "This file is not UTF-8. Save the file as CSV UTF-8.".to_owned(),
        })?;
        let delimiter = sniff_delimiter(text);
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|err| CoreError::Import {
                row: None,
                reason: format!("The CSV file could not be parsed: {err}"),
            })?;
            let row = record.position().map_or(1, |position| {
                u32::try_from(position.line()).unwrap_or(u32::MAX)
            });
            let mut cells: Vec<String> = record.iter().map(|cell| cell.trim().to_owned()).collect();
            trim_empty_tail(&mut cells);
            rows.push(RawRow { row, cells });
        }

        Ok(RawTable {
            sheet_name: None,
            sheets: Vec::new(),
            rows,
        })
    }
}

/// Choose `;`, `,` or tab when its count is positive and identical on the first
/// five non-empty lines. Semicolon wins ties because German Excel exports it.
pub fn sniff_delimiter(text: &str) -> u8 {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .collect();
    let mut best = (b',', 0usize);
    for delimiter in *b";,\t" {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| line.matches(char::from(delimiter)).count())
            .collect();
        let Some(&first) = counts.first() else {
            continue;
        };
        if first == 0 || counts.iter().any(|count| *count != first) {
            continue;
        }
        if first > best.1 {
            best = (delimiter, first);
        }
    }
    best.0
}

fn trim_empty_tail(cells: &mut Vec<String>) {
    while cells.last().is_some_and(|cell| cell.is_empty()) {
        cells.pop();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn sniffs_consistent_delimiters() {
        assert_eq!(sniff_delimiter("a;b;c\n1;2;3\n"), b';');
        assert_eq!(sniff_delimiter("a,b,c\n1,2,3\n"), b',');
        assert_eq!(sniff_delimiter("a\tb\tc\n1\t2\t3\n"), b'\t');
        assert_eq!(sniff_delimiter("only one line\n"), b',');
    }

    #[test]
    fn semicolon_wins_a_tie() {
        assert_eq!(sniff_delimiter("a;b;c,d\n1;2;3,4\n"), b';');
    }

    #[test]
    fn inconsistent_counts_are_ignored() {
        // Comma counts differ (2 then 1), semicolon is consistent.
        assert_eq!(sniff_delimiter("a;b,c,d\n1;2,3\n"), b';');
    }
}
