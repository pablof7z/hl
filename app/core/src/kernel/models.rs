//! Kernel-owned record types (Phase 7 — Part-C prep).
//!
//! These artifact / podcast record types are used directly by the kernel
//! (`kernel/domains/podcast.rs`, `kernel/actor.rs`) and were relocated out of
//! the bespoke `crate::models` so the kernel no longer depends on the bespoke
//! lane. The bespoke `crate::models` re-imports them while it still exists; the
//! ~24 bespoke files that reference `crate::models::ArtifactRecord` (etc.)
//! continue to resolve unchanged.
//!
//! Field shapes and derives are preserved verbatim from the original
//! definitions (FFI + JSON round-trip compatibility — do not reorder fields).

use serde::{Deserialize, Serialize};

/// Mirrors `ArtifactPreview` in `web/src/lib/ndk/artifacts.ts:19-53`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct ArtifactPreview {
    pub id: String,
    pub url: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub description: String,
    /// "article" | "book" | "podcast" | "video" | "paper" | "web"
    pub source: String,
    pub domain: String,
    pub catalog_id: String,
    pub catalog_kind: String,
    /// NIP-73 feed GUID (from `<podcast:guid>` in the RSS feed). Identifies
    /// the show. Emitted on shares as a secondary `i podcast:guid:<feed-guid>`
    /// so discovery-by-feed still works alongside the episode identifier.
    pub podcast_guid: String,
    /// NIP-73 episode GUID (from `<item><guid>` in the RSS feed). Identifies
    /// a specific episode — the canonical NIP-73 target for podcast
    /// highlights and NIP-22 comments: `i podcast:item:guid:<episode-guid>`.
    pub podcast_item_guid: String,
    pub podcast_show_title: String,
    pub audio_url: String,
    pub audio_preview_url: String,
    pub transcript_url: String,
    pub feed_url: String,
    pub published_at: String,
    pub duration_seconds: Option<i64>,
    /// Primary reference tag: "a" | "e" | "i"
    pub reference_tag_name: String,
    pub reference_tag_value: String,
    pub reference_kind: String,
    /// Highlight reference tag: "a" | "e" | "r"
    pub highlight_tag_name: String,
    pub highlight_tag_value: String,
    pub highlight_reference_key: String,
    /// NIP-73 podcast chapter list (from `chapter` tags on the kind:11
    /// share). Each entry: `["chapter", "<seconds>", "<title>"]`. Empty when
    /// the source has no chapters or the publisher didn't capture them.
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct Chapter {
    pub start_seconds: f64,
    pub title: String,
}

/// Mirrors `ArtifactRecord` in `web/src/lib/ndk/artifacts.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct ArtifactRecord {
    pub preview: ArtifactPreview,
    pub group_id: String,
    pub share_event_id: String,
    pub pubkey: String,
    pub created_at: Option<u64>,
    pub note: String,
}

/// Last podcast playback position persisted by the Rust core. Native shells
/// own AV playback handles, but durable playback state and the cold-launch
/// episode projection live here so every platform resumes the same episode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastPositionRecord {
    pub guid: String,
    pub position_seconds: f64,
    pub last_played_at_unix_seconds: u64,
    pub artifact: ArtifactRecord,
}
