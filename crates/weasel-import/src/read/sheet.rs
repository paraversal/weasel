//! XLSX/XLSM/XLS/ODS reading through calamine's auto detection.

use calamine::Reader;
use weasel_core::{CoreError, CoreResult, enums::SourceKind};

use crate::source::{
    RawRow, RawTable, ReadOptions, SourceReader, cell_text, chosen_sheet, open_sheet,
};

pub struct SheetReader;

impl SourceReader for SheetReader {
    fn kinds(&self) -> &[SourceKind] {
        &[SourceKind::Xlsx, SourceKind::Xls, SourceKind::Ods]
    }

    fn read(&self, bytes: &[u8], opts: &ReadOptions) -> CoreResult<RawTable> {
        let mut workbook = open_sheet(bytes, opts)?;
        let sheets = workbook.sheet_names();
        let sheet_name = chosen_sheet(&workbook, opts)?;
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|err| CoreError::Import {
                row: None,
                reason: format!("Sheet \"{sheet_name}\" could not be read: {err:?}"),
            })?;

        let (start_row, _) = range.start().unwrap_or((0, 0));
        let mut rows = Vec::with_capacity(range.height());
        for (offset, cells) in range.rows().enumerate() {
            let mut cells: Vec<String> = cells.iter().map(cell_text).collect();
            while cells.last().is_some_and(|cell| cell.is_empty()) {
                cells.pop();
            }
            rows.push(RawRow {
                row: start_row + u32::try_from(offset).unwrap_or(u32::MAX) + 1,
                cells,
            });
        }

        Ok(RawTable {
            sheet_name: Some(sheet_name),
            sheets,
            rows,
        })
    }
}
