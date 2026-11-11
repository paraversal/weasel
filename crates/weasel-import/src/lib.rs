//! CSV/XLSX import pipeline introduced by 03-exec.
//!
//! Layering: this crate depends on `weasel-core` only. It never touches SQLite or
//! Tauri — parsing produces plain values that the caller hands to a repository.
//! Parsing is synchronous; async only enters through core ports.
//! `tools/check-layers.sh` enforces both rules.

pub mod header;
pub mod mapping;
pub mod normalize;
pub mod read;
pub mod reconcile;
pub mod service;
pub mod source;

pub use header::{HeaderAnalysis, analyze};
pub use mapping::{SuggestedMapping, resolve, suggest};
pub use normalize::normalize;
pub use service::{ImportPreview, ImportService, PreviewOptions};
pub use source::{ParsedSource, RawRow, RawTable, ReadOptions, SourceReader, read_source};
