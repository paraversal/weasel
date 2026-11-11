use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::{error::StoreError, meta::set_meta};

/// Cheap-to-clone handle over the SQLite pool.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Copy)]
pub struct OpenOptions<'a> {
    pub db_path: &'a Path,
    pub backups_dir: &'a Path,
}

#[derive(Debug, Clone, Default)]
pub struct OpenReport {
    /// Highest applied migration version.
    pub schema_version: i64,
    /// Backup taken before migrating, if any.
    pub backup: Option<PathBuf>,
    /// True when the database file was created by this call.
    pub created: bool,
}

/// Open (or create) the application database, backing it up before any pending
/// migration runs.
pub async fn open(opts: OpenOptions<'_>) -> Result<(Store, OpenReport), StoreError> {
    open_with(opts, &sqlx::migrate!("../../migrations")).await
}

/// [`open`] with an explicit migrator. Exposed for tests that need fixture schemas.
#[doc(hidden)]
pub async fn open_with(
    opts: OpenOptions<'_>,
    migrator: &Migrator,
) -> Result<(Store, OpenReport), StoreError> {
    let created = !opts.db_path.exists();

    let connect = SqliteConnectOptions::new()
        .filename(opts.db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(connect)
        .await?;
    let store = Store { pool: pool.clone() };

    let known_max = migrator.iter().map(|m| m.version).max().unwrap_or(0);
    let applied = store.applied_versions().await?;
    let applied_max = applied.iter().copied().max().unwrap_or(0);

    if applied_max > known_max {
        return Err(StoreError::NewerSchema {
            db: applied_max,
            app: known_max,
        });
    }

    let pending: Vec<i64> = migrator
        .iter()
        .filter(|m| !applied.contains(&m.version))
        .map(|m| m.version)
        .collect();

    let mut backup = None;
    if !created && !pending.is_empty() {
        let target = pending.iter().copied().max().unwrap_or(0);
        let path = store.backup_to(opts.backups_dir, "pre", target).await?;
        tracing::info!(path = %path.display(), "backed up database before migrating");
        backup = Some(path);
    }

    migrator.run(store.pool()).await?;

    if let Some(path) = &backup {
        let path = path.to_string_lossy().into_owned();
        set_meta(&store, "last_backup_path", &path).await?;
    }

    if created {
        set_meta(
            &store,
            "created_at",
            &SystemTime::now().now_ms().to_string(),
        )
        .await?;
        let instance_id: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
            .fetch_one(&pool)
            .await?;
        set_meta(&store, "instance_id", &instance_id).await?;
    }

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await?;
    if foreign_keys != 1 {
        return Err(StoreError::Io(
            "foreign_keys pragma is not enabled".to_owned(),
        ));
    }
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Io(format!(
            "journal_mode is {journal_mode}, expected wal"
        )));
    }

    let schema_version = store.schema_version().await?;
    Ok((
        store,
        OpenReport {
            schema_version,
            backup,
            created,
        },
    ))
}

impl Store {
    /// Raw pool access for repository modules in this crate and for diagnostics.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }

    /// Highest successfully applied migration version, or 0 for an unmigrated file.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        if !self.has_migrations_table().await? {
            return Ok(0);
        }
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(version)
    }

    /// Consistent copy of the database (`VACUUM INTO`), safe while WAL is active.
    pub async fn backup_now(&self, dir: &Path) -> Result<PathBuf, StoreError> {
        let version = self.schema_version().await?;
        self.backup_to(dir, "backup", version).await
    }

    async fn applied_versions(&self) -> Result<Vec<i64>, StoreError> {
        if !self.has_migrations_table().await? {
            return Ok(Vec::new());
        }
        let versions =
            sqlx::query_scalar::<_, i64>("SELECT version FROM _sqlx_migrations WHERE success = 1")
                .fetch_all(&self.pool)
                .await?;
        Ok(versions)
    }

    async fn has_migrations_table(&self) -> Result<bool, StoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    async fn backup_to(&self, dir: &Path, kind: &str, version: i64) -> Result<PathBuf, StoreError> {
        std::fs::create_dir_all(dir).map_err(StoreError::io)?;
        let stamp = utc_stamp(SystemTime::now());
        let mut path = dir.join(format!("weasel.sqlite3.{kind}-v{version}-{stamp}.bak"));
        let mut suffix = 1;
        while path.exists() {
            path = dir.join(format!(
                "weasel.sqlite3.{kind}-v{version}-{stamp}-{suffix}.bak"
            ));
            suffix += 1;
        }
        let target = path
            .to_str()
            .ok_or_else(|| StoreError::io("backup path is not valid UTF-8"))?
            .to_owned();
        sqlx::query("VACUUM INTO ?")
            .bind(target)
            .execute(&self.pool)
            .await?;
        Ok(path)
    }
}

/// Minimal `yyyyMMddTHHmmss` UTC formatter (no external date dependency).
fn utc_stamp(now: SystemTime) -> String {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

trait NowMs {
    fn now_ms(self) -> i64;
}

impl NowMs for SystemTime {
    fn now_ms(self) -> i64 {
        self.duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}
