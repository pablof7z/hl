//! Kernel-owned clock abstraction. Production code uses `SystemClock`;
//! tests and replayable flows inject deterministic time via `ManualClock`
//! without ever touching wall-clock reads (D9).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now_unix_seconds(&self) -> u64;

    fn now_unix_nanos(&self) -> u128 {
        u128::from(self.now_unix_seconds()) * 1_000_000_000
    }
}

/// Production clock — reads wall-clock UNIX seconds.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_seconds(&self) -> u64 {
        UNIX_EPOCH.elapsed().map(|d| d.as_secs()).unwrap_or(0)
    }

    fn now_unix_nanos(&self) -> u128 {
        UNIX_EPOCH.elapsed().map(|d| d.as_nanos()).unwrap_or(0)
    }
}

/// Deterministic clock for tests. Advance via `advance(secs)` or `set(secs)`.
/// All methods are thread-safe (atomic u64).
#[derive(Debug)]
pub struct ManualClock {
    secs: AtomicU64,
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ManualClock {
    pub fn new(initial: u64) -> Self {
        Self {
            secs: AtomicU64::new(initial),
        }
    }

    /// Advance the clock by `secs` seconds. Returns the new time.
    pub fn advance(&self, secs: u64) -> u64 {
        self.secs.fetch_add(secs, Ordering::SeqCst) + secs
    }

    /// Set the clock to an absolute value.
    pub fn set(&self, secs: u64) {
        self.secs.store(secs, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_unix_seconds(&self) -> u64 {
        self.secs.load(Ordering::SeqCst)
    }
}
