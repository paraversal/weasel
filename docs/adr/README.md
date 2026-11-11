# Architecture Decision Records

Short, immutable records of decisions that would be expensive to reverse. New decisions
get the next number; superseding a decision adds a new ADR that references the old one
instead of editing history.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-tauri-sveltekit.md) | Tauri 2 + SvelteKit for the desktop app | Accepted |
| [0002](0002-rust-backend-sqlite.md) | Rust backend owning SQLite; no Python sidecar | Accepted |
| [0003](0003-contact-model.md) | `Contact` domain with soft removal and immutable send history | Accepted |
| [0004](0004-feature-modules-and-typed-ipc.md) | Feature modules composed by manifests + typed IPC | Accepted |
| [0005](0005-static-spa.md) | SvelteKit static SPA, no SSR at runtime | Accepted |

Template: [0000-template.md](0000-template.md)
