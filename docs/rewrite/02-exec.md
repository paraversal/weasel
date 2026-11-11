# 02-exec — Execution plan for the target architecture

This document turns [02-architecture.md](02-architecture.md) into an ordered,
checkable work plan. It is written for an agent (or developer) who has **not** read the
planning conversation. Read this file top to bottom before you touch the repository, then
work through the milestones in order. Each milestone ends in a verifiable state and one
commit.

- **Written:** 2026-11-11
- **Estimated effort:** 4–6 ideal days (≈ phase 0 + phase 1 of
  [06-delivery-plan.md](06-delivery-plan.md), plus the database runtime from 02)
- **Prerequisite reading:** 02 (required), 04 §Errors/§Events/§Type generation, 05
  §Feature modules, 08 §Keyboard model/§Visual system. 03 only for the `settings` and
  `app_meta` DDL.

---

## 1. Premise change: this is a clean rewrite

The planning documents (01–09) were written assuming the TUI's SQLite data would be
migrated. **That is no longer the case.** The owner decided on 2026-11-11:

> The current codebase is just a reference implementation. Its data model and older
> databases do not have to be supported. Treat this as a complete rewrite.

Consequences for everything below, and for later exec plans:

| Planning-doc item | Status now |
| --- | --- |
| `weasel-store/src/legacy_import.rs`, legacy dry-run report (03, 07) | **Dropped** |
| `app_import_legacy_db` command (04) | **Dropped** |
| Legacy DB discovery on first run (07, 09 phase 7) | **Dropped** |
| `import_batches.source_kind = 'legacy'` (03) | **Drop the value** when 03 is executed |
| "Keep legacy TUI runnable until parity" (README, 06 phase 0, 09) | **Dropped.** The Python code is kept only as a read-only behavioural reference |
| Pre-migration backup of the *app's own* DB (02, 07) | **Kept** — this protects the new app's data across its own schema migrations |
| Feature parity with the TUI's *workflows* (01, 09) | **Kept** — behaviour, not data, is the contract |

M0 records this as ADR 0006 and puts a short "premise change" note into the affected
docs so later exec plans do not reintroduce legacy work.

## 2. Scope

### In scope (what "executing 02" means)

1. Repository layout from 02 §Repository layout: Rust workspace, five crates, `desktop/`
   SvelteKit app, `desktop/src-tauri/` adapter crate, `migrations/`, `tools/`.
2. The dependency rule from 02 §Layering, **enforced by a check**, not only by review.
3. Runtime composition from 02 §Runtime composition: data-dir resolution, SQLite open with
   WAL/foreign keys/busy timeout, backup-before-migrate, embedded forward-only
   migrations, `AppState`, plugins, logging.
4. The typed IPC seam from 04: `AppError`, generated `bindings.ts`, one event channel
   with a revision counter, `pnpm bindings` + CI diff check. Only the **app** and
   **settings** commands are implemented here.
5. Frontend architecture from 02 §Frontend architecture: shell (sidebar, top bar,
   command palette, toasts, error boundary), TanStack Query, event → invalidation bridge,
   theme without flash, feature registry from manifests.
6. Proof features: `dashboard`, `settings`, `help` (shortcut list only), and a hidden
   developer page listing frontend manifests and backend descriptors.
7. The feature generator (`pnpm gen feature …`) — required by phase 1 acceptance.
8. Security posture from 02: least-privilege capabilities, locked CSP.
9. CI skeleton and developer docs.

### Out of scope (owned by later exec plans)

| Item | Owner |
| --- | --- |
| Domain tables (contacts, campaigns, sends, imports, audit) and their repositories/services | 03-exec |
| The rest of the command surface (contacts/campaigns/sends/imports/search) and their events | 04-exec |
| Real feature modules beyond dashboard/settings/help; row/bulk action registry | 05-exec and later |
| Screens from 08 other than the shell, dashboard, settings, help | 08-exec |
| Spreadsheet parsing (`calamine`, `csv`) | 03/04-exec (import) |
| Updater, signing, notarization, installers | 07-exec |
| FTS5, virtualization, perf budgets | phase 6 |

Anything you are tempted to add that is not in the in-scope list: don't. Leave a
`TODO(03-exec)`-style note only where the next plan needs a hook.

## 3. Before you start — ask the owner

Stop and ask these before M0. Do not guess.

1. **Uncommitted work.** At planning time the working tree had: modified `README.md`
   (rewrite note — part of the plan) and `add_freshies.py` (scratch lines), and untracked
   `docs/`, `csp.ipynb`, `test.ipynb`, `src/conflict_resolution.py`. The notebooks may
   contain real names/addresses from rosters. **Do not commit the notebooks without the
   owner's say-so.** Proposed default: commit `docs/` + `README.md` as "docs: rewrite
   plan"; move `src/conflict_resolution.py` and `add_freshies.py` into the legacy folder
   as-is; leave the notebooks untracked and move them out of the repo root only if the
   owner agrees.
2. **Git permissions.** Default proposal: work on branch `rewrite/02-architecture`, one
   commit per milestone, no pushes and no tag pushes until the owner says so.
3. **Keep or delete the Python code?** Default: keep under `legacy/python-tui/` as a
   read-only reference (cheap, and 01 points into it). Deleting is equally valid now
   that data compatibility is gone; `git tag tui-final` preserves it either way.

## 4. Environment and versions

Observed on the owner's machine (macOS, Apple silicon) on 2026-11-11:

| Tool | Version | Note |
| --- | --- | --- |
| rustc / cargo | 1.99.1 | pin in `rust-toolchain.toml` |
| node | 24.1.0 | pin via `.node-version` and `engines` |
| pnpm | 12.14.0 | pin via `packageManager` |
| Xcode | installed | required by Tauri on macOS |
| `cargo tauri` | not installed | use `@tauri-apps/cli` from npm instead (`pnpm tauri …`) |

Latest versions resolved on 2026-11-11. **Use these majors.** Within a major, take the
latest patch at execution time and record what you used in the M10 README.

| Crate / package | Version | Notes |
| --- | --- | --- |
| `tauri` | **2.12.x** | crates.io "latest" is `3.0.0-alpha` — **do not use 3.x** |
| `tauri-build` | 2.7.x | |
| `@tauri-apps/cli`, `@tauri-apps/api` | 2.12.x | keep major.minor equal to the `tauri` crate or the CLI warns |
| `tauri-plugin-log` | 2.10.x | npm `@tauri-apps/plugin-log` matching |
| `tauri-plugin-dialog` | 2.8.x | Rust side only (see §M4 capabilities) |
| `tauri-plugin-clipboard-manager` | 2.5.x | Rust side only |
| `tauri-plugin-opener` | 2.7.x | Rust side only |
| `tauri-plugin-single-instance` | 2.5.x | |
| `tauri-specta` | 2.0.0-rc.28 | features `derive`, `typescript` |
| `specta` | 2.0.0-rc.28 | must be the exact version `tauri-specta` depends on (`cargo tree -i specta`); features `derive`, `serde_json` |
| `specta-typescript` | 0.0.15 | same rule |
| `sqlx` | 0.9.x | features: sqlite (bundled), tokio runtime, migrate, derive. **Verify the 0.9 feature names** (they were reshuffled across releases); if 0.9 does not compile with the rest, fall back to the latest 0.8.x and note it |
| `thiserror` | 2.x | |
| `tracing` | 0.1.x | enable its `log` feature so events reach `tauri-plugin-log` |
| `async-trait` | latest | for dyn-compatible async ports |
| `tempfile` | 3.x | testkit |
| `svelte` | 5.x (5.57) | runes |
| `@sveltejs/kit` | **3.x** (3.2) | 02 says "SvelteKit"; v3 is current. Use `$app/state`, not `$app/stores` |
| `@sveltejs/adapter-static` | 4.x | |
| `tailwindcss` | 4.x | via `@tailwindcss/vite`, no `tailwind.config.js` |
| `@tanstack/svelte-query` | **6.x** | v6 API takes **thunks**: `createQuery(() => ({ … }))`. The v5-style example in 04 is outdated |
| `bits-ui` | 2.x | primitives behind shadcn-svelte components |
| `@lucide/svelte` | 1.x | icons |
| `svelte-sonner` | latest | toast renderer, wrapped by our store |
| `vitest` | 5.x | |
| `@playwright/test` | 1.65.x | |
| `sv` | 1.3.x | SvelteKit scaffolder |

If a combination does not compile (most likely: `tauri-specta` rc vs `tauri` 2.12, or
SvelteKit 3 vs shadcn-svelte), resolve it by moving the *newer* side back one minor,
not by adopting alphas. Record the final pins in the README.

## 5. Repository facts that will bite you

Found while surveying the repo; each has a fix in the milestone noted.

1. **`.gitignore` ignores `lib/`** (Python boilerplate). That would silently ignore
   `desktop/src/lib/` — i.e. the whole frontend. → M0 rewrites `.gitignore`.
2. `.gitignore` ignores `*.db` and `*.xlsx`. Fine for user data, but future test fixtures
   need negations. → M0 adds `!**/tests/fixtures/**`.
3. The legacy app opens `weasel_lair.db` relative to the CWD and imports `src.*`. After
   the move it still runs from inside `legacy/python-tui/` with `uv run main.py`; no
   code edit is needed (and none is required any more).
4. `pyproject.toml` names the project `git`. Irrelevant after the move; don't fix it.
5. Compiling the Tauri crate needs `frontendDist` (`desktop/build/`) to exist at compile
   time (`generate_context!`). `cargo clippy/test` and `pnpm bindings` on a clean
   checkout fail without it. → M4 adds a tiny `build.rs` guard or the CI creates the
   directory; pick one, document it, and make clean-checkout `cargo test` work.
6. Compiling the Tauri crate on Linux needs WebKitGTK dev packages, **even for
   `cargo test` and the bindings export**. → M10 installs them in every Rust CI job.
7. `specta-typescript` refuses to export `i64`/`u64` by default (BigInt). IDs,
   millisecond timestamps and the event revision are all `i64`/`u64`. → M5 sets
   `BigIntExportBehavior::Number` and documents why it is safe (all values < 2^53).
8. pnpm workspaces keep **one lockfile at the root**, not `desktop/pnpm-lock.yaml` as
   02's tree shows. Follow pnpm, not the tree.

---

## 6. Target tree after this plan

Only what this plan creates. Files marked † are generated.

```text
weasel/
├── .editorconfig
├── .gitattributes
├── .gitignore                    # rewritten
├── .node-version                 # 24
├── .github/workflows/ci.yml
├── Cargo.toml                    # workspace
├── Cargo.lock
├── rust-toolchain.toml
├── package.json                  # root scripts only, private
├── pnpm-workspace.yaml
├── pnpm-lock.yaml
├── README.md                     # rewritten for the new app
├── crates/
│   ├── weasel-core/src/{lib.rs,error.rs,ports.rs,feature.rs,settings.rs}
│   ├── weasel-store/
│   │   ├── src/{lib.rs,pool.rs,error.rs,settings.rs,meta.rs}
│   │   └── tests/{pool.rs,settings.rs,fixtures/migrations_v1/,fixtures/migrations_v2/}
│   ├── weasel-import/src/lib.rs           # empty, doc comment only
│   ├── weasel-platform/src/{lib.rs,paths.rs,fs.rs,clock.rs}
│   └── weasel-testkit/src/{lib.rs,db.rs,clock.rs,clipboard.rs,dialogs.rs}
├── migrations/
│   └── 0001_app.sql              # settings + app_meta only
├── desktop/
│   ├── package.json
│   ├── svelte.config.js
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── eslint.config.js
│   ├── playwright.config.ts
│   ├── components.json           # shadcn-svelte
│   ├── src/
│   │   ├── app.html  app.css  app.d.ts
│   │   ├── routes/
│   │   │   ├── +layout.ts        # ssr = false
│   │   │   ├── +layout.svelte    # QueryClient, event bridge, AppShell, boot gate
│   │   │   ├── +error.svelte
│   │   │   ├── +page.svelte      # dashboard
│   │   │   ├── settings/+page.svelte
│   │   │   ├── help/+page.svelte
│   │   │   └── dev/features/+page.svelte
│   │   └── lib/
│   │       ├── ipc/{bindings.ts†,client.ts,queries.ts,events.ts,mock.ts}
│   │       ├── features/{types.ts,registry.ts,registry.test.ts}
│   │       ├── features/{dashboard,settings,help,developer}/manifest.ts (+ components/)
│   │       ├── shell/{AppShell,Sidebar,Topbar,CommandPalette,Toasts,ErrorBoundary,BootError}.svelte
│   │       ├── shell/icons.ts
│   │       ├── components/ui/…   # shadcn-svelte copies
│   │       ├── stores/{theme.svelte.ts,toasts.svelte.ts}
│   │       └── utils/{shortcuts.ts,format.ts}
│   ├── tests/
│   │   ├── boundaries.test.ts    # import-rule test
│   │   └── e2e/shell.spec.ts     # Playwright against mocked IPC
│   └── src-tauri/
│       ├── Cargo.toml  build.rs  tauri.conf.json
│       ├── capabilities/default.json
│       ├── icons/                # placeholder, from `pnpm tauri icon`
│       └── src/
│           ├── main.rs  lib.rs  state.rs  error.rs  events.rs  adapters.rs  bindings.rs
│           ├── bin/export_bindings.rs
│           └── commands/{mod.rs,app.rs,settings.rs}
├── legacy/python-tui/            # moved Python app + README "reference only"
├── tools/
│   ├── gen-feature.mjs
│   ├── gen-feature.test.mjs
│   └── check-layers.sh
└── docs/
    ├── adr/0006-clean-rewrite-no-legacy-data.md
    └── rewrite/02-exec.md        # this file (tick the checklist in §8)
```

Note: 02's tree lists `contact.rs`, `campaign.rs`, `contacts.rs`, etc. **Do not create
empty placeholder modules**; the owning exec plan adds them with content.

---

## 7. Milestones

Each milestone: goal → steps → done-when → commit message. Run the "done-when" checks
literally. If one fails, fix it before moving on; do not carry red checks forward.

### M0 — Preserve the reference and clean the root (≈0.5 d)

**Goal:** the root is free for the new layout; the old app is preserved; the docs no
longer promise legacy migration.

Steps:

1. Resolve §3 with the owner. Commit the planning docs first if agreed.
2. `git tag tui-final 8419e6e` (local only; the commit is the last TUI commit at
   planning time — re-check with `git log --oneline -1 -- src/` and tag that commit if
   newer TUI commits exist).
3. `git checkout -b rewrite/02-architecture`.
4. `mkdir -p legacy/python-tui` and `git mv main.py src pyproject.toml uv.lock
   .python-version add_freshies.py legacy/python-tui/`. Move untracked
   `src/conflict_resolution.py` with it (it travels inside `src/` automatically if you
   `mv` the folder; check `git status`). Leave the notebooks per §3.
5. Add `legacy/python-tui/README.md`:
   - "Reference implementation of the Textual TUI. Not maintained. Its database format is
     **not** supported by the new app."
   - How to run it if someone wants to look: `cd legacy/python-tui && uv run main.py`.
   - Pointer to `docs/rewrite/01-current-state.md` for the feature inventory.
6. Replace `.gitignore` with a short, rewrite-oriented file:

   ```gitignore
   # OS / editors
   .DS_Store
   .idea/
   .vscode/*
   !.vscode/extensions.json

   # Rust
   /target/

   # Node / SvelteKit
   node_modules/
   /desktop/build/
   /desktop/.svelte-kit/
   /desktop/test-results/
   /desktop/playwright-report/

   # Local data — never commit real rosters or databases
   *.db
   *.sqlite3
   *.sqlite3-*
   *.bak
   *.xlsx
   *.xls
   *.ods
   *.csv
   !**/tests/fixtures/**
   .dev-data/

   # Python reference app
   __pycache__/
   .venv/
   *.ipynb_checkpoints/
   .open

   # Env
   .env
   .env.*
   ```

   Verify nothing previously tracked became ignored: `git ls-files -ci --exclude-standard`
   must print nothing.
7. Write `docs/adr/0006-clean-rewrite-no-legacy-data.md` from the template (status
   Accepted, date 2026-11-11): context = §1 quote; decision = no legacy DB import, no
   discovery, Python app kept only as behavioural reference; consequences = table in §1;
   alternatives = "one-shot legacy import" (rejected: cost with no user need). Add it to
   `docs/adr/README.md`. Also list in the ADR the smaller deviations this plan makes
   (§9) so they have a durable home.
8. Put a two-line "Premise change (2026-11-11): no legacy data support — see ADR 0006"
   callout at the top of `docs/rewrite/README.md`, `03-data-model.md` (above "Legacy
   data mapping"), `04-ipc-and-events.md` (next to `app_import_legacy_db`),
   `06-delivery-plan.md` (phase 0/2/7), `07-testing-migration-release.md` (above
   "Legacy database migration"), `09-parity-checklist.md`. **Do not rewrite those
   sections**; their own exec plans will. Also change README decision 1's "kept under
   `legacy/` until parity is signed off" to "kept under `legacy/` as a reference".
9. Root `README.md`: replace the roadmap with a pointer to `docs/rewrite/` for now (M10
   writes the real one). Move the roadmap prose into
   `legacy/python-tui/README.md#original-roadmap` so it is not lost — 03/05 reference it.

Done when:

- `git tag --list tui-final` prints the tag.
- Root contains no Python files; `ls` shows only `docs/ legacy/ LICENSE README.md`
  plus dotfiles (and the notebooks if the owner chose to leave them).
- `git ls-files -ci --exclude-standard` is empty.

Commit: `chore: move TUI to legacy/python-tui, record clean-rewrite ADR`

---

### M1 — Workspace roots and toolchain pins (≈0.25 d)

**Goal:** empty-but-valid Rust and pnpm workspaces with pinned toolchains.

Files:

`rust-toolchain.toml`

```toml
[toolchain]
channel = "1.99.1"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

`Cargo.toml` (root) — sketch:

```toml
[workspace]
resolver = "3"
members = ["crates/*", "desktop/src-tauri"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "<SPDX id matching LICENSE>"   # read LICENSE and set accordingly
repository = "https://github.com/paraversal/weasel"
publish = false

[workspace.dependencies]
weasel-core     = { path = "crates/weasel-core" }
weasel-store    = { path = "crates/weasel-store" }
weasel-import   = { path = "crates/weasel-import" }
weasel-platform = { path = "crates/weasel-platform" }
weasel-testkit  = { path = "crates/weasel-testkit" }

serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "2"
async-trait = "0.1"
tracing     = { version = "0.1", features = ["log"] }
tokio       = { version = "1", features = ["rt-multi-thread", "macros"] }
sqlx        = { version = "0.9", default-features = false, features = [ "<verify: sqlite bundled, runtime-tokio, migrate, derive>" ] }
tempfile    = "3"
# tauri + plugins + specta pinned here too (see §4)

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
unwrap_used = "warn"     # tests may `#[allow]`
expect_used = "warn"
dbg_macro = "warn"

[profile.dev.package.sqlx-macros]
opt-level = 3
```

Every crate's `Cargo.toml` uses `version.workspace = true`, `edition.workspace = true`,
`[lints] workspace = true`, and only `workspace = true` dependency references.

`package.json` (root):

```json
{
  "name": "weasel-workspace",
  "private": true,
  "packageManager": "pnpm@12.14.0",
  "engines": { "node": ">=24" },
  "scripts": {
    "dev": "pnpm -C desktop tauri dev",
    "dev:web": "pnpm -C desktop dev:mock",
    "tauri": "pnpm -C desktop tauri",
    "build": "pnpm -C desktop build",
    "check": "pnpm -C desktop check",
    "lint": "pnpm -C desktop lint",
    "test": "pnpm -C desktop test && node --test tools/",
    "test:e2e": "pnpm -C desktop test:e2e",
    "bindings": "cargo run -q -p weasel-desktop --bin export-bindings",
    "gen": "node tools/gen-feature.mjs",
    "check:layers": "bash tools/check-layers.sh",
    "verify": "pnpm check && pnpm lint && pnpm test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && pnpm check:layers"
  }
}
```

`pnpm-workspace.yaml`: `packages: ["desktop"]`. `.node-version`: `24`.
`.editorconfig`: LF, UTF-8, final newline; 4 spaces for `*.rs`, 2 for everything else.
`.gitattributes`: `* text=auto eol=lf`, and
`desktop/src/lib/ipc/bindings.ts linguist-generated=true`.

Done when: `cargo metadata --format-version 1 >/dev/null` succeeds (after M2 adds at
least one member) and `pnpm --version` matches `packageManager`. Commit together with M2.

---

### M2 — Rust crates and the enforced dependency rule (≈1 d)

**Goal:** the five crates compile, the ports/error/descriptor/settings patterns that
every later feature copies exist, and a script fails the build if the layering rule is
broken.

#### weasel-core (pure domain)

Dependencies: `serde`, `serde_json`, `thiserror`, `async-trait`. **Nothing else.** No
`tokio`, `sqlx`, `tauri`, `specta`, `calamine`.

- `error.rs` — the domain error every service returns:

  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum CoreError {
      #[error("invalid {field}: {reason}")]
      Validation { field: &'static str, reason: String },
      #[error("{entity} {id} not found")]
      NotFound { entity: &'static str, id: String },
      #[error("{0}")]
      Conflict(String),
      #[error("{0}")]
      InvalidState(String),
      #[error("i/o: {0}")]
      Io(String),
      #[error(transparent)]
      Internal(Box<dyn std::error::Error + Send + Sync>),
  }
  pub type CoreResult<T> = Result<T, CoreError>;
  ```

  Repository adapters convert their own errors into `CoreError` (see store), so the
  adapter crate only ever maps `CoreError → AppError`.

- `ports.rs` — dyn-compatible ports (`#[async_trait]` where async):
  - `Clock { fn now_ms(&self) -> i64; }`
  - `Clipboard { async fn write_text(&self, text: String) -> CoreResult<()>; }`
  - `FileDialog { async fn pick_file(&self, filters: &[FileFilter]) -> CoreResult<Option<PathBuf>>; }`
    with `FileFilter { name: String, extensions: Vec<String> }`
  - `SettingsRepository { async fn load(&self) -> CoreResult<Vec<(String, serde_json::Value)>>;
    async fn save(&self, entries: Vec<(String, serde_json::Value)>, now_ms: i64) -> CoreResult<()>; }`
    (`save` is one transaction.)
  - Do **not** add repository traits for contacts etc. yet.

- `settings.rs` — the first application service and the template for all others:
  - `enum Theme { System, Light, Dark }` (serde `lowercase`), `Default = System`.
  - `struct Settings { theme: Theme, feature_flags: BTreeMap<String, bool> }` with
    `Default`.
  - `struct SettingsPatch { theme: Option<Theme>, feature_flags: Option<BTreeMap<String, bool>> }`;
    `validate()` rejects flag ids that are not `[a-z0-9-]{1,64}` with
    `CoreError::Validation { field: "featureFlags", … }`.
  - Storage keys: `"theme"`, `"featureFlags"`. Unknown stored keys are ignored; missing
    keys fall back to defaults; a stored value that fails to deserialize falls back to
    default **and is logged** (never crash on bad settings).
  - `SettingsService { repo: Arc<dyn SettingsRepository>, clock: Arc<dyn Clock> }` with
    `get() -> Settings` and `update(patch) -> (Settings, Vec<&'static str> /* changed keys */)`.
    Feature flags merge (patch keys overwrite, others kept).
  - Unit tests with in-file fakes (testkit is a dev-dependency, fine to use).

- `feature.rs` — from 05:

  ```rust
  pub struct FeatureDescriptor {
      pub id: &'static str,
      pub title: &'static str,
      pub commands: &'static [&'static str],
      pub events: &'static [&'static str],
  }
  ```

  Drop 05's `migrations` field: migrations are global and ordered, not per feature
  (see §9).

#### weasel-store (sqlx adapter)

Dependencies: `weasel-core`, `sqlx`, `tokio` (only what sqlx needs), `serde_json`,
`thiserror`, `tracing`.

- `pool.rs` — `Store` wraps `SqlitePool` (cheap to clone).

  ```rust
  pub struct OpenOptions<'a> { pub db_path: &'a Path, pub backups_dir: &'a Path }
  pub struct OpenReport { pub schema_version: i64, pub backup: Option<PathBuf>, pub created: bool }

  pub async fn open(opts: OpenOptions<'_>) -> Result<(Store, OpenReport), StoreError> {
      open_with(opts, &sqlx::migrate!("../../migrations")).await
  }
  // pub(crate) + #[doc(hidden)] pub for tests:
  pub async fn open_with(opts: OpenOptions<'_>, migrator: &Migrator) -> Result<(Store, OpenReport), StoreError>
  ```

  Order inside `open_with`:
  1. `created = !db_path.exists()`.
  2. Connect with `SqliteConnectOptions`: `create_if_missing(true)`,
     `journal_mode(Wal)`, `foreign_keys(true)`, `busy_timeout(5s)`,
     `synchronous(Normal)`. Pool `max_connections(4)`.
  3. Determine pending migrations: applied versions from `_sqlx_migrations` (table may be
     absent) vs `migrator.iter()`. Also **refuse** (error `StoreError::NewerSchema`) if
     the DB has a version the binary does not know — this protects against running an
     older app build on a newer DB.
  4. If `!created` and pending is non-empty: backup with
     `VACUUM INTO '<backups_dir>/weasel.sqlite3.pre-v<target>-<yyyyMMddTHHmmss>.bak'`
     (consistent with WAL, no file-copy races). Bind the path as a parameter. Create
     `backups_dir` first.
  5. `migrator.run(&pool)`.
  6. If a backup was made, upsert `app_meta('last_backup_path', …)`. On first creation
     write `app_meta('created_at', now)` and `app_meta('instance_id', <random hex>)`.
  7. Sanity: `PRAGMA foreign_keys` returns 1, `PRAGMA journal_mode` returns `wal`;
     otherwise error. Return `schema_version` = max successful version.

  Also expose `Store::ping()` (`SELECT 1`), `Store::schema_version()`, and
  `Store::backup_now(dir)` (same `VACUUM INTO`, used later by `app_backup_create`).

- `error.rs` — `StoreError` (`Sqlx`, `Migrate`, `NewerSchema { db: i64, app: i64 }`,
  `Io`) and `impl From<StoreError> for CoreError`: unique-constraint violation →
  `Conflict`, `RowNotFound` → `NotFound`, everything else → `Internal`.
- `settings.rs` — `SqliteSettings(Store)` implements `SettingsRepository`. `save` is a
  single transaction of `INSERT … ON CONFLICT(key) DO UPDATE`. Use runtime-checked
  `sqlx::query`/`query_as` (not the `query!` macros — no `DATABASE_URL`/offline data in
  this phase; 03-exec may adopt them).
- `meta.rs` — `get_meta(key)`, `set_meta(key, value)`.
- Transaction rule, documented in `lib.rs` docs: every write path takes `&mut
  Transaction` or opens one; there are no autocommit multi-statement writes.

`migrations/0001_app.sql` — copy the `settings` and `app_meta` DDL **verbatim** from
03 §Initial schema. Nothing else. 03-exec adds the domain tables as `0002_domain.sql`
(see §9 for why this is split).

Tests (`crates/weasel-store/tests/`), using `weasel-testkit`:

- fresh open creates the file, `created = true`, `schema_version = 1`, no backup;
- `foreign_keys = 1`, `journal_mode = wal`;
- reopen with nothing pending → no backup;
- open with fixture migrator v1, then with v2 (fixtures: `tests/fixtures/migrations_v1/`
  has one trivial migration, `_v2/` has the same plus one more) → backup file exists,
  is a valid SQLite DB containing v1 only, and `app_meta.last_backup_path` points at it;
- open a DB migrated with v2 using migrator v1 → `NewerSchema`;
- settings save/load round trip; forced failure mid-`save` (e.g. second entry with
  invalid JSON violating `json_valid`) rolls back the first entry.

#### weasel-import

Only `lib.rs` with a module doc: "Parsing, column mapping and reconciliation. Populated
by 03/04-exec. Depends on weasel-core only." Dependency: `weasel-core`. It exists now
so the layering script covers it from day one.

#### weasel-platform (OS concerns without Tauri)

Dependencies: `weasel-core`. No Tauri.

- `paths.rs` — `DataPaths { data_dir, db_path, backups_dir, logs_dir }` with
  `DataPaths::resolve(app_data_dir: PathBuf, override_dir: Option<PathBuf>, dev: bool)`:
  - `WEASEL_DATA_DIR` (passed in as `override_dir`) wins;
  - else in debug builds (`dev = true`): `<app_data_dir>/dev/` so development never
    touches real data;
  - else `<app_data_dir>/`.
  - `db_path = data_dir/weasel.sqlite3`, `backups_dir = data_dir/backups`.
  - `ensure()` creates the directories.
  The "DB path overridable in settings" idea from 02 cannot live in the DB itself
  (chicken-and-egg); it is deferred to phase 6 as a small file in the config dir.
- `fs.rs` — `validate_source_path(path, allowed_exts) -> CoreResult<PathBuf>`:
  canonicalize, must be a regular file, extension in the allowlist (case-insensitive).
  Used by the import feature later; tested now.
- `clock.rs` — `SystemClock` implementing `Clock`.

#### weasel-testkit

Dependencies: `weasel-core`, `weasel-store`, `tempfile`, `tokio`. Used only as a
`dev-dependency`.

- `TempDb::new().await` → `TempDir` + migrated `Store` (+ `path()`).
- `FixedClock` (settable, `advance(ms)`).
- `FakeClipboard` (records writes; `fail_next()` makes the next write return
  `CoreError::Io`).
- `FakeFileDialog` (queue of `Option<PathBuf>` answers).

#### Layer check

`tools/check-layers.sh` (bash, `set -euo pipefail`):

```bash
deny() { # crate, regex of forbidden deps
  if cargo tree -p "$1" -e normal --prefix none --format '{p}' | grep -Eq "^($2) "; then
    echo "layering violation: $1 depends on one of: $2" >&2; exit 1; fi; }
deny weasel-core     'tauri.*|sqlx.*|calamine|specta.*|tokio|weasel-store|weasel-import|weasel-platform'
deny weasel-import   'tauri.*|sqlx.*|weasel-store|weasel-platform'
deny weasel-store    'tauri.*|specta.*|weasel-import|weasel-platform'
deny weasel-platform 'tauri.*|sqlx.*|weasel-store|weasel-import'
echo "layers ok"
```

(Checks transitive normal deps, which is what we want. Verify the `--format` output
shape with your cargo version and adjust the regex anchor.)

Done when:

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` pass (the Tauri crate does not exist yet).
- `pnpm check:layers` prints `layers ok`; temporarily adding `sqlx` to `weasel-core`
  makes it fail (try it, then revert).

Commit: `feat(core): rust workspace, ports, settings service, sqlite store with backup-before-migrate`

---

### M3 — SvelteKit app scaffold (≈0.5 d)

**Goal:** `desktop/` builds to a static SPA with Tailwind, strict TS, lint, Vitest and
Playwright wired.

Steps:

1. From the repo root: `pnpm dlx sv create desktop` — template *minimal*, TypeScript,
   add-ons: tailwindcss, eslint, prettier, vitest, playwright. Use the non-interactive
   flags of the `sv` version you have (`pnpm dlx sv create --help`); pnpm as the package
   manager. Delete any demo routes it creates.
2. `pnpm -C desktop add -D @sveltejs/adapter-static` and remove `adapter-auto`.
   `svelte.config.js`:

   ```js
   import adapter from "@sveltejs/adapter-static";
   import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
   export default {
     preprocess: vitePreprocess(),
     kit: { adapter: adapter({ fallback: "index.html" }) },
   };
   ```

3. `src/routes/+layout.ts`: `export const ssr = false; export const prerender = false;`
   (ADR 0005). `pnpm build` must produce `desktop/build/index.html`.
4. `vite.config.ts`: Tauri-recommended dev server settings — port `1420`,
   `strictPort: true`, `clearScreen: false`, `host: process.env.TAURI_DEV_HOST || false`,
   `server.watch.ignored: ["**/src-tauri/**"]`, `envPrefix: ["VITE_", "TAURI_ENV_"]`.
   Vitest config: `environment: "jsdom"` (or `happy-dom`) for `*.svelte.test.ts` and
   component tests, node for the rest; include `tests/**/*.test.ts` and `src/**/*.test.ts`.
5. `tsconfig.json`: extends the generated `.svelte-kit/tsconfig.json`, `strict: true`,
   `noUncheckedIndexedAccess: true`, `noImplicitOverride: true`.
6. Tailwind v4: `@tailwindcss/vite` plugin; `src/app.css`:
   - `@import "tailwindcss";`
   - `@custom-variant dark (&:where(.dark, .dark *));`
   - semantic tokens from 08 §Visual system as CSS variables in `:root` and `.dark`
     (`--surface`, `--surface-raised`, `--text`, `--text-muted`, `--border-subtle`,
     `--accent`, `--danger`, `--success`, `--warning`, `--focus-ring`), exposed through
     `@theme inline { --color-surface: var(--surface); … }` so classes like `bg-surface`,
     `text-muted`, `border-subtle` exist. Respect `prefers-reduced-motion`.
   - Before-paint theme: in `src/app.html` `<head>`, a tiny inline script reads
     `localStorage["weasel.theme"]` (`system|light|dark`) and `matchMedia`, then sets
     `document.documentElement.classList.toggle("dark", …)`. The backend setting is
     authoritative; localStorage is only a cache to avoid flashing (see M7 theme store).
7. shadcn-svelte: `pnpm dlx shadcn-svelte@latest init` (aliases under `$lib/components/ui`,
   base colour neutral, CSS variables mapped to the tokens above), then add only:
   `button`, `dialog`, `command`, `dropdown-menu`, `tooltip`, `badge`, `card`,
   `separator`, `sonner`, `kbd` (if available). If the CLI does not support SvelteKit 3
   yet, copy the component sources manually from the shadcn-svelte repo (they are
   plain bits-ui wrappers) — do not downgrade SvelteKit for this.
8. Icons: `pnpm -C desktop add @lucide/svelte`.
9. ESLint (flat config) additions:
   - `no-restricted-imports`: forbid `@tauri-apps/api/core` and `@tauri-apps/api/event`
     everywhere except `src/lib/ipc/**`; forbid `svelte-sonner` except
     `src/lib/stores/toasts.svelte.ts` and `src/lib/shell/Toasts.svelte` and
     `src/lib/components/ui/sonner/**`.
   - Ignore `src/lib/ipc/bindings.ts` (also in `.prettierignore`).
10. `desktop/package.json` scripts:
    `dev`, `dev:mock` (`VITE_IPC_MOCK=1 vite dev`), `build`, `preview`,
    `check` (`svelte-kit sync && svelte-check --tsconfig ./tsconfig.json --fail-on-warnings`),
    `lint` (`prettier --check . && eslint .`), `format`, `test` (`vitest run`),
    `test:e2e` (`playwright test`), `tauri` (`tauri`).

Done when: `pnpm install`, `pnpm check`, `pnpm lint`, `pnpm test` (zero or placeholder
tests pass), `pnpm build` all succeed, and `desktop/build/index.html` exists.

Commit: `feat(desktop): sveltekit static spa scaffold with tailwind, lint and tests`

---

### M4 — Tauri adapter crate and runtime composition (≈1 d)

**Goal:** `pnpm dev` opens a native window showing the SvelteKit app; the backend has
resolved its data dir, opened/migrated the DB, and holds `AppState`.

Steps:

1. `pnpm -C desktop add -D @tauri-apps/cli@~2.12` and `pnpm -C desktop add
   @tauri-apps/api@~2.12 @tauri-apps/plugin-log@~2.10`. Run `pnpm -C desktop tauri init`
   with: app name `Weasel`, window title `Weasel`, frontend dist `../build`, dev URL
   `http://localhost:1420`, before-dev `pnpm dev`, before-build `pnpm build`. Then
   reshape the generated crate as below.
2. `desktop/src-tauri/Cargo.toml`: package `weasel-desktop`, lib name
   `weasel_desktop_lib`, workspace inheritance, `[[bin]] name = "weasel" path =
   "src/main.rs"` and `[[bin]] name = "export-bindings" path =
   "src/bin/export_bindings.rs"`. Dependencies: `weasel-core`, `weasel-store`,
   `weasel-platform`, `tauri` (2.12), the plugins from §4, `tauri-specta`, `specta`,
   `specta-typescript`, `serde`, `serde_json`, `tracing`, `tokio`, `async-trait`.
   Dev-deps: `weasel-testkit`.
3. **frontendDist at compile time** (§5.5): add to `build.rs` before
   `tauri_build::build()`: create `../build` with an empty `index.html` placeholder if
   it does not exist. Verify `cargo test --workspace` works from a clean clone (delete
   `desktop/build` and retry). If you prefer the CI `mkdir` approach, document it in the
   README instead.
4. `tauri.conf.json` (key points; keep the rest default):
   - `productName: "Weasel"`, `identifier: "dev.paraversal.weasel"` (open question in
     the README; fine for dev), version from the workspace `Cargo.toml` (omit `version`).
   - one window: `label: "main"`, `title: "Weasel"`, `width: 1280`, `height: 800`,
     `minWidth: 960`, `minHeight: 600`.
   - `app.security.csp`:
     `"default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:; font-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"`.
     Tauri injects hashes for the inline scripts it finds in bundled HTML (SvelteKit's
     bootstrap script and our theme script). `style-src 'unsafe-inline'` is needed for
     Svelte transitions/inline styles.
   - `app.security.devCsp`: same plus `ws://localhost:1420 http://localhost:1420` in
     `connect-src` for Vite HMR.
   - `app.security.freezePrototype: true`.
   - `bundle.active: true`, `targets: "all"`; icons from `pnpm tauri icon` with any
     square placeholder PNG (real icon is phase 7).
5. `capabilities/default.json` — **tighter than 02** (see §9): the frontend never calls
   dialog/clipboard/opener directly; the backend does through ports.

   ```json
   {
     "$schema": "../gen/schemas/desktop-schema.json",
     "identifier": "default",
     "windows": ["main"],
     "permissions": ["core:default", "log:default"]
   }
   ```

6. `src/lib.rs` → `pub fn run()`:
   1. Build the specta builder from `commands::builder()` (M5).
   2. `tauri::Builder::default()`
      - `.plugin(tauri_plugin_single_instance::init(|app, _, _| focus main window))`
        **first**;
      - `.plugin(tauri_plugin_log::Builder::new()…)`: targets Stdout (debug only) +
        LogDir; `RotationStrategy::KeepSome(5)`, max file size 5 MB; level `Info`
        (`Debug` in debug builds);
      - `.plugin(tauri_plugin_dialog::init())`, `.plugin(tauri_plugin_clipboard_manager::init())`,
        `.plugin(tauri_plugin_opener::init())`;
      - `.invoke_handler(builder.invoke_handler())`;
      - `.setup(move |app| { builder.mount_events(app); boot(app); Ok(()) })`.
   3. `boot(app)`:
      - `DataPaths::resolve(app.path().app_data_dir()?, env WEASEL_DATA_DIR,
        cfg!(debug_assertions))`, `ensure()`.
      - `tauri::async_runtime::block_on(weasel_store::open(…))`.
      - On success: construct `SqliteSettings`, `SystemClock`, `TauriClipboard`,
        `TauriFileDialog`, `SettingsService`; manage `AppState` with
        `boot: Ok(Services { … })`. Log the open report (path, version, backup).
      - On failure: **do not abort**. Manage `AppState` with `boot: Err(BootError {
        message, data_dir })`, log it at `error`. The UI shows a boot-error screen (M7)
        with "Open data folder". A desktop app that silently exits on a locked or
        corrupt DB is the worst outcome.
7. `src/state.rs`:

   ```rust
   pub struct AppState {
       pub paths: DataPaths,
       pub boot: Result<Services, BootError>,
       pub events: EventBus,
   }
   pub struct Services {
       pub store: Store,
       pub clock: Arc<dyn Clock>,
       pub clipboard: Arc<dyn Clipboard>,
       pub dialogs: Arc<dyn FileDialog>,
       pub settings: SettingsService,
   }
   impl AppState {
       pub fn services(&self) -> Result<&Services, AppError> { /* Err → INTERNAL "database unavailable" */ }
   }
   ```

8. `src/adapters.rs`: `TauriClipboard(AppHandle)` → `app.clipboard().write_text(…)`;
   `TauriFileDialog(AppHandle)` → `app.dialog().file().add_filter(…).blocking_pick_file()`
   inside `tokio::task::spawn_blocking` (never block the async runtime or the main
   thread). Map plugin errors to `CoreError::Io`. Not used by any command yet except
   through tests; 04-exec's `imports_pick_source` and `campaigns_prepare_send` use them.
9. `src/main.rs`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
   and `weasel_desktop_lib::run()`.

Done when:

- `pnpm dev` opens a window on macOS showing the (still bare) SvelteKit page.
- `<app_data_dir>/dev/weasel.sqlite3` exists (macOS:
  `~/Library/Application Support/dev.paraversal.weasel/dev/`), with `settings` and
  `app_meta` tables; log file present in the app log dir.
- Starting a second instance focuses the first window instead of opening another.
- With `WEASEL_DATA_DIR=/tmp/x` pointing at a file instead of a dir (or a DB file made
  read-only), the app still opens (boot error recorded; UI arrives in M7).
- `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm check:layers` pass.

Commit: `feat(desktop): tauri shell, plugins, csp, data dir and db boot`

---

### M5 — Typed IPC: errors, commands, events, bindings (≈0.75 d)

**Goal:** a reviewable, generated IPC contract with the first real commands and the
event channel.

1. `src/error.rs` — `AppError`/`ErrorCode` exactly as in 04 §Errors (`Serialize`,
   `specta::Type`, `camelCase` fields, `SCREAMING_SNAKE_CASE` codes). Mapping:

   | `CoreError` | `ErrorCode` | `retryable` | `details` |
   | --- | --- | --- | --- |
   | `Validation { field, reason }` | `VALIDATION` | false | `{ "field", "reason" }` |
   | `NotFound` | `NOT_FOUND` | false | `{ "entity", "id" }` |
   | `Conflict` | `CONFLICT` | false | — |
   | `InvalidState` | `INVALID_STATE` | false | — |
   | `Io` | `IO` | true | — |
   | `Internal` | `INTERNAL` | false | `{ "logId" }` |

   For `INTERNAL`, log the full error with a short `logId` (e.g. base36 of
   `now_ms` + counter) and return a generic message containing that id. Never put SQL
   or file contents in messages. Add `fn redact_emails(&str) -> String` (regex-free
   simple scanner is fine) and use it on every logged error string.
2. `src/events.rs` — **envelope instead of per-variant revision** (see §9):

   ```rust
   #[derive(Clone, Serialize, specta::Type)]
   #[serde(tag = "type", rename_all = "camelCase")]
   pub enum DomainEvent {
       SettingsChanged { keys: Vec<String> },
       OperationFailed { operation: String, error: AppError },
       // later exec plans add: ContactsChanged, CampaignsChanged, SendBatchesChanged,
       // ImportsChanged, ImportProgress, ImportFinished
   }

   #[derive(Clone, Serialize, specta::Type, tauri_specta::Event)]
   pub struct WeaselEvent { pub revision: u64, pub event: DomainEvent }

   pub struct EventBus { revision: AtomicU64 }
   impl EventBus {
       pub fn next_revision(&self) -> u64 { self.revision.fetch_add(1, Ordering::SeqCst) + 1 }
       /// Call only after the transaction that caused the change has committed.
       pub fn emit(&self, app: &AppHandle, event: DomainEvent) { /* WeaselEvent{..}.emit(app); log on failure */ }
   }
   ```

   The channel name is whatever `tauri_specta::Event` derives from `WeaselEvent`
   (expected `weasel-event`); the frontend only uses the generated `events.weaselEvent`,
   so 04's literal `weasel://event` is not needed.
3. `src/commands/app.rs` — descriptor + commands (all `#[tauri::command]
   #[specta::specta] async fn …(state: State<'_, AppState>, …) -> Result<_, AppError>`):

   | Command | Response DTO | Notes |
   | --- | --- | --- |
   | `app_info` | `AppInfoDto { name, version, tauriVersion, apiVersion, dbPath, dataDir }` | `apiVersion = "0.1.0"` const; works even if boot failed |
   | `app_health` | `AppHealthDto { dbOk, schemaVersion: Option<i64>, bootError: Option<String> }` | works even if boot failed; `dbOk` = boot ok && `ping()` ok. 03/04-exec add `contactCount`, `pendingImports` as optional fields (additive) |
   | `app_features` | `Vec<FeatureDescriptorDto>` | from `commands::feature_descriptors()`; powers the developer page |
   | `app_open_data_dir` | `()` | opener plugin `open_path(data_dir)` from Rust |

4. `src/commands/settings.rs`: `settings_get_all() -> SettingsDto` and
   `settings_update(request: SettingsUpdateRequest { patch: SettingsPatchDto }) ->
   SettingsDto`. `update` calls the service, then `events.emit(SettingsChanged { keys })`
   only if keys changed. DTOs mirror the core types with `camelCase`; conversion is
   `From` impls in the command module (domain types never derive `specta::Type`).
5. `src/commands/mod.rs` — with **generator markers** (M9 depends on these exact lines):

   ```rust
   pub mod app;
   pub mod settings;
   // @gen:modules

   pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
       vec![
           app::descriptor(),
           settings::descriptor(),
           // @gen:descriptors
       ]
   }

   pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
       tauri_specta::Builder::<tauri::Wry>::new()
           .commands(tauri_specta::collect_commands![
               app::app_info,
               app::app_health,
               app::app_features,
               app::app_open_data_dir,
               settings::settings_get_all,
               settings::settings_update,
               // @gen:commands
           ])
           .events(tauri_specta::collect_events![crate::events::WeaselEvent])
   }
   ```

6. `src/bindings.rs` + `src/bin/export_bindings.rs`: export to
   `concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc/bindings.ts")` with
   `Typescript::default().bigint(BigIntExportBehavior::Number).header("// @generated by `pnpm bindings` — do not edit\n")`.
   Then run prettier on the output? **No** — the file is prettier-ignored; keep the
   exporter output byte-for-byte deterministic. Do not export at app startup.
7. Tests in the adapter crate:
   - `CoreError → AppError` mapping table (each row above).
   - `EventBus::next_revision` is strictly increasing across threads.
   - **Descriptor/registration consistency:** read the generated `bindings.ts`, extract
     every `invoke("…")` name, and assert it equals the union of all
     `descriptor().commands` — both directions. This catches "registered but not
     advertised" and "advertised but not registered".
   - Settings command path with `TempDb`: update theme → `get_all` reflects it.

Done when:

- `pnpm bindings` writes `desktop/src/lib/ipc/bindings.ts` containing `commands.appInfo`,
  `commands.settingsUpdate`, the `AppError` type and `events.weaselEvent`; running it
  twice produces no diff.
- `cargo test --workspace` passes, including the consistency test.

Commit: `feat(ipc): app/settings commands, AppError, event envelope, generated bindings`

---

### M6 — Frontend IPC layer and query plumbing (≈0.5 d)

**Goal:** components never see raw `invoke` or result unions; server state is cached by
query key and refreshed by backend events.

1. `lib/ipc/client.ts`

   ```ts
   import type { AppError } from "./bindings";
   export class IpcError extends Error {
     constructor(public readonly error: AppError) { super(error.message); }
     get code() { return this.error.code; }
     get retryable() { return this.error.retryable; }
   }
   type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };
   /** Unwrap a tauri-specta result; also converts unexpected rejections into INTERNAL. */
   export async function call<T>(p: Promise<Result<T>>): Promise<T> { … }
   export function isIpcError(e: unknown): e is IpcError { … }
   export function fieldError(e: unknown, field: string): string | undefined { … }
   ```

   Check the exact result shape in the generated file and match it.
2. `lib/ipc/queries.ts` — the only place query keys are built:

   ```ts
   export const keys = {
     app: { info: ["app", "info"] as const, health: ["app", "health"] as const,
            features: ["app", "features"] as const },
     settings: ["settings"] as const,
   };
   export const appQueries = {
     info: () => ({ queryKey: keys.app.info, queryFn: () => call(commands.appInfo()) }),
     …
   };
   ```

   Feature-specific query modules (later) live in the feature folder and reuse `call`.
3. `lib/ipc/events.ts` — typed invalidation map and bridge:

   ```ts
   type EventType = WeaselEvent["event"]["type"];
   export const invalidations = {
     settingsChanged: [keys.settings],
     operationFailed: [],                     // handled: toast + console.error
   } satisfies Record<EventType, readonly QueryKey[]>;   // missing key = type error
   export function startEventBridge(qc: QueryClient, onFailed: (e: AppError) => void): () => void
   ```

   Behaviour: drop events whose `revision <= lastSeen`; unknown `type` → ignore (open
   union, 04 §Compatibility); `operationFailed` → `onFailed`. Returns the unlisten
   function.
4. `lib/ipc/mock.ts` — when `import.meta.env.VITE_IPC_MOCK === "1"`, install
   `mockIPC` from `@tauri-apps/api/mocks` with a handler table typed against the command
   names (`Record<string, (args) => unknown>` keyed by snake_case command), backed by an
   in-memory settings object. Imported dynamically from `+layout.svelte` only in that
   mode so it is tree-shaken from production builds. This powers `pnpm dev:web` and
   Playwright.
5. `+layout.svelte`: create `QueryClient` with defaults `staleTime: 30_000`,
   `refetchOnWindowFocus: false`, `retry: (n, e) => isIpcError(e) && e.retryable && n < 2`;
   wrap in `QueryClientProvider`; start the event bridge in `onMount` and stop it on
   destroy.
6. Tests (Vitest): `call` unwraps ok/error/rejection; bridge drops stale revisions,
   invalidates the mapped keys, ignores unknown types (mock `events.weaselEvent.listen`).

Done when: `pnpm check`, `pnpm test` pass; a type error appears if you add a variant to
`DomainEvent`, regenerate bindings, and forget the `invalidations` entry (try it, then
revert).

Commit: `feat(desktop): ipc client, query keys, event→invalidation bridge, mock ipc`

---

### M7 — Feature registry and app shell (≈1 d)

**Goal:** navigation, palette, dashboard widgets and settings sections are composed from
manifests; the shell never names a feature.

1. `lib/features/types.ts` — 05's interfaces, with these adjustments:
   - `NavItem.icon: IconName` where `IconName = keyof typeof icons` from
     `lib/shell/icons.ts` (a map of lucide components). Typos become type errors.
   - `FeatureManifest.flag?: string` — if set, the feature is hidden unless
     `settings.featureFlags[flag]` is true.
   - `FeatureManifest.hidden?: boolean` — routes exist but no nav entry (developer page).
   - `PaletteCommand.group?: string` for palette sections.
   - `CommandContext = { goto, queryClient, toast }` passed to `run`.
2. `lib/features/registry.ts` — split pure logic from discovery so it is testable:

   ```ts
   export function buildRegistry(manifests: FeatureManifest[], flags: Record<string, boolean>) {
     // returns { features, navItems, paletteCommands, dashboardWidgets, settingsSections, warnings }
   }
   const discovered = import.meta.glob<{ default: FeatureManifest }>("./*/manifest.ts", { eager: true });
   export const manifests = Object.values(discovered).map((m) => m.default);
   ```

   Rules: sort by `order ?? 100` then `id`; duplicate feature ids or palette command ids
   → `warnings` (and `throw` in `import.meta.env.DEV`); shortcut conflicts → warnings;
   flagged-off features contribute nothing. **Do not** copy 05's
   `.filter(f => f.id !== "settings")` — the settings feature is a normal feature.
   Every nav item also yields an automatic palette command "Go to <label>".
   Expose a reactive `registry` (a `$derived` over the settings query's flags) from
   `lib/features/registry.svelte.ts`.
3. `lib/utils/shortcuts.ts` — parse `"mod+shift+c"` (mod = ⌘ on macOS, Ctrl elsewhere)
   and two-key sequences `"g d"` (1 s timeout). One global `keydown` listener installed
   by `AppShell`; ignored when focus is in `input`, `textarea`, `select` or
   `[contenteditable]` (except `mod+k`). Built-in shell shortcuts: `mod+k` palette, `?`
   help, `Esc` closes overlays. `g` + letter navigation comes from each nav item's
   optional `shortcut` field (08 §Keyboard model).
4. `lib/stores/toasts.svelte.ts` — `toast.success|error|info(message, { action? })`
   wrapping svelte-sonner; `toast.ipcError(e)` renders `INTERNAL` with its `logId` and
   a "Copy details" action.
5. `lib/stores/theme.svelte.ts` — `$state` mode (`system|light|dark`); resolves
   `system` through a `matchMedia` listener; applies `.dark` on `<html>`; writes the
   localStorage cache; on `settings` query data, adopts the backend value (backend
   wins); `setTheme()` calls `settings_update` via a TanStack mutation (optimistic).
6. Shell components (`lib/shell/`), using shadcn primitives and semantic tokens:
   - `AppShell.svelte` — CSS grid: sidebar | (topbar / main). Hosts palette, toasts,
     shortcut listener, `ErrorBoundary` around `children`.
   - `Sidebar.svelte` — renders `registry.navItems`; active via `page.url.pathname`
     (`$app/state`); collapses to icons with tooltips below 1100 px (08 §Responsive).
   - `Topbar.svelte` — palette trigger showing the shortcut, theme dropdown.
   - `CommandPalette.svelte` — shadcn `Command` in a dialog; items from
     `registry.paletteCommands` filtered by `when()`; grouped; shows shortcuts; runs
     with `CommandContext`; restores focus on close.
   - `ErrorBoundary.svelte` — `<svelte:boundary>` with a `failed` snippet: message,
     "Try again" (`reset`), "Open data folder". Logs via `@tauri-apps/plugin-log`
     (wrap in `lib/ipc/` to keep the import rule).
   - `BootError.svelte` — full-screen state when `app_health.dbOk` is false: shows
     `bootError`, data dir, "Open data folder", "Retry" (reload).
   - `routes/+error.svelte` — not-found/route error inside the shell.
   - `+layout.svelte` gates on `app_health`: skeleton while loading, `BootError` if not
     ok, `AppShell` otherwise.
7. Tests:
   - `registry.test.ts`: ordering, duplicate ids → warning/throw, shortcut conflict,
     flag filtering, auto "Go to" commands.
   - `shortcuts.test.ts`: parsing, mod mapping per platform, input suppression,
     sequences.
   - `CommandPalette` component test (Testing Library): typing filters, Enter runs.
   - `tests/boundaries.test.ts`: scan `src/lib/features/<a>/**` for imports of
     `$lib/features/<b>/` (b ≠ a) and of `@tauri-apps/api/core`; fail with file names.
     Also assert `src/lib/shell/**` imports nothing from `$lib/features/*/` except
     `registry`/`types`.

Done when: unit/component tests pass; with zero feature manifests the shell renders an
empty sidebar without errors (temporarily verify by renaming the glob).

Commit: `feat(desktop): feature registry, app shell, palette, shortcuts, theme, toasts`

---

### M8 — Proof features and the round trip (≈0.5 d)

**Goal:** real features exercising read, write and event paths through the registry.

| Feature | Manifest | Route | Proves |
| --- | --- | --- | --- |
| `dashboard` | order 0, nav "Dashboard" (`home`, `g d`), dashboard widget `AppStatusWidget` | `/` renders `registry.dashboardWidgets` lazily | read round trip (`app_info`, `app_health`), lazy widget loading |
| `settings` | order 90, nav "Settings" (`settings`, `g s`), settings section `GeneralSettings` (theme) | `/settings` renders `registry.settingsSections` | write path → `SettingsChanged` → invalidation → theme applied |
| `help` | order 95, nav "Help" (`help-circle`), palette "Keyboard shortcuts" | `/help` lists every shortcut from the registry + shell built-ins | registry introspection; `?` opens it |
| `developer` | `hidden: true`, palette "Developer: features" with `when: () => import.meta.env.DEV` | `/dev/features` | discovery: table of frontend manifests vs `app_features`, highlighting ids present on one side only (frontend-only features like `help` are listed as such, not as errors) |

Each feature folder: `manifest.ts`, `components/…`, `README.md` (3–5 lines: purpose,
commands used, owner doc). Routes are thin: they import feature components and nothing
else.

`AppStatusWidget` shows version, API version, schema version, DB path (with "Open data
folder" → `app_open_data_dir`), and DB status. Settings section shows the theme radio
group (System/Light/Dark) and, in dev builds, the raw feature-flag map.

Playwright (`tests/e2e/shell.spec.ts`, runs `pnpm build` with `VITE_IPC_MOCK=1` then
`vite preview`):

- sidebar shows Dashboard, Settings, Help (from manifests);
- `Control+K` (and `Meta+K` on macOS) opens the palette; "Go to Settings" navigates;
- choosing Dark adds `dark` to `<html>` and survives reload;
- `g` then `h` navigates to Help; typing `g` inside an input does not.

Done when (manual, in `pnpm dev`, real backend):

- Dashboard shows schema version 1 and the dev DB path.
- Changing the theme persists across an app restart (backend value), and the app
  launches without a light/dark flash.
- `/dev/features` lists `app`, `settings` (backend) and `dashboard`, `settings`, `help`,
  `developer` (frontend).
- In devtools (debug build), `fetch("https://example.com")` is blocked by CSP and
  `window.__TAURI__` is undefined (no `withGlobalTauri`).
- `pnpm test:e2e` passes.

Commit: `feat(desktop): dashboard, settings, help and developer proof features`

---

### M9 — Feature generator (≈0.5 d)

**Goal:** `pnpm gen feature demo --nav --command` creates a compiling vertical slice; the
nav entry appears without touching the shell.

`tools/gen-feature.mjs` — Node ≥ 24, zero dependencies (`node:fs`, `node:path`,
`node:util` `parseArgs`). Usage:

```text
pnpm gen feature <name> [--nav] [--command] [--dry-run] [--force-none]
```

- `<name>`: `^[a-z][a-z0-9-]{1,30}$`. Derive `id` (kebab), `snake`, `camel`, `Pascal`,
  `Title`.
- Refuse if any target file exists or `commands/mod.rs` lacks the three `@gen:` markers.
  Never overwrite. `--dry-run` prints the plan only.

Creates:

| Path | Always / flag | Content |
| --- | --- | --- |
| `desktop/src/lib/features/<id>/manifest.ts` | always | id, title, order 50, palette "Go to <Title>" if `--nav`; nav item with icon `box` if `--nav` |
| `desktop/src/lib/features/<id>/README.md` | always | purpose TODO, owner, commands, checklist link to 05 §review checklist |
| `desktop/src/lib/features/<id>/components/<Pascal>Panel.svelte` | always | if `--command`: `createQuery(() => ({ queryKey: ["<id>"], queryFn: () => call(commands.<camel>List()) }))` with loading/empty/error states; else static placeholder |
| `desktop/src/routes/<id>/+page.svelte` | `--nav` | renders the panel |
| `desktop/src-tauri/src/commands/<snake>.rs` | `--command` | `descriptor()` + `<snake>_list() -> Result<Vec<String>, AppError>` returning `Ok(vec![])` with `TODO` |
| `crates/weasel-core/src/<snake>.rs` | `--command` | module doc + `TODO: domain types and service` |

Edits (marker insertion, one line each, idempotent): `commands/mod.rs` at
`// @gen:modules`, `// @gen:descriptors`, `// @gen:commands`; `weasel-core/src/lib.rs`
at `// @gen:modules` (add that marker in M2). Then it prints next steps: `pnpm bindings`,
add a migration *by hand* if needed (it does **not** generate one — see §9), add
`DomainEvent` variant + `invalidations` entry if the feature mutates data, write tests.

`tools/gen-feature.test.mjs` (`node --test`): copies the minimal set of target files
into a temp dir, runs the generator against it (`--root <dir>` option), asserts files and
marker insertions, and asserts a second run refuses.

Done when (in the real tree, then revert with `git checkout -- . && git clean -fd
desktop crates`):

```bash
pnpm gen feature demo --nav --command
pnpm bindings
cargo check --workspace
pnpm check && pnpm test
```

…all succeed, `/demo` appears in the sidebar and palette in `pnpm dev`, the panel shows
the empty state from a real `demo_list` round trip, and the backend/frontend
consistency test still passes. `AppShell.svelte`, `Sidebar.svelte` and `registry.ts`
are untouched (`git diff --stat` shows only generated files and marker lines).

Commit: `feat(tools): feature generator with marker-based registration`

---

### M10 — CI, docs, checklist (≈0.5 d)

**Goal:** a clean checkout is verified automatically; a newcomer can run the app from
the README.

`.github/workflows/ci.yml` (on `pull_request` and pushes to `main`; `concurrency` cancels
superseded runs):

| Job | Runner | Steps |
| --- | --- | --- |
| `frontend` | ubuntu-24.04 | checkout → `pnpm/action-setup` → `actions/setup-node` (node 24, pnpm cache) → `pnpm install --frozen-lockfile` → `pnpm check` → `pnpm lint` → `pnpm test` → `pnpm build` → `pnpm exec playwright install --with-deps chromium` → `pnpm test:e2e` |
| `rust` | ubuntu-24.04 | apt: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev build-essential` → toolchain from `rust-toolchain.toml` → `Swatinem/rust-cache` → `cargo fmt --all --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace` → `bash tools/check-layers.sh` |
| `contract` | ubuntu-24.04 | same apt + rust cache → pnpm install → `pnpm bindings` → `git diff --exit-code` |
| `generator` | ubuntu-24.04 | same setup → `pnpm gen feature demo --nav --command` → `pnpm bindings` → `cargo check --workspace` → `pnpm check` (tree is discarded with the runner) |
| `bundle` | matrix macos-14, windows-latest, ubuntu-22.04 | `pnpm tauri build --debug --no-bundle` — only on pushes to `main` and PRs labelled `build-all`, to save minutes |

Docs:

1. Root `README.md`: what Weasel is (one paragraph), prerequisites (Rust per
   `rust-toolchain.toml`, Node 24, pnpm via corepack, platform prerequisites from the
   Tauri v2 docs for macOS/Windows/Linux), commands (`pnpm install`, `pnpm dev`,
   `pnpm dev:web`, `pnpm verify`, `pnpm bindings`, `pnpm gen feature …`), where data and
   logs live (and `WEASEL_DATA_DIR`, and that debug builds use a `dev/` subfolder), the
   repo map, the pinned versions you actually used, link to `docs/rewrite/`.
2. Tick the **Foundation (phase 1)** items in `09-parity-checklist.md` with evidence
   links (CI run, test names). Do not tick anything without evidence.
3. Update `docs/rewrite/README.md` status line: "Phase 1 (architecture) implemented — see
   02-exec.md".
4. Fill §10 of this file (execution log) with what deviated during execution.

Done when: CI is green on the branch (requires the owner to push, see §3) — or, if
pushing is not allowed yet, `pnpm verify && pnpm bindings && git diff --exit-code &&
pnpm test:e2e` pass locally and you say CI is unverified.

Commit: `ci: frontend, rust, contract and generator jobs; docs for the new app`

---

## 8. Final acceptance (copy into the PR description)

Maps to 06 phase 1 acceptance and 09 §Foundation, adjusted for §1.

- [ ] `pnpm dev` opens a window on macOS; Windows/Linux commands documented.
- [ ] Fresh start creates `weasel.sqlite3` in the (dev) app-data dir with WAL and
      foreign keys on; a pending migration triggers a `VACUUM INTO` backup first (test).
- [ ] An unopenable DB shows the boot-error screen instead of crashing.
- [ ] `pnpm bindings && git diff --exit-code` passes; descriptor/registration consistency
      test passes.
- [ ] `pnpm check:layers` passes and fails when a forbidden dependency is added.
- [ ] Navigation, palette, dashboard widgets and settings sections come only from
      manifests; boundary test passes.
- [ ] `pnpm gen feature demo --nav --command` produces a compiling slice whose nav entry
      appears without shell edits.
- [ ] Theme persists in the backend, applies before first paint, and follows the system
      in `system` mode.
- [ ] Capabilities are `core:default` + `log:default` only; CSP blocks remote fetches.
- [ ] `pnpm verify` and `pnpm test:e2e` are green; CI green or explicitly unverified.
- [ ] No business logic in Svelte components; no `invoke` outside `lib/ipc`.

## 9. Deviations from the planning docs (decided here)

Record these in ADR 0006's "related decisions" section (M0) and reflect them when the
owning doc is executed.

| # | Planning doc says | This plan does | Why |
| --- | --- | --- | --- |
| 1 | Legacy import/discovery (03, 04, 07) | Dropped | Owner decision, §1 |
| 2 | `0001_initial.sql` holds the full schema (03) | `0001_app.sql` = `settings` + `app_meta`; domain tables arrive as `0002_domain.sql` in 03-exec | Keeps "never edit a migration once applied" true from the first dev DB onward; 03-exec owns its schema |
| 3 | Each `DomainEvent` variant carries `revision` (04) | `WeaselEvent { revision, event }` envelope | One place for ordering; variants stay purely about the domain |
| 4 | Channel literally named `weasel://event` (04) | Name derived by `tauri_specta::Event`; frontend uses the generated handle | No hand-written channel strings |
| 5 | Capabilities: dialog, clipboard, opener, updater (02) | `core:default`, `log:default` | Backend performs dialog/clipboard/open through ports; the webview needs none of them. Updater added in 07-exec |
| 6 | `FeatureDescriptor.migrations` (05) | Removed | Migrations are a single global ordered sequence |
| 7 | Generator scaffolds a migration (05) | It prints a reminder instead | An empty generated migration would get applied, then edited → checksum mismatch |
| 8 | `desktop/pnpm-lock.yaml` (02 tree) | Root `pnpm-lock.yaml` | pnpm workspace behaviour |
| 9 | DB path overridable via settings (02) | `WEASEL_DATA_DIR` env var now; config-file override in phase 6 | Settings live inside the DB being located |
| 10 | TanStack Query example uses object form (04) | Thunk form (`createQuery(() => …)`) | svelte-query v6 API |
| 11 | Registry filters out `settings` (05 sample) | No special case | The sample's filter had no stated reason; settings is a normal feature |
| 12 | Tauri adapters "in weasel-platform" (02) | Tauri-backed adapters live in `desktop/src-tauri/src/adapters.rs`; `weasel-platform` stays Tauri-free | Keeps every crate except the adapter free of Tauri, which the layer check enforces |

## 10. Execution log

Fill in during execution: actual versions pinned, anything that did not work as
described, and decisions taken on the fly (with the milestone). Keep entries short.

| Date | Milestone | Note |
| --- | --- | --- |
| | | |

## 11. Handoff to later exec plans

What exists after this plan and how the next plans plug in:

- **03-exec (data model):** add `migrations/0002_domain.sql` (03's DDL minus legacy
  bits, `source_kind` without `'legacy'`); repositories in `weasel-store` following
  `SqliteSettings`; services in `weasel-core` following `SettingsService`; tests on
  `TempDb`. Extend `app_health` with `contactCount`/`pendingImports` (optional fields).
- **04-exec (IPC):** one `commands/<feature>.rs` per feature with `descriptor()`;
  register via the `@gen:` markers (or the generator); add `DomainEvent` variants and
  `invalidations` entries; `campaigns_prepare_send` uses `Services.clipboard` before
  commit as in 02 §Request flow; `imports_pick_source` uses `Services.dialogs`.
- **05-exec (extensibility):** row/bulk action descriptors, feature-flag route guard,
  generator refinements. The registry, manifest types and generator are already here.
- **08-exec (UI):** screens drop into the shell via manifests; tokens and primitives are
  in place.
- **07-exec (release):** updater plugin + capability, signing, real icons, bundle id
  confirmation.
