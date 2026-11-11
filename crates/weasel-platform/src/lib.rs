//! OS-facing adapters: data directory resolution, path validation, system clock.
//!
//! Nothing here knows about Tauri. `app_data_dir` is passed in by the composition
//! root so this crate stays testable without a running app.

pub mod fs;
pub mod paths;

pub use paths::DataPaths;

use weasel_core::ports::Clock;

/// Milliseconds since the Unix epoch from the OS clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}
