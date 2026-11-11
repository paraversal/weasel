# ADR 0004 — Feature modules composed by manifests + typed IPC

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

The TUI grew as global app state watched by multiple views, with screens importing the
database directly. The rewrite is expected to add features beyond current parity, so the
cost of adding a feature must stay low and the boundaries must stay visible.

## Decision

Organize the app as vertical feature modules:
- The Rust side uses `weasel-core` services/repositories plus one command module per
  feature, registered explicitly in `commands/mod.rs`.
- The Svelte side uses `src/lib/features/<name>/manifest.ts` to declare navigation,
  command-palette actions, dashboard widgets and settings sections. A registry composes
  manifests; routes compose feature components.
- All UI/backend traffic goes through Rust-defined commands exported to TypeScript with
  `specta`/`tauri-specta`; generated bindings are committed and CI-checked.
- New tables/columns arrive via append-only migrations.
- A generator scaffolds both sides of a new feature.

## Consequences

- Adding a feature is a localized change plus a few explicit registration lines.
- The full IPC surface stays reviewable; there is no hidden runtime string dispatch.
- Generated types catch drift at build time.
- Contributors must follow the layering rule and regenerate bindings; the checklist in
  `docs/rewrite/05-extensibility.md` is part of review.
- Runtime third-party plugins are out of scope until a sandbox/capability design exists.

## Alternatives considered

- **Folders by technical type (components/, services/, models/):** scatters a feature
  across the tree and encourages cross-feature coupling.
- **Runtime command bus / dynamic plugins now:** more flexible, but adds security,
  versioning and debugging costs before there is a concrete need.
- **Hand-written frontend invoke strings:** fast initially, but silently breaks when the
  backend changes.
