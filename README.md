# weasel

Email campaign management for a small team: contacts, campaigns, merge-tag
personalisation, clipboard send batches and send history, in a native desktop window.

**Rewrite in progress.** The Textual TUI lives under [`legacy/python-tui/`](legacy/python-tui/)
as a behavioural reference only; its database is not imported ([ADR 0006](docs/adr/0006-clean-rewrite-no-legacy-data.md)).
The new application is a Rust workspace behind a Tauri 2 + SvelteKit frontend. The plan and
status live in [`docs/rewrite/`](docs/rewrite/README.md), and the milestone log in
[`docs/rewrite/02-exec.md`](docs/rewrite/02-exec.md).

## Prerequisites

| Tool | Version |
| --- | --- |
| Rust | pinned by [`rust-toolchain.toml`](rust-toolchain.toml) (1.99.1, with `rustfmt` and `clippy`) |
| Node.js | 24 (see [`.node-version`](.node-version)) |
| pnpm | 12.14.0 through corepack (`corepack enable`) |
| Platform libs | [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for macOS (Xcode command line tools), Windows (WebView2, MSVC build tools) and Linux (webkit2gtk-4.1, libappindicator3, librsvg2, patchelf) |

The Python app is not needed to build or run the rewrite.

## Commands

```bash
pnpm install          # workspace install (desktop + tools)
pnpm dev              # Tauri window against Vite on :1420
pnpm dev:web          # browser-only UI with the mock IPC backend
pnpm check            # svelte-check
pnpm lint             # prettier + eslint
pnpm test             # vitest (unit + component) and the tool tests
pnpm test:e2e         # Playwright against a mock-mode preview build
pnpm verify           # check + lint + tests + fmt + clippy + workspace tests + layers
pnpm bindings         # regenerate desktop/src/lib/ipc/bindings.ts (commit the result)
pnpm gen feature foo [--nav] [--command] [--dry-run]
```

`pnpm gen feature` is the recipe from [`docs/rewrite/05-extensibility.md`](docs/rewrite/05-extensibility.md):
it creates the manifest, component, route and (optionally) the Rust command, then registers
everything through the `@gen:` markers. It never overwrites and never generates a migration.

## Data and logs

The app resolves its data directory through Tauri's app-data path, or `WEASEL_DATA_DIR` when
set. Debug builds and `pnpm dev` use a `dev/` subfolder so they never touch a real database.

```text
<data dir>/[dev/]weasel.sqlite3   # schema + settings, WAL, foreign keys on
<data dir>/[dev/]backups/         # VACUUM INTO snapshots taken before migrations
<data dir>/[dev/]logs/weasel.log  # rolling log (5 files, 5 MB each)
```

The dashboard shows the resolved paths and opens the folder. A database that cannot be
opened yields the boot-error screen with the same actions instead of a crash.

## Repository map

```text
crates/weasel-core       # domain types, ports and services (no Tauri, no SQLite)
crates/weasel-store      # SQLite pool, migrations, repositories
crates/weasel-import     # file parsing and reconciliation (phase 2)
crates/weasel-platform   # data paths, clock, filesystem validation (Tauri-free)
crates/weasel-testkit    # TempDb, fixed clock, fake clipboard/dialog
desktop/                 # SvelteKit SPA, Tauri shell, feature modules
desktop/src/lib/features # one folder per feature; manifests drive nav, palette, widgets
desktop/src-tauri        # Tauri adapter crate, commands, bindings export
migrations/              # ordered SQL migrations, global sequence
tools/                   # layer check and feature generator (zero deps)
legacy/python-tui/       # the previous Textual app, reference only
docs/rewrite/            # architecture, data model, IPC, extensibility, plan
```

## Pinned versions

Rust 1.99.1 · Node 24.1.0 · pnpm 12.14.0 · Tauri 2.12.5 · tauri-specta 2.0.0-rc.28 ·
SvelteKit 3.2.0 · Svelte 5.57.3 · Vite 8.3.5 · Tailwind CSS 4.3 · SQLx 0.9 · Vitest 5.1 ·
Playwright 1.65.
