# 05 — Extensibility

> **Superseded on the frontend (2026-11-11):** see
> [ADR 0007](../adr/0007-mvp-cut.md). Screens are plain routes listed in
> `desktop/src/lib/shell/nav.ts`; the manifest registry, generator and IPC mock described
> below were removed. The backend sections still describe the code.

The rewrite is expected to grow beyond the current TUI. Extensibility here means four
concrete properties:

1. **Additive features:** a new screen or capability is added as a module, not by editing
   unrelated files.
2. **Stable boundaries:** UI talks only through typed commands, the domain ignores the
   framework, and adapters are swappable.
3. **Additive data:** new features add tables/columns through migrations without
   rewriting existing ones.
4. **Discoverable composition:** navigation, command palette, dashboard and settings are
   assembled from feature manifests, not hard-coded lists.

## Feature modules

A feature is a vertical slice with the same identity on both sides of the IPC boundary.

```text
Backend                                      Frontend
────────────────────────────────────────     ────────────────────────────────────────
crates/weasel-core/src/favorites.rs          desktop/src/lib/features/favorites/
  domain types + service                       manifest.ts
crates/weasel-store/src/favorites.rs           components/FavoriteButton.svelte
  repository + SQL                            components/FavoritesPanel.svelte
desktop/src-tauri/src/commands/favorites.rs   routes/favorites/+page.svelte
  IPC handlers + DTO mapping
migrations/0007_favorites.sql
```

A feature may depend on `weasel-core` and shared UI primitives, never on another
feature's internals. Cross-feature needs go through a domain service or a shared module.

### Frontend manifest

```ts
// desktop/src/lib/features/types.ts
import type { Component } from "svelte";

export interface FeatureManifest {
  id: string;
  title: string;
  description?: string;
  order?: number;
  nav?: NavItem[];
  commands?: PaletteCommand[];
  dashboard?: DashboardWidget[];
  settings?: SettingsSection[];
}

export interface NavItem {
  label: string;
  href: string;
  icon: string;                 // key into the icon map
  order?: number;
  badge?: () => number | undefined;
}

export interface PaletteCommand {
  id: string;                   // "campaigns.create"
  title: string;
  keywords?: string[];
  shortcut?: string;            // "mod+shift+c"
  when?: () => boolean;         // context predicate
  run: (ctx: CommandContext) => void | Promise<void>;
}

export interface DashboardWidget {
  id: string;
  order?: number;
  component: () => Promise<{ default: Component }>;
}

export interface SettingsSection {
  id: string;
  title: string;
  order?: number;
  component: () => Promise<{ default: Component }>;
}
```

```ts
// desktop/src/lib/features/campaigns/manifest.ts
import type { FeatureManifest } from "$lib/features/types";
import { goto } from "$app/navigation";
import { openCampaignDialog } from "./state/dialogs";

export default {
  id: "campaigns",
  title: "Campaigns",
  order: 20,
  nav: [{ label: "Campaigns", href: "/campaigns", icon: "mail", order: 20 }],
  commands: [
    {
      id: "campaigns.create",
      title: "Create campaign",
      keywords: ["new", "email", "campaign"],
      shortcut: "mod+shift+c",
      run: () => openCampaignDialog(),
    },
    {
      id: "campaigns.go",
      title: "Go to campaigns",
      run: () => goto("/campaigns"),
    },
  ],
  dashboard: [
    {
      id: "campaigns.recent",
      order: 20,
      component: () => import("./components/RecentCampaignsWidget.svelte"),
    },
  ],
} satisfies FeatureManifest;
```

```ts
// desktop/src/lib/features/registry.ts
import type { FeatureManifest } from "./types";

const discovered = import.meta.glob<{ default: FeatureManifest }>("./*/manifest.ts", {
  eager: true,
});

export const features = Object.values(discovered)
  .map((module) => module.default)
  .filter((feature) => feature.id !== "settings")
  .sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

export const navItems = features
  .flatMap((feature) => feature.nav ?? [])
  .sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

export const paletteCommands = features.flatMap((feature) => feature.commands ?? []);

export const dashboardWidgets = features
  .flatMap((feature) => feature.dashboard ?? [])
  .sort((a, b) => (a.order ?? 100) - (b.order ?? 100));
```

The shell renders `navItems`, the command palette renders `paletteCommands`, the dashboard
loads `dashboardWidgets`, and the settings screen composes `settings` sections. Adding a
feature does not touch the shell.

SvelteKit still owns routing through `src/routes/`. A feature's route file is a thin
page that composes feature components and wires query parameters. The generator creates
that file; the manifest provides the navigation metadata SvelteKit cannot infer.

### Backend feature descriptor

Rust command registration must remain static for Tauri's compile-time safety. Features
therefore expose a descriptor for discovery/diagnostics and a registration function for
handlers:

```rust
// crates/weasel-core/src/feature.rs
pub struct FeatureDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    pub commands: &'static [&'static str],
    pub events: &'static [&'static str],
    pub migrations: &'static [&'static str],
}

pub trait FeatureModule {
    fn descriptor(&self) -> FeatureDescriptor;
}
```

```rust
// desktop/src-tauri/src/commands/mod.rs
pub mod app;
pub mod campaigns;
pub mod contacts;
pub mod favorites;     // new module: one line here ...
pub mod imports;
pub mod sends;
pub mod settings;

pub fn feature_descriptors() -> Vec<FeatureDescriptor> {
    vec![
        contacts::descriptor(),
        campaigns::descriptor(),
        sends::descriptor(),
        imports::descriptor(),
        favorites::descriptor(),   // ... and one line here
    ]
}

pub fn register() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        app::app_info,
        contacts::contacts_list,
        contacts::contacts_get,
        // ...
        favorites::favorites_list,     // and its handlers
        favorites::favorites_toggle,
    ])
}
```

`feature_descriptors()` powers `app_health`/diagnostics, the generated feature index in
the docs, and a test that every advertised command name exists in the registered list.
The two-line registration is deliberate: it keeps the IPC surface visible and reviewable
instead of hiding it behind runtime string dispatch.

## Adding a feature, end to end

Example: "favourites" for contacts.

1. **Migration** — `migrations/0007_favorites.sql` creates `favorites(contact_id,
   created_at)` with a foreign key. Never edit old migrations.
2. **Domain** — `weasel-core/src/favorites.rs` defines `Favorite`, `FavoriteService`, and
   repository port methods. Unit tests use fakes from `weasel-testkit`.
3. **Store** — `weasel-store/src/favorites.rs` implements the port with `sqlx`. Tests run
   against a temporary SQLite file with all migrations.
4. **Commands** — `desktop/src-tauri/src/commands/favorites.rs` exposes
   `favorites_list` / `favorites_toggle`, maps rows to DTOs, emits
   `FavoritesChanged`.
5. **Contract** — add the handlers to `commands/mod.rs`; run `pnpm bindings`; the
   generated TypeScript now includes the new commands.
6. **Event** — add `FavoritesChanged { revision }` to `DomainEvent` and its invalidation
   entry. Unknown events are ignored by older frontends.
7. **Frontend** — `features/favorites/manifest.ts`, components, and
   `routes/favorites/+page.svelte`. The nav entry appears automatically.
8. **Tests** — core unit tests, store integration test, frontend component test, and one
   command wiring test.
9. **Docs** — the generator writes a short `README.md` in the feature folder.

`pnpm gen feature favorites --nav --command` scaffolds steps 1, 2, 4, 5, 7 and 9 with
TODOs and prints the remaining registration lines. The generated code compiles
immediately (empty list/toggle stubs) so the feature can be built incrementally.

## Backend extension points

Defined as traits in `weasel-core::ports` (or the relevant core module):

| Port | Purpose | v1 adapter | Likely future adapters |
| --- | --- | --- | --- |
| `ContactRepository` | Contact CRUD/list/stats | SQLite | remote sync, in-memory tests |
| `CampaignRepository` | Campaigns, audience queries | SQLite | — |
| `SendRepository` | Batches and immutable records | SQLite | — |
| `ImportRepository` | Draft/applied import batches | SQLite | — |
| `ContactImporter` | Parse a source into normalized rows | xlsx, csv | Google Sheets, clipboard paste, API |
| `IdentityStrategy` | Decide whether a row matches a contact | email + identity hash | matriculation number, external id |
| `Clipboard` | Write recipient list | Tauri/arboard | fake in tests |
| `FileDialog` | Pick import file | Tauri dialog | fake in tests |
| `Clock` | Timestamps | system clock | fixed clock in tests |
| `EmailProvider` | Actually deliver mail (future) | none in v1 | SMTP, Resend, Mailgun |
| `Exporter` | Export contacts/history (future) | none in v1 | CSV, xlsx |

`ContactImporter` is the main format-extension point:

```rust
pub trait ContactImporter: Send + Sync {
    fn kind(&self) -> ImportKind;
    fn can_parse(&self, path: &Path) -> bool;
    fn read(&self, path: &Path, mapping: &ColumnMapping)
        -> Result<Vec<RawContactRow>, ImportError>;
}
```

A new file format is one implementation plus one line in the importer registry; the
preview/reconcile/apply pipeline is unchanged. Similarly, a new identity rule is one
`IdentityStrategy` implementation plus a settings option.

## Frontend extension points

- **Navigation:** `manifest.nav`.
- **Command palette and shortcuts:** `manifest.commands`; the shell binds shortcuts and
  handles conflicts.
- **Dashboard cards:** lazy-loaded `manifest.dashboard` components.
- **Settings sections:** `manifest.settings`; each section owns its form and validation.
- **Row actions and bulk actions:** exported by feature action modules and composed into
  the shared contact table through typed action descriptors, not conditionals in the
  table component.
- **Toast/notification kinds:** shared `toasts` store; features do not create their own
  notification systems.
- **Dialogs:** shared accessible dialog primitive; features register opener functions.

## Feature flags and gradual rollout

```ts
export interface FeatureFlag {
  id: string;
  title: string;
  description: string;
  default: boolean;
  requiresRestart?: boolean;
}
```

Flags live in backend settings and are exposed through `settings_get_all`. The shell
filters `navItems`/`paletteCommands` and the route guard hides disabled features. Backend
commands for a disabled feature return `INVALID_STATE` with `{ feature: "favorites" }`.
This supports shipping half-built features without exposing them.

## Multi-window readiness

Tauri can host more than one window (for example a detached campaign composer). Because
all state lives in the backend and changes are broadcast as events, new windows do not
need to share frontend stores. Query invalidation on `weasel://event` keeps every window
consistent. Avoid window-local mutable business state by design.

## What is deliberately not built yet

- **Runtime plugin loading / third-party extensions.** Dynamic command dispatch,
  capability grants and sandboxing are a separate design problem. If it is needed, add a
  WASM host with a manifest that declares commands and permissions; the typed IPC and
  domain ports remain the stable inner API.
- **A generic "execute arbitrary SQL/report" feature.** It would break the domain
  boundary and the safety model.
- **Cross-feature imports.** If two features need the same code, move it into
  `weasel-core`, a shared store module, or `lib/components`; do not import one feature
  from another.

## Extensibility review checklist

Every feature PR should answer yes to:

- [ ] Is the feature a vertical slice with its own module and manifest?
- [ ] Are all writes behind one command with validation and a transaction?
- [ ] Are new types exported through `specta` and the bindings regenerated?
- [ ] Are new tables/columns added through a new migration?
- [ ] Are events emitted only after commit, with an invalidation mapping?
- [ ] Are there unit, store and component tests for the new behaviour?
- [ ] Does the feature work with the command palette and keyboard only?
- [ ] Could this feature be disabled by a flag without breaking other screens?
- [ ] Are existing migrations, commands and events left backwards compatible?
