//! SQLite adapter: pool setup, migrations, settings repository and instance metadata.
//!
//! Transaction rule: every write path either takes an existing `&mut Transaction` or
//! opens one. There are no autocommit multi-statement writes anywhere in this crate.

mod error;
mod meta;
mod pool;
mod settings;

pub use error::StoreError;
pub use meta::{get_meta, set_meta};
pub use pool::{OpenOptions, OpenReport, Store, open, open_with};
pub use settings::SqliteSettings;
