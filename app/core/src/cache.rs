//! nostrdb wrapper — event storage + indexed queries. Thin layer so callers
//! don't need to know nostrdb's API shape.

use crate::errors::CoreError;
use std::path::Path;

pub struct Cache {
    // Held behind a handle so the Rust core owns the nostrdb lifetime.
    _db: (),
}

impl Cache {
    pub fn open(_path: &Path) -> Result<Self, CoreError> {
        // TODO: initialize nostrdb at the given path. Using nostrdb-rs (pablof7z's
        // fork pinned in Cargo.toml). Must enable FTS + kind indexes.
        Ok(Self { _db: () })
    }

    /// Count of kind:9802 highlights indexed per artifact reference key.
    /// Ports `highlightCountsByArtifact` from `web/src/lib/ndk/highlights.ts:275-286`.
    pub fn highlight_counts_by_artifact(
        &self,
        _group_id: &str,
    ) -> Result<std::collections::HashMap<String, u64>, CoreError> {
        todo!("index highlights by sourceReferenceKey within group scope")
    }
}
