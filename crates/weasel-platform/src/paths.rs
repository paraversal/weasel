use std::path::{Path, PathBuf};

/// Where the app keeps its state, derived once at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub backups_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl DataPaths {
    /// Resolve paths.
    ///
    /// Precedence: `override_dir` (from `WEASEL_DATA_DIR`) wins over
    /// `app_data_dir`. In dev builds everything lives in a `dev/` subdirectory so a
    /// release install and a checkout never share a database.
    pub fn resolve(app_data_dir: &Path, override_dir: Option<&Path>, dev: bool) -> Self {
        let base = match override_dir {
            Some(dir) => dir.to_path_buf(),
            None => app_data_dir.to_path_buf(),
        };
        let data_dir = if dev { base.join("dev") } else { base };
        Self {
            db_path: data_dir.join("weasel.sqlite3"),
            backups_dir: data_dir.join("backups"),
            logs_dir: data_dir.join("logs"),
            data_dir,
        }
    }

    /// Create every directory the app writes into. Idempotent.
    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.backups_dir)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn release_layout_uses_app_dir() {
        let paths = DataPaths::resolve(Path::new("/data/weasel"), None, false);
        assert_eq!(paths.data_dir, Path::new("/data/weasel"));
        assert_eq!(paths.db_path, Path::new("/data/weasel/weasel.sqlite3"));
        assert_eq!(paths.backups_dir, Path::new("/data/weasel/backups"));
        assert_eq!(paths.logs_dir, Path::new("/data/weasel/logs"));
    }

    #[test]
    fn dev_builds_are_isolated() {
        let paths = DataPaths::resolve(Path::new("/data/weasel"), None, true);
        assert_eq!(paths.data_dir, Path::new("/data/weasel/dev"));
        assert_eq!(paths.db_path, Path::new("/data/weasel/dev/weasel.sqlite3"));
    }

    #[test]
    fn override_wins_over_app_dir() {
        let paths = DataPaths::resolve(Path::new("/data/weasel"), Some(Path::new("/tmp/w")), true);
        assert_eq!(paths.data_dir, Path::new("/tmp/w/dev"));
    }

    #[test]
    fn ensure_creates_all_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let paths = DataPaths::resolve(dir.path(), None, false);
        paths.ensure().unwrap();
        assert!(paths.data_dir.is_dir());
        assert!(paths.backups_dir.is_dir());
        assert!(paths.logs_dir.is_dir());
        paths.ensure().unwrap();
    }
}
