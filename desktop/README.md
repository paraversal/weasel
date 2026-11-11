# Weasel desktop

SvelteKit (static SPA) inside a Tauri 2 window. Run everything from the repository root.

## Working on screens

```sh
pnpm dev:demo
```

This seeds a demo database into `.dev-data/dev/` (gitignored, replaced on every run) and
opens the app on it via `WEASEL_DATA_DIR`. Debug builds always append `dev/` to the data
directory, which is why the seed goes there. `pnpm dev` opens the app on your normal
development database instead.

There is no browser-only mode: screens talk to the real Rust backend.

## Layout

- `src/routes/<screen>/` — one route per screen; `src/lib/shell/nav.ts` lists them.
- `src/lib/screens/<screen>/` — components and state for a screen.
- `src/lib/components/ui/` — shadcn-svelte primitives (`pnpm exec shadcn-svelte add …`).
- `src/lib/components/app/` — shared app components (pager, search, dialogs).
- `src/lib/ipc/` — generated bindings, `call()`, query keys and event invalidation. The
  only place allowed to import `@tauri-apps/api`.

## Checks

`pnpm verify` runs type checks, lint, vitest and the Rust suite. After changing a Rust
command, run `pnpm bindings` and commit the regenerated `bindings.ts`.
