use std::path::{Path, PathBuf};

use weasel_core::{CoreError, CoreResult};

/// Validate a user-chosen source file: resolve symlinks, require a regular file, and
/// accept only the listed extensions (case-insensitive).
///
/// Returns the canonical path so later reads cannot be redirected by a swapped
/// symlink. `allowed_exts` are given without the dot.
pub fn validate_source_path(path: &Path, allowed_exts: &[&str]) -> CoreResult<PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|err| {
        CoreError::validation("path", format!("cannot open {}: {err}", path.display()))
    })?;

    let meta = std::fs::metadata(&canonical)
        .map_err(|err| CoreError::validation("path", format!("cannot stat: {err}")))?;
    if !meta.is_file() {
        return Err(CoreError::validation("path", "not a regular file"));
    }

    let ext = canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| CoreError::validation("path", "file has no extension"))?;

    if !allowed_exts
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(&ext))
    {
        return Err(CoreError::validation(
            "path",
            format!(
                "unsupported file type .{ext}; expected one of: {}",
                allowed_exts.join(", ")
            ),
        ));
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn accepts_allowed_extension_case_insensitively() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("addresses.CSV");
        std::fs::write(&file, "a").unwrap();
        let got = validate_source_path(&file, &["csv", "xlsx"]).unwrap();
        assert_eq!(got, std::fs::canonicalize(&file).unwrap());
    }

    #[test]
    fn rejects_unknown_extension() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "a").unwrap();
        let err = validate_source_path(&file, &["csv"]).unwrap_err();
        assert!(matches!(err, CoreError::Validation { field: "path", .. }));
    }

    #[test]
    fn rejects_missing_file_and_directories() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(validate_source_path(&dir.path().join("nope.csv"), &["csv"]).is_err());
        assert!(validate_source_path(dir.path(), &["csv"]).is_err());
    }
}
