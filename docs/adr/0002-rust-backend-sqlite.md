# ADR 0002 — Rust backend owning SQLite; no Python sidecar

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

The current backend is Python (Pandas, openpyxl, pyperclip, SQLite). The rewrite needs
spreadsheet import, database access, clipboard writes and native dialogs. Keeping Python
would preserve Pandas behaviour but add process supervision, packaging and version-skew
complexity to a desktop app.

## Decision

Implement the backend in Rust inside Tauri. Use `sqlx` with SQLite, `calamine` for
`.xlsx`/`.xls`/`.ods`, `csv` for CSV, and small ports for clipboard/dialogs. Do not ship
a Python sidecar. Keep the old Python app under `legacy/` as a reference until parity.

## Consequences

- One binary, one toolchain for the backend, no Python runtime in installers.
- Spreadsheet handling must be validated against real files; `calamine` does not expose
  every Pandas convenience (merged headers, skipfooter), so the import wizard makes
  mapping explicit.
- The domain is unit-testable without Python or a window.
- Any future analytics that truly needs Pandas can be added as an optional external tool
  without putting Python on the critical path.

## Alternatives considered

- **Python sidecar over JSON-RPC:** keeps Pandas but reintroduces packaging and lifecycle
  problems, and splits the data model across two runtimes.
- **PyO3 embedding:** still ships Python and complicates cross-platform builds.
