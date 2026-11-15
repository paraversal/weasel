//! One-time copy of a Python TUI database (`weasel_lair.db`) into a fresh Weasel database.
//!
//! `migrate-legacy --from <weasel_lair.db> --data-dir <dir>`
//!
//! Not part of the app: Weasel never imports or discovers a legacy database, so this
//! is a personal one-off kept outside the app's flows. The legacy file is
//! opened read-only. The target must not exist yet; it is created through
//! `weasel_store::open`, so it carries the app's real migrations and metadata.
//!
//! Mapping:
//! - `freshies.batch_id`          → one applied `import_batches` row per batch (same id)
//! - `freshies`                   → `contacts` (same id) + one open email-history row;
//!   `removed = 1` → `removed`, `0`/NULL → `active`
//! - `campaigns`                  → `campaigns` (same id), `text` → body, `info` → description
//! - `campaigns_sent_instances`   → one `send_batches` row per `paste_id` + `send_records`
//!   with status `copied`, so nobody gets a campaign twice
//!
//! The legacy app never stored send times, so campaigns and send batches use the
//! migration time; batch ids follow the paste order, which keeps the history order.
//! Contact and import dates come from `date_added` (naive local time).
//!
//! Output contains counts only: never names, addresses or campaign text.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Row, Sqlite, Transaction};
use weasel_core::audit::actions;
use weasel_core::email::EmailAddress;
use weasel_core::identity::{IdentityStrategy, NAME_IDENTITY_ID, NameIdentity, clean_name};
use weasel_store::Store;

const MIGRATED_FROM: &str = "legacy-tui";

struct Args {
    from: PathBuf,
    data_dir: PathBuf,
}

fn usage() -> &'static str {
    "usage: migrate-legacy --from <weasel_lair.db> --data-dir <dir>"
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut from = None;
    let mut data_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--from" => from = Some(PathBuf::from(args.next().ok_or("--from needs a value")?)),
            "--data-dir" => {
                data_dir = Some(PathBuf::from(
                    args.next().ok_or("--data-dir needs a value")?,
                ));
            }
            "--help" | "-h" => return Err(usage().to_owned()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }
    Ok(Args {
        from: from.ok_or_else(|| format!("--from is required\n{}", usage()))?,
        data_dir: data_dir.ok_or_else(|| format!("--data-dir is required\n{}", usage()))?,
    })
}

struct Freshie {
    id: i64,
    first_name: String,
    last_name: String,
    email: EmailAddress,
    identity_key: Option<String>,
    added_ms: i64,
    batch_id: i64,
    removed: bool,
}

struct Campaign {
    id: i64,
    name: String,
    body: String,
    description: String,
}

struct SentInstance {
    campaign_id: i64,
    student_id: i64,
    paste_id: i64,
}

struct Legacy {
    freshies: Vec<Freshie>,
    campaigns: Vec<Campaign>,
    sent: Vec<SentInstance>,
}

fn main() {
    if let Err(err) = run_main() {
        eprintln!("migrate-legacy: {err}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let args = parse_args()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("runtime: {err}"))?;
    runtime.block_on(migrate(&args))
}

async fn migrate(args: &Args) -> Result<(), String> {
    if !args.from.is_file() {
        return Err(format!("{} does not exist", args.from.display()));
    }
    let db_path = args.data_dir.join("weasel.sqlite3");
    if db_path.exists() {
        return Err(format!(
            "{} already exists; this tool only writes a fresh database",
            db_path.display()
        ));
    }

    let legacy = read_legacy(&args.from).await?;
    println!(
        "legacy: {} freshies, {} campaigns, {} sent instances",
        legacy.freshies.len(),
        legacy.campaigns.len(),
        legacy.sent.len()
    );

    std::fs::create_dir_all(&args.data_dir).map_err(|err| format!("create data dir: {err}"))?;
    let (store, report) = weasel_store::open(weasel_store::OpenOptions {
        db_path: &db_path,
        backups_dir: &args.data_dir.join("backups"),
    })
    .await
    .map_err(|err| err.to_string())?;
    println!(
        "target: {} (schema {})",
        db_path.display(),
        report.schema_version
    );

    let now_ms = now_ms()?;
    let written = write(&store, &legacy, now_ms).await;
    if let Err(err) = written {
        store.close().await;
        return Err(format!(
            "{err}\nnothing was committed; delete {} before retrying",
            args.data_dir.display()
        ));
    }
    verify(&store, &legacy).await?;
    store.close().await;
    println!("done");
    Ok(())
}

fn now_ms() -> Result<i64, String> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("clock: {err}"))?;
    i64::try_from(elapsed.as_millis()).map_err(|err| format!("clock: {err}"))
}

async fn read_legacy(path: &Path) -> Result<Legacy, String> {
    let options = SqliteConnectOptions::from_str("sqlite://")
        .map_err(|err| err.to_string())?
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let mut conn = SqliteConnection::connect_with(&options)
        .await
        .map_err(|err| format!("open legacy db: {err}"))?;

    // `date_added` is Python's `str(datetime.now())`: naive local time. SQLite's
    // 'utc' modifier interprets it as local time on this machine.
    let rows = sqlx::query(
        "SELECT id, first_name, last_name, email, batch_id, \
                COALESCE(removed, 0) AS removed, \
                CAST(round((julianday(date_added, 'utc') - 2440587.5) * 86400000.0) AS INTEGER) \
                    AS added_ms \
         FROM freshies ORDER BY id",
    )
    .fetch_all(&mut conn)
    .await
    .map_err(|err| format!("read freshies: {err}"))?;

    let identity = NameIdentity;
    let mut freshies = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = get(&row, "id")?;
        let raw_email: String = get(&row, "email")?;
        let email = EmailAddress::parse(&raw_email)
            .map_err(|err| format!("freshie {id}: invalid email ({err})"))?;
        let first_name = clean_name(&get::<String>(&row, "first_name")?);
        let last_name = clean_name(&get::<String>(&row, "last_name")?);
        if first_name.is_empty() || last_name.is_empty() {
            return Err(format!("freshie {id}: empty first or last name"));
        }
        let added_ms: Option<i64> = get(&row, "added_ms")?;
        let added_ms =
            added_ms.ok_or_else(|| format!("freshie {id}: date_added is not a valid date"))?;
        freshies.push(Freshie {
            id,
            identity_key: identity.key(&first_name, &last_name),
            first_name,
            last_name,
            email,
            added_ms,
            batch_id: get(&row, "batch_id")?,
            removed: get::<i64>(&row, "removed")? != 0,
        });
    }

    let campaigns = sqlx::query("SELECT id, name, text, info FROM campaigns ORDER BY id")
        .fetch_all(&mut conn)
        .await
        .map_err(|err| format!("read campaigns: {err}"))?
        .iter()
        .map(|row| {
            Ok(Campaign {
                id: get(row, "id")?,
                name: get::<String>(row, "name")?.trim().to_owned(),
                body: get(row, "text")?,
                description: get(row, "info")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let sent = sqlx::query(
        "SELECT campaign_id, student_id, paste_id FROM campaigns_sent_instances \
         ORDER BY paste_id, id",
    )
    .fetch_all(&mut conn)
    .await
    .map_err(|err| format!("read campaigns_sent_instances: {err}"))?
    .iter()
    .map(|row| {
        Ok(SentInstance {
            campaign_id: get(row, "campaign_id")?,
            student_id: get(row, "student_id")?,
            paste_id: get(row, "paste_id")?,
        })
    })
    .collect::<Result<Vec<_>, String>>()?;

    conn.close().await.map_err(|err| err.to_string())?;
    check_legacy(&freshies, &campaigns, &sent)?;
    Ok(Legacy {
        freshies,
        campaigns,
        sent,
    })
}

fn get<'r, T>(row: &'r sqlx::sqlite::SqliteRow, column: &str) -> Result<T, String>
where
    T: sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(column)
        .map_err(|err| format!("column {column}: {err}"))
}

/// Fail before writing anything if the legacy data breaks a rule of the new schema.
fn check_legacy(
    freshies: &[Freshie],
    campaigns: &[Campaign],
    sent: &[SentInstance],
) -> Result<(), String> {
    let mut seen = BTreeMap::new();
    for freshie in freshies {
        if let Some(other) = seen.insert(freshie.email.normalized.clone(), freshie.id) {
            return Err(format!(
                "freshies {other} and {} share an address after lowercasing",
                freshie.id
            ));
        }
    }
    let freshie_ids: BTreeMap<i64, ()> = freshies.iter().map(|f| (f.id, ())).collect();
    let campaign_ids: BTreeMap<i64, ()> = campaigns.iter().map(|c| (c.id, ())).collect();
    let mut paste_campaign: BTreeMap<i64, i64> = BTreeMap::new();
    let mut pairs = BTreeMap::new();
    for instance in sent {
        if !freshie_ids.contains_key(&instance.student_id) {
            return Err(format!(
                "paste {} references missing freshie {}",
                instance.paste_id, instance.student_id
            ));
        }
        if !campaign_ids.contains_key(&instance.campaign_id) {
            return Err(format!(
                "paste {} references missing campaign {}",
                instance.paste_id, instance.campaign_id
            ));
        }
        let campaign = *paste_campaign
            .entry(instance.paste_id)
            .or_insert(instance.campaign_id);
        if campaign != instance.campaign_id {
            return Err(format!("paste {} spans two campaigns", instance.paste_id));
        }
        if pairs
            .insert((instance.paste_id, instance.student_id), ())
            .is_some()
        {
            return Err(format!(
                "paste {} lists freshie {} twice",
                instance.paste_id, instance.student_id
            ));
        }
    }
    Ok(())
}

async fn write(store: &Store, legacy: &Legacy, now_ms: i64) -> Result<(), String> {
    let mut tx = store.begin_write().await.map_err(|err| err.to_string())?;

    // Import batches: one per legacy batch_id, dated by its earliest freshie.
    let mut batches: BTreeMap<i64, (i64, u64)> = BTreeMap::new();
    for freshie in &legacy.freshies {
        let entry = batches
            .entry(freshie.batch_id)
            .or_insert((freshie.added_ms, 0));
        entry.0 = entry.0.min(freshie.added_ms);
        entry.1 += 1;
    }
    for (&batch_id, &(created_ms, count)) in &batches {
        sqlx::query(
            "INSERT INTO import_batches \
             (id, source_kind, source_name, scope, status, created_at, applied_at, \
              row_count, added_count) \
             VALUES (?, 'manual', ?, 'delta', 'applied', ?, ?, ?, ?)",
        )
        .bind(batch_id)
        .bind(format!("Legacy batch {batch_id}"))
        .bind(created_ms)
        .bind(created_ms)
        .bind(count as i64)
        .bind(count as i64)
        .execute(&mut *tx)
        .await
        .map_err(|err| format!("import batch {batch_id}: {err}"))?;
        audit(
            &mut tx,
            actions::IMPORT_APPLY,
            "import_batch",
            batch_id,
            json!({
                "scope": "delta",
                "fileHash": null,
                "summary": {
                    "rowCount": count, "added": count, "updated": 0, "removed": 0,
                    "conflicts": 0, "unchanged": 0, "invalid": 0
                },
                "migratedFrom": MIGRATED_FROM,
            }),
            created_ms,
        )
        .await?;
    }

    for freshie in &legacy.freshies {
        sqlx::query(
            "INSERT INTO contacts \
             (id, first_name, last_name, email, email_normalized, identity_key, status, \
              source, import_batch_id, date_added, updated_at, version, notes, metadata_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 'import', ?, ?, ?, 1, '', ?)",
        )
        .bind(freshie.id)
        .bind(&freshie.first_name)
        .bind(&freshie.last_name)
        .bind(&freshie.email.display)
        .bind(&freshie.email.normalized)
        .bind(&freshie.identity_key)
        .bind(if freshie.removed { "removed" } else { "active" })
        .bind(freshie.batch_id)
        .bind(freshie.added_ms)
        .bind(freshie.added_ms)
        .bind(json!({ "migratedFrom": MIGRATED_FROM }).to_string())
        .execute(&mut *tx)
        .await
        .map_err(|err| format!("contact {}: {err}", freshie.id))?;
        sqlx::query(
            "INSERT INTO contact_email_history \
             (contact_id, email, email_normalized, valid_from, reason) \
             VALUES (?, ?, ?, ?, 'import')",
        )
        .bind(freshie.id)
        .bind(&freshie.email.display)
        .bind(&freshie.email.normalized)
        .bind(freshie.added_ms)
        .execute(&mut *tx)
        .await
        .map_err(|err| format!("email history {}: {err}", freshie.id))?;
    }
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES ('identity_strategy', ?) \
         ON CONFLICT(key) DO NOTHING",
    )
    .bind(NAME_IDENTITY_ID)
    .execute(&mut *tx)
    .await
    .map_err(|err| format!("app_meta: {err}"))?;

    for campaign in &legacy.campaigns {
        sqlx::query(
            "INSERT INTO campaigns \
             (id, name, description, subject, body, status, created_at, updated_at, version) \
             VALUES (?, ?, ?, '', ?, 'active', ?, ?, 1)",
        )
        .bind(campaign.id)
        .bind(&campaign.name)
        .bind(&campaign.description)
        .bind(&campaign.body)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await
        .map_err(|err| format!("campaign {}: {err}", campaign.id))?;
        audit(
            &mut tx,
            actions::CAMPAIGN_CREATE,
            "campaign",
            campaign.id,
            json!({ "migratedFrom": MIGRATED_FROM }),
            now_ms,
        )
        .await?;
    }

    let freshies: BTreeMap<i64, &Freshie> = legacy.freshies.iter().map(|f| (f.id, f)).collect();
    let mut pastes: BTreeMap<i64, (i64, Vec<i64>)> = BTreeMap::new();
    for instance in &legacy.sent {
        pastes
            .entry(instance.paste_id)
            .or_insert((instance.campaign_id, Vec::new()))
            .1
            .push(instance.student_id);
    }
    // Ascending ids in paste order: the app sorts by created_at, then id.
    for (paste_id, (campaign_id, students)) in &pastes {
        let batch_id: i64 = sqlx::query_scalar(
            "INSERT INTO send_batches (campaign_id, created_at, recipient_count, note, copied_at) \
             VALUES (?, ?, ?, '', ?) RETURNING id",
        )
        .bind(campaign_id)
        .bind(now_ms)
        .bind(students.len() as i64)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| format!("send batch for paste {paste_id}: {err}"))?;
        for student_id in students {
            let freshie = freshies[student_id];
            sqlx::query(
                "INSERT INTO send_records \
                 (batch_id, contact_id, email, email_normalized, first_name, last_name, \
                  status, created_at) VALUES (?, ?, ?, ?, ?, ?, 'copied', ?)",
            )
            .bind(batch_id)
            .bind(freshie.id)
            .bind(&freshie.email.display)
            .bind(&freshie.email.normalized)
            .bind(&freshie.first_name)
            .bind(&freshie.last_name)
            .bind(now_ms)
            .execute(&mut *tx)
            .await
            .map_err(|err| format!("send record (paste {paste_id}): {err}"))?;
        }
        audit(
            &mut tx,
            actions::SEND_BATCH_CREATE,
            "send_batch",
            batch_id,
            json!({
                "campaignId": campaign_id,
                "recipients": students.len(),
                "legacyPasteId": paste_id,
                "migratedFrom": MIGRATED_FROM,
            }),
            now_ms,
        )
        .await?;
    }

    tx.commit().await.map_err(|err| format!("commit: {err}"))
}

async fn audit(
    tx: &mut Transaction<'static, Sqlite>,
    action: &str,
    entity_type: &str,
    entity_id: i64,
    metadata: serde_json::Value,
    at_ms: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO audit_events (at, actor, action, entity_type, entity_id, metadata_json) \
         VALUES (?, 'migration', ?, ?, ?, ?)",
    )
    .bind(at_ms)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id.to_string())
    .bind(metadata.to_string())
    .execute(&mut **tx)
    .await
    .map_err(|err| format!("audit {action}: {err}"))?;
    Ok(())
}

/// Re-read the committed database and compare counts with the legacy file.
async fn verify(store: &Store, legacy: &Legacy) -> Result<(), String> {
    let pool = store.pool();
    let scalar = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(pool)
            .await
            .map_err(|err| format!("{sql}: {err}"))
    };

    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .map_err(|err| err.to_string())?;
    let fk_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .map_err(|err| err.to_string())?
        .len();

    let removed = legacy.freshies.iter().filter(|f| f.removed).count() as i64;
    let pastes = legacy
        .sent
        .iter()
        .map(|s| s.paste_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len() as i64;
    let checks = [
        (
            "contacts",
            scalar("SELECT COUNT(*) FROM contacts").await?,
            legacy.freshies.len() as i64,
        ),
        (
            "active contacts",
            scalar("SELECT COUNT(*) FROM contacts WHERE status = 'active'").await?,
            legacy.freshies.len() as i64 - removed,
        ),
        (
            "removed contacts",
            scalar("SELECT COUNT(*) FROM contacts WHERE status = 'removed'").await?,
            removed,
        ),
        (
            "open email history rows",
            scalar("SELECT COUNT(*) FROM contact_email_history WHERE valid_to IS NULL").await?,
            legacy.freshies.len() as i64,
        ),
        (
            "campaigns",
            scalar("SELECT COUNT(*) FROM campaigns").await?,
            legacy.campaigns.len() as i64,
        ),
        (
            "send batches",
            scalar("SELECT COUNT(*) FROM send_batches").await?,
            pastes,
        ),
        (
            "send records",
            scalar("SELECT COUNT(*) FROM send_records").await?,
            legacy.sent.len() as i64,
        ),
        (
            "batch counts that disagree with their records",
            scalar(
                "SELECT COUNT(*) FROM send_batches b WHERE recipient_count <> \
                 (SELECT COUNT(*) FROM send_records r WHERE r.batch_id = b.id)",
            )
            .await?,
            0,
        ),
    ];

    let mut failed = integrity != "ok" || fk_violations != 0;
    println!("integrity_check: {integrity}, foreign key violations: {fk_violations}");
    for (label, got, want) in checks {
        let mark = if got == want { "ok" } else { "MISMATCH" };
        failed |= got != want;
        println!("  {label}: {got} (expected {want}) {mark}");
    }
    let batches = scalar("SELECT COUNT(*) FROM import_batches").await?;
    println!("  import batches: {batches}");
    if failed {
        return Err("verification failed; do not use this database".to_owned());
    }
    Ok(())
}
