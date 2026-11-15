<p align="center">
  <img src="desktop/src-tauri/icons/128x128@2x.png" width="128" height="128" alt="Weasel logo: a weasel mascot">
</p>

<h1 align="center">Weasel</h1>

<p align="center">
  A small, local-first desktop app for running email campaigns to a changing list of people.
</p>

<p align="center">
  <a href="https://github.com/paraversal/weasel/actions/workflows/ci.yml"><img src="https://github.com/paraversal/weasel/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue" alt="License: GPL-3.0-only"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Platforms: macOS, Windows, Linux">
</p>

---

Weasel is for the person who gets a fresh roster spreadsheet every term and has to make
sure everyone on it hears about the right things exactly once. Import the roster, write a
campaign with merge tags, copy the next batch of addresses to the clipboard, paste them
into your mail client, and Weasel remembers who already got what.

Everything stays on your machine in a single SQLite file. There is no server, no account
and no SMTP: Weasel never sends mail itself.

## Features

- **Roster imports** from CSV, XLSX, XLS and ODS. Header rows are detected, columns are
  mapped in a wizard, and you preview every add, update and removal before it is applied.
  Drag a spreadsheet anywhere onto the window to start.
- **Conflict review** when a re-import disagrees with what you already have, decided per
  row.
- **Contacts that are never lost.** People move `active → removed → archived`; removing
  someone frees their address for reuse but keeps their history.
- **Campaigns with merge tags** and sent / unsent / all audience views.
- **Clipboard sends.** *Prepare send* copies the addresses of everyone who has not received
  a campaign yet and records an immutable snapshot of who got what, so later edits never
  rewrite history. Any past batch can be copied again.
- **Invalid-address handling** with wildcard rules, manual flags and bounce tracking.
- **Activity log** for every change, backups before every schema migration, and a
  dashboard that shows where your data lives.
- Keyboard-first: a `⌘K` command palette and `g`-prefixed shortcuts, plus light and dark
  themes.
- Call the people whatever you like: the word for them (default *freshies*) is a setting.

## Status

Weasel works end to end, but it is early: there are no signed installers or
auto-updates yet, so you build it from source. See
[`HANDOFF.md`](HANDOFF.md) for known gaps and what comes next.

## Getting started

### Prerequisites

| Tool | Version |
| --- | --- |
| Rust | pinned by [`rust-toolchain.toml`](rust-toolchain.toml), with `rustfmt` and `clippy` |
| Node.js | 24 (see [`.node-version`](.node-version)) |
| pnpm | through corepack: `corepack enable` |
| System libraries | the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS |

### Run it

```bash
pnpm install
```

```bash
pnpm dev:demo
```

`dev:demo` seeds a throwaway database in `.dev-data/` with made-up people and opens the
app against it, which is the best way to look around. `pnpm dev` opens the app against
your real data directory instead.

### Build it

```bash
pnpm tauri build
```

## Where your data lives

Weasel keeps everything in the platform's app-data directory (on macOS,
`~/Library/Application Support/dev.weasel.app`), or in `WEASEL_DATA_DIR` when that is set.
Debug builds use a `dev/` subfolder so they never touch a real database.

```text
<data dir>/weasel.sqlite3   # the database (WAL, foreign keys on)
<data dir>/backups/         # VACUUM INTO snapshots taken before migrations
<data dir>/logs/weasel.log  # rolling log; email addresses are redacted
```

## How it is built

A Rust workspace behind a [Tauri 2](https://tauri.app) shell and a
[SvelteKit](https://svelte.dev) frontend (Svelte 5, Tailwind CSS 4, shadcn-svelte).
Rust owns all data logic; the UI only renders. The IPC surface is generated into
TypeScript by tauri-specta, so a backend change breaks the frontend build instead of
failing at runtime.

```text
crates/weasel-core       domain types, validation, queries, audit (no Tauri, no SQLite)
crates/weasel-store      SQLite, migrations, transactions, backups
crates/weasel-import     file readers, header detection, column mapping, reconciliation
crates/weasel-platform   OS effects: data paths, clock, clipboard, dialogs
crates/weasel-testkit    temp databases, fakes, demo seeding
desktop/src-tauri        Tauri commands, events and DTOs
desktop/src              SvelteKit UI
migrations/              forward-only SQL, embedded at compile time
```

`tools/check-layers.sh` keeps those layers honest in CI.

## Development

| Command | What it does |
| --- | --- |
| `pnpm verify` | everything CI runs: svelte-check, lint, vitest, `cargo fmt`, clippy, `cargo test`, layer check |
| `pnpm dev:demo` | app against a freshly seeded demo database |
| `pnpm test` | frontend unit tests |
| `cargo test --workspace` | Rust tests |
| `pnpm bindings` | regenerate `desktop/src/lib/ipc/bindings.ts` after changing a command |
| `pnpm seed:demo -- --contacts 5000` | seed a larger demo database |

The rules for working in this repository (transactions, auditing, migrations, frontend
conventions) are in [`AGENTS.md`](AGENTS.md). Run `pnpm verify` before sending a change.

## License

[GPL-3.0-only](LICENSE).
