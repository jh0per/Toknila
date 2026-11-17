//! The line the hand-in checks of this machine stand in (the rule is
//! [`toknila_app::checks::CheckLine`]) and where its load number comes from.

use std::sync::Arc;

pub use toknila_app::checks::CheckLine;
use toknila_app::checks::{Load, LoadReader};

/// The machine's one-minute load average and core count, read through `sysinfo`. Windows has no
/// load average, so there the line is gated by count alone.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLoad;

impl LoadReader for SystemLoad {
    fn read(&self) -> Option<Load> {
        if cfg!(windows) {
            return None;
        }
        let cores = std::thread::available_parallelism().ok()?.get();
        let average = sysinfo::System::load_average().ok()?;
        Some(Load {
            one_minute: average.one,
            cores: u32::try_from(cores).unwrap_or(u32::MAX),
        })
    }
}

/// A line that reads this machine's load.
pub fn machine_line() -> Arc<CheckLine> {
    Arc::new(CheckLine::new(Arc::new(SystemLoad)))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_machine_reads_a_load_and_its_cores() {
        let load = SystemLoad
            .read()
            .expect("a unix machine has a load average");
        assert!(load.cores >= 1 && load.one_minute >= 0.0, "{load:?}");
    }
}
