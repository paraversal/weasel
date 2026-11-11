use std::{
    ffi::OsString,
    fs,
    io,
    path::{Path, PathBuf},
};

use crate::error::StoreError;

/// Replace `db_path` with `backup`.
///
/// The caller guarantees that the store has been closed: copying over a database that
/// still has WAL writers would corrupt it. A stale `-wal`/`-shm` next to a restored
/// file is worse than useless, so both are removed *before* the rename.
pub fn restore_backup(backup: &Path, db_path: &Path) -> Result<(), StoreError> {
    if !backup.is_file() {
        return Err(StoreError::Io(format!(
            "backup {} is not a regular file",
            backup.display()
        )));
    }
    let parent = db_path
        .parent()
        .ok_or_else(|| StoreError::Io("database path has no parent directory".to_owned()))?;
    fs::create_dir_all(parent).map_err(StoreError::io)?;

    let temp = sidecar(db_path, ".restore-tmp");
    match fs::remove_file(&temp) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(StoreError::io(err)),
    }

    fs::copy(backup, &temp).map_err(StoreError::io)?;
    {
        let file = fs::File::open(&temp).map_err(StoreError::io)?;
        file.sync_all().map_err(StoreError::io)?;
    }

    for suffix in ["-wal", "-shm"] {
        match fs::remove_file(sidecar(db_path, suffix)) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(StoreError::io(err)),
        }
    }

    fs::rename(&temp, db_path).map_err(StoreError::io)?;
    // Best effort: makes the rename durable on POSIX filesystems.
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// `weasel.sqlite3` + `-wal` → `weasel.sqlite3-wal`.
fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = db_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
