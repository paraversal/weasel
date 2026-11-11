//! CSV/XLSX import pipeline. Introduced by 03-exec; intentionally empty for now.
//!
//! Layering: this crate may depend on `weasel-core` only. It must never touch SQLite
//! or Tauri — parsing produces plain values that the caller hands to a repository.
//! `tools/check-layers.sh` enforces that.
