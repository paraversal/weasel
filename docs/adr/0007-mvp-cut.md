# ADR 0007 — MVP cut: plain routes, no frontend registry, no IPC mock

- **Status:** Accepted
- **Date:** 2026-11-11
- **Partly supersedes:** [ADR 0004](0004-feature-modules-and-typed-ipc.md) (the frontend
  manifest/registry and the generator; typed IPC stays)

## Context

After the IPC surface landed (04-exec) the rewrite had about 20k lines of code and no
screen that did the job of the ~1k-line TUI. Much of the cost was machinery for growth
that one developer and about six screens do not need: a manifest registry that composed
the shell, a generator that scaffolded features, and an in-memory TypeScript copy of the
backend (`mock-domain.ts`) that every command change had to mirror. Playwright could only
run against that mock, because the Tauri webview cannot be scripted.

## Decision

- Screens are SvelteKit routes. `src/lib/shell/nav.ts` lists the sidebar and palette
  entries by hand.
- Delete the feature registry, manifests, the developer feature page, `tools/gen-feature*`
  and the frontend boundary test. The eslint ban on importing Tauri outside `lib/ipc`
  stays.
- Delete the IPC mock and the Playwright suite. Develop screens against the real backend
  with `pnpm dev:demo` (seeded database). UI behaviour is verified by the manual smoke
  run in [08-exec §7](../rewrite/08-exec.md) until tauri-driver e2e is added.
- The Rust side is unchanged: typed commands, generated bindings, the CI contract check
  and the crate layering all stay. Backend `FeatureDescriptor`s and `app_features` stay
  but have no frontend consumer.

## Consequences

- A new screen is a route, a line in `nav.ts`, and any `queries.ts` factories it needs.
- `pnpm dev:web` (browser-only mode) no longer exists.
- There is no automated UI test until tauri-driver e2e lands (08-exec §9 backlog).
- The remaining rewrite plan is 08-exec, an MVP cut of 06 phases 3–4; phases 5–7 become
  a backlog.
