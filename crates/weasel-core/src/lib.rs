//! Pure domain crate: errors, ports, feature descriptors and application services.
//!
//! Rules:
//! - No framework, database or OS dependencies. `sqlx`, `tauri`, `specta` and the
//!   other crates in the workspace are forbidden here and enforced by
//!   `tools/check-layers.sh`.
//! - Services own behaviour; adapters own IO. Every fallible operation returns
//!   [`CoreResult`].
//! - New feature modules are added below the `@gen:modules` marker by
//!   `pnpm gen feature`.

pub mod audit;
pub mod campaign;
pub mod contact;
pub mod email;
pub mod enums;
pub mod error;
pub mod feature;
pub mod identity;
pub mod import;
pub mod limits;
pub mod ports;
pub mod query;
pub mod send;
pub mod settings;
// @gen:modules

pub use error::{CoreError, CoreResult};
pub use feature::FeatureDescriptor;
