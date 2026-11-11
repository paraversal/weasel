//! Builds a deterministic demo database through the domain services.
//!
//! `seed-demo --data-dir <dir> [--contacts 500] [--seed 42] [--force]`
//!
//! Writes only via services, so seeding exercises validation, reconciliation and
//! audit trails instead of bypassing them with SQL.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use weasel_core::campaign::CampaignService;
use weasel_core::contact::ContactService;
use weasel_core::enums::{CampaignStatus, ImportScope};
use weasel_core::identity::NameIdentity;
use weasel_core::ports::{Clock, NoProgress};
use weasel_core::send::SendService;
use weasel_import::service::{ImportService, PreviewOptions};
use weasel_store::{SqliteCampaigns, SqliteContacts, SqliteImports, SqliteSends, Store};
use weasel_testkit::FakeClipboard;
use weasel_testkit::builders::{CampaignSeed, ContactSeed, parsed_source_owned};

const FIRST_NAMES: &[&str] = &[
    "Anna", "Ben", "Clara", "David", "Emil", "Frida", "Greta", "Hannes", "Ida", "Jonas", "Katrin",
    "Leon", "Mia", "Nils", "Olga", "Paul", "Quirin", "Rosa", "Stefan", "Tina",
];
const LAST_NAMES: &[&str] = &[
    "Albrecht",
    "Bauer",
    "Conradi",
    "Dietrich",
    "Engel",
    "Fischer",
    "Graf",
    "Huber",
    "Imhof",
    "Jäger",
    "Keller",
    "Lorenz",
    "Müller",
    "Neumann",
    "Ostermann",
    "Pfeiffer",
    "Richter",
    "Schulz",
    "Vogel",
    "Weber",
];

struct SeedClock {
    now: AtomicI64,
}

impl SeedClock {
    fn new() -> Self {
        Self {
            now: AtomicI64::new(1_700_000_000_000),
        }
    }
}

impl Clock for SeedClock {
    fn now_ms(&self) -> i64 {
        self.now.fetch_add(1, Ordering::Relaxed)
    }
}

/// xorshift64*, so a seed reproduces the same database without a `rand` dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

struct Args {
    data_dir: PathBuf,
    contacts: usize,
    seed: u64,
    force: bool,
}

fn usage() -> &'static str {
    "usage: seed-demo --data-dir <dir> [--contacts 500] [--seed 42] [--force]"
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut data_dir = None;
    let mut contacts = 500usize;
    let mut seed = 42u64;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                data_dir = Some(PathBuf::from(
                    args.next().ok_or("--data-dir needs a value")?,
                ));
            }
            "--contacts" => {
                contacts = args
                    .next()
                    .ok_or("--contacts needs a value")?
                    .parse()
                    .map_err(|_| "--contacts must be a positive integer".to_owned())?;
            }
            "--seed" => {
                seed = args
                    .next()
                    .ok_or("--seed needs a value")?
                    .parse()
                    .map_err(|_| "--seed must be an integer".to_owned())?;
            }
            "--force" => force = true,
            "--help" | "-h" => return Err(usage().to_owned()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }

    let data_dir = data_dir.ok_or_else(|| format!("--data-dir is required\n{}", usage()))?;
    if contacts == 0 {
        return Err("--contacts must be at least 1".to_owned());
    }
    Ok(Args {
        data_dir,
        contacts,
        seed,
        force,
    })
}

struct SeedServices {
    contacts: ContactService,
    campaigns: CampaignService,
    sends: SendService,
    imports: ImportService,
    clipboard: Arc<FakeClipboard>,
}

fn services(store: Store) -> SeedServices {
    let clock: Arc<dyn Clock> = Arc::new(SeedClock::new());
    let identity = Arc::new(NameIdentity);
    let clipboard = Arc::new(FakeClipboard::new());

    SeedServices {
        contacts: ContactService::new(
            Arc::new(SqliteContacts::new(store.clone())),
            clock.clone(),
            identity.clone(),
        ),
        campaigns: CampaignService::new(
            Arc::new(SqliteCampaigns::new(store.clone())),
            clock.clone(),
        ),
        sends: SendService::new(
            Arc::new(SqliteSends::new(store.clone())),
            clipboard.clone(),
            clock.clone(),
        ),
        imports: ImportService::new(Arc::new(SqliteImports::new(store.clone())), clock, identity),
        clipboard,
    }
}

fn person(index: usize, rng: &mut Rng) -> (String, String, String) {
    let first = (*rng.pick(FIRST_NAMES)).to_owned();
    let last = format!("{}{}", rng.pick(LAST_NAMES), index);
    let email = format!("person{index}@example.edu");
    (first, last, email)
}

fn apply_import(
    services: &SeedServices,
    name: &str,
    rows: &[(String, String, String)],
) -> Result<(), String> {
    let source = parsed_source_owned(name, rows);
    let preview = run(services.imports.preview(
        &source,
        PreviewOptions {
            mapping: None,
            scope: ImportScope::Roster,
        },
        &NoProgress,
    ))?;
    if !preview.conflicts.is_empty() || !preview.errors.is_empty() {
        return Err(format!(
            "{name}: {} conflicts, {} invalid rows",
            preview.conflicts.len(),
            preview.errors.len()
        ));
    }
    run(services.imports.apply(preview.batch.id, Vec::new()))?;
    println!(
        "{name}: {} added, {} updated, {} removed",
        preview.summary.added, preview.summary.updated, preview.summary.removed
    );
    Ok(())
}

fn run<T>(future: impl Future<Output = weasel_core::CoreResult<T>>) -> Result<T, String> {
    tauri_free_block_on(future).map_err(|err| err.to_string())
}

/// Small executor so the binary does not need a runtime feature choice beyond tokio.
fn tauri_free_block_on<T>(future: impl Future<Output = T>) -> T {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => panic!("seed runtime: {err}"),
    };
    runtime.block_on(future)
}

fn open_store(data_dir: &Path, force: bool) -> Result<Store, String> {
    let db_path = data_dir.join("weasel.sqlite3");
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(data_dir).map_err(|err| format!("create data dir: {err}"))?;

    if db_path.exists() {
        if !force {
            return Err(format!(
                "{} already exists; pass --force to replace it",
                db_path.display()
            ));
        }
        for path in [
            db_path.clone(),
            db_path.with_extension("sqlite3-wal"),
            db_path.with_extension("sqlite3-shm"),
        ] {
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|err| format!("remove {}: {err}", path.display()))?;
            }
        }
    }

    let (store, report) = tauri_free_block_on(weasel_store::open(weasel_store::OpenOptions {
        db_path: &db_path,
        backups_dir: &backups_dir,
    }))
    .map_err(|err| err.to_string())?;
    println!(
        "database {} (schema {})",
        db_path.display(),
        report.schema_version
    );
    Ok(store)
}

fn main() {
    if let Err(err) = run_main() {
        eprintln!("seed-demo: {err}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let args = parse_args()?;
    let store = open_store(&args.data_dir, args.force)?;
    let services = services(store);

    let mut rng = Rng::new(args.seed);
    let base: Vec<(String, String, String)> =
        (0..args.contacts).map(|i| person(i, &mut rng)).collect();
    apply_import(&services, "roster-1.csv", &base)?;

    let mut changed = Vec::with_capacity(base.len());
    for (index, (first, last, email)) in base.iter().enumerate() {
        if index % 11 == 0 {
            continue; // removed by this roster
        }
        if index % 7 == 0 {
            changed.push((
                first.clone(),
                last.clone(),
                format!("person{index}.new@example.edu"),
            ));
        } else {
            changed.push((first.clone(), last.clone(), email.clone()));
        }
    }
    let extras = args.contacts / 10 + 1;
    for offset in 0..extras {
        let index = args.contacts + offset;
        changed.push(person(index, &mut rng));
    }
    apply_import(&services, "roster-2.csv", &changed)?;

    let mut returnees: Vec<(String, String, String)> = base
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 11 == 0)
        .map(|(_, row)| row.clone())
        .collect();
    if returnees.is_empty() {
        returnees.push(base[0].clone());
    }
    for offset in 0..extras / 2 + 1 {
        let index = args.contacts + extras + offset;
        returnees.push(person(index, &mut rng));
    }
    apply_import(&services, "roster-3.csv", &returnees)?;

    for index in 0..5 {
        let seed = ContactSeed::new(
            rng.pick(FIRST_NAMES),
            &format!("Manuell{index}"),
            &format!("manual{index}@example.edu"),
        )
        .with_notes("added by hand");
        run(services.contacts.create_manual(seed.into_new_contact()))?;
    }
    println!("manual contacts: 5");

    let campaign_names = ["Rückblick", "Einladung", "Erinnerung", "Archiv"];
    let mut campaigns = Vec::new();
    for name in campaign_names {
        let campaign = run(services
            .campaigns
            .create(CampaignSeed::new(name).into_new_campaign()))?;
        campaigns.push(campaign);
    }
    let active: Vec<_> = campaigns.iter().map(|c| c.id).collect();
    for campaign in campaigns.iter().take(3) {
        run(services
            .campaigns
            .set_status(campaign.id, campaign.version, CampaignStatus::Active))?;
    }
    let archived = &campaigns[3];
    run(services
        .campaigns
        .set_status(archived.id, archived.version, CampaignStatus::Archived))?;
    println!("campaigns: 4 (1 archived)");

    for campaign_id in &active[..3] {
        let prepared = run(services.sends.prepare_send(*campaign_id, None))?;
        println!("send batch: {} recipients", prepared.emails.len());
    }
    if !services.clipboard.writes().is_empty() {
        println!(
            "clipboard writes: {} (last {} bytes)",
            services.clipboard.writes().len(),
            services.clipboard.writes().last().map_or(0, String::len)
        );
    }
    size_hint(&services, args.contacts)?;
    Ok(())
}

fn size_hint(services: &SeedServices, requested: usize) -> Result<(), String> {
    let active = run(services.contacts.count_active())?;
    println!("active contacts: {active} (requested {requested})");
    Ok(())
}
