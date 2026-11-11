//! Source readers: CSV and spreadsheet files become [`ParsedSource`] values.
//!
//! Everything here is synchronous. The adapter runs [`read_source`] on a blocking
//! thread after validating the path; this crate never touches a runtime.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use calamine::Reader;
use sha2::{Digest, Sha256};
use weasel_core::{CoreError, CoreResult, enums::SourceKind};

use crate::{header, mapping, read};

/// Default cap for a source file: 20 MiB.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;
/// Default cap for data rows: 100 000.
pub const MAX_ROWS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOptions {
    /// Worksheet to read; the first sheet when `None`.
    pub sheet: Option<String>,
    pub max_bytes: usize,
    pub max_rows: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            sheet: None,
            max_bytes: MAX_BYTES,
            max_rows: MAX_ROWS,
        }
    }
}

/// One physical row. `row` is the 1-based sheet/line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRow {
    pub row: u32,
    pub cells: Vec<String>,
}

/// A reader's output before header detection and trailing-row trimming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTable {
    pub sheet_name: Option<String>,
    pub sheets: Vec<String>,
    pub rows: Vec<RawRow>,
}

/// Everything the import pipeline needs from one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSource {
    pub kind: SourceKind,
    /// File name only, never the directory.
    pub name: String,
    pub path: PathBuf,
    /// Lowercase hex SHA-256 of the file bytes.
    pub file_hash: String,
    pub sheet_name: Option<String>,
    pub sheets: Vec<String>,
    /// 1-based row the headers were found on.
    pub header_row: u32,
    pub headers: Vec<String>,
    pub rows: Vec<RawRow>,
    pub ignored_trailing: u32,
}

/// The extension point described in `docs/rewrite/05-extension-points.md`.
///
/// A reader only decodes bytes into rows. Header detection, mapping and
/// reconciliation are shared and never reimplemented per format.
pub trait SourceReader: Send + Sync {
    fn kinds(&self) -> &[SourceKind];
    fn read(&self, bytes: &[u8], opts: &ReadOptions) -> CoreResult<RawTable>;
}

/// CSV first: its extension is unambiguous.
static READERS: [&dyn SourceReader; 2] = [&read::csv::CsvReader, &read::sheet::SheetReader];

fn reader_for(kind: SourceKind) -> CoreResult<&'static dyn SourceReader> {
    READERS
        .iter()
        .copied()
        .find(|reader| reader.kinds().contains(&kind))
        .ok_or_else(|| CoreError::Import {
            row: None,
            reason: format!("No reader is registered for {}.", kind.as_str()),
        })
}

fn kind_for(path: &Path) -> CoreResult<SourceKind> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" | "xlsm" => Ok(SourceKind::Xlsx),
        "xls" => Ok(SourceKind::Xls),
        "ods" => Ok(SourceKind::Ods),
        "csv" => Ok(SourceKind::Csv),
        other => Err(CoreError::Import {
            row: None,
            reason: if other.is_empty() {
                "The file has no extension; choose a CSV or spreadsheet file.".to_owned()
            } else {
                format!("Unsupported file type .{other}; choose a CSV or spreadsheet file.")
            },
        }),
    }
}

/// Read and analyze one file. The caller is responsible for path validation and
/// for running this on a blocking thread.
pub fn read_source(path: &Path, opts: &ReadOptions) -> CoreResult<ParsedSource> {
    let kind = kind_for(path)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CoreError::Validation {
            field: "path",
            reason: "The file name is not valid UTF-8.".to_owned(),
        })?
        .to_owned();

    let metadata = fs::metadata(path).map_err(CoreError::io)?;
    if metadata.len() > opts.max_bytes as u64 {
        return Err(CoreError::Import {
            row: None,
            reason: format!(
                "The file is {} bytes; the limit is {} bytes.",
                metadata.len(),
                opts.max_bytes
            ),
        });
    }
    let bytes = fs::read(path).map_err(CoreError::io)?;
    if bytes.len() > opts.max_bytes {
        return Err(CoreError::Import {
            row: None,
            reason: format!(
                "The file is {} bytes; the limit is {} bytes.",
                bytes.len(),
                opts.max_bytes
            ),
        });
    }

    let hash = Sha256::digest(&bytes);
    let file_hash = hash.iter().map(|byte| format!("{byte:02x}")).collect();

    let table = reader_for(kind)?.read(&bytes, opts)?;
    let analysis = header::analyze(&table.rows);
    let suggested = mapping::suggest(&analysis.headers);
    let mut data: Vec<RawRow> = table.rows[analysis.data_start..].to_vec();
    let ignored_trailing = trim_trailing_footer(&mut data, suggested.email);

    if data.len() > opts.max_rows {
        return Err(CoreError::Import {
            row: None,
            reason: format!(
                "The sheet has {} rows; the limit is {}.",
                data.len(),
                opts.max_rows
            ),
        });
    }

    Ok(ParsedSource {
        kind,
        name,
        path: path.to_path_buf(),
        file_hash,
        sheet_name: table.sheet_name,
        sheets: table.sheets,
        header_row: analysis.header_row,
        headers: analysis.headers,
        rows: data,
        ignored_trailing,
    })
}

/// Drop rows after the last row whose email cell is non-empty (legacy footers).
fn trim_trailing_footer(rows: &mut Vec<RawRow>, email_col: Option<u32>) -> u32 {
    let Some(col) = email_col else {
        return 0;
    };
    let keep = rows
        .iter()
        .rposition(|row| {
            row.cells
                .get(col as usize)
                .is_some_and(|cell| !cell.trim().is_empty())
        })
        .map_or(0, |index| index + 1);
    let ignored = (rows.len() - keep) as u32;
    rows.truncate(keep);
    ignored
}

/// Decode one calamine cell according to 03-exec §4.10.
pub(crate) fn cell_text(data: &calamine::Data) -> String {
    use calamine::Data;
    match data {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(text) => text.clone(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) if *value == 0.0 => "0".to_owned(),
        Data::Float(value) if value.fract() == 0.0 && value.is_finite() => {
            format!("{value:.0}")
        }
        Data::Float(value) => value.to_string(),
        Data::Bool(value) => {
            if *value {
                "TRUE".to_owned()
            } else {
                "FALSE".to_owned()
            }
        }
        Data::DateTime(date) => {
            let (year, month, day, _, _, _, _) = date.to_ymd_hms_milli();
            format!("{year:04}-{month:02}-{day:02}")
        }
        Data::DateTimeIso(text) | Data::DurationIso(text) => text.clone(),
    }
}

/// Shared sheet opener. `calamine::open_workbook_auto_from_rs` recognizes the
/// format from the bytes, so xls/xlsx/xlsm/ods all take this path.
pub(crate) fn open_sheet<'a>(
    bytes: &'a [u8],
    _opts: &ReadOptions,
) -> CoreResult<calamine::Sheets<Cursor<&'a [u8]>>> {
    calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|err| CoreError::Import {
        row: None,
        reason: format!("The workbook could not be opened: {err:?}"),
    })
}

pub(crate) fn chosen_sheet(
    workbook: &calamine::Sheets<Cursor<&[u8]>>,
    opts: &ReadOptions,
) -> CoreResult<String> {
    let sheets = workbook.sheet_names();
    let first = sheets.first().ok_or_else(|| CoreError::Import {
        row: None,
        reason: "The workbook has no worksheets.".to_owned(),
    })?;
    match &opts.sheet {
        None => Ok(first.clone()),
        Some(wanted) => {
            if sheets.iter().any(|sheet| sheet == wanted) {
                Ok(wanted.clone())
            } else {
                Err(CoreError::Import {
                    row: None,
                    reason: format!(
                        "Sheet \"{wanted}\" does not exist. Available sheets: {}.",
                        sheets.join(", ")
                    ),
                })
            }
        }
    }
}
