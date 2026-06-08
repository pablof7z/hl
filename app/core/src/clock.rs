//! Kernel-owned clock abstraction. Production code uses `SystemClock`;
//! tests and replayable flows can inject deterministic time without moving
//! wall-clock reads into feature logic.

use std::time::UNIX_EPOCH;

use nostr_sdk::Timestamp;

pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now_unix_seconds(&self) -> u64;

    fn now_unix_nanos(&self) -> u128 {
        u128::from(self.now_unix_seconds()) * 1_000_000_000
    }

    fn now_nostr_timestamp(&self) -> Timestamp {
        Timestamp::from(self.now_unix_seconds())
    }
}

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
