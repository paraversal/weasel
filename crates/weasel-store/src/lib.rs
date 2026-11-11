//! SQLite adapter: pool setup, migrations, settings repository and instance metadata.
//!
//! Transaction rule: every write path either takes an existing `&mut Transaction` or
//! opens one. There are no autocommit multi-statement writes anywhere in this crate.

mod codec;
mod error;
mod meta;
mod pool;
pub mod restore;
mod settings;
mod tx;

pub mod audit;
pub mod contacts;

pub use audit::SqliteAudit;
pub use contacts::SqliteContacts;
pub use error::StoreError;
pub use meta::{get_meta, set_meta};
pub use pool::{OpenOptions, OpenReport, Store, open, open_with};
pub use restore::restore_backup;
pub use settings::SqliteSettings;
