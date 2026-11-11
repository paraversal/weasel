# ADR 0001 — Tauri 2 + SvelteKit

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

The app is a local, single-user email-campaign manager currently built with Textual. It
needs a real GUI, native file pickers, clipboard access, local persistence, and
installers for non-developer users. The rewrite must be cheap to extend.

## Decision

Use Tauri 2 as the desktop shell and SvelteKit (Svelte 5, TypeScript) as the UI, served
as a static SPA from the Tauri bundle.

## Consequences

- Small installers and a native window without a bundled browser engine or Node runtime.
- Rust backend gives direct access to SQLite, files, clipboard and OS dialogs.
- SvelteKit provides file-based routing, forms and a mature tooling ecosystem.
- The team needs Rust and TypeScript skills; CI must build three platforms.
- Tauri's security model (capabilities/CSP) must be configured deliberately.

## Alternatives considered

- **Electron:** larger bundles and an additional Node runtime; more attack surface for a
  local tool.
- **Pure web app:** cannot reliably access local files/clipboard and would require a
  server or browser storage.
- **Keep Textual / add a web UI:** retains the TUI's limitations and splits the UI.
