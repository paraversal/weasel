# ADR 0005 — SvelteKit static SPA, no SSR at runtime

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

Tauri serves bundled frontend assets from disk. The app has no server, no accounts and no
SEO requirements. SvelteKit supports both server-rendered and static output.

## Decision

Build with `@sveltejs/adapter-static` and SPA fallback (`index.html`), with `ssr = false`
and no prerendered data fetching. All data comes from Tauri commands at runtime.

## Consequences

- The bundle is static and works identically offline.
- There is no Node server to package or supervise.
- Deep links work through the SPA fallback; routes cannot rely on server load functions.
- Initial data loading happens client-side after the shell mounts; skeleton states are
  required.
- If a future web client needs SSR, it can reuse `weasel-core` through a separate server
  adapter rather than changing the desktop shell.

## Alternatives considered

- **SSR with a bundled Node server:** unnecessary complexity for a local desktop app.
- **Prerender every route:** routes depend on runtime database state, so prerendering
  provides little value and complicates dynamic pages.
