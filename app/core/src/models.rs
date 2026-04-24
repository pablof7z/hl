//! UniFFI-exposed data types. These mirror the TypeScript types in
//! `web/src/lib/ndk/{groups,artifacts,highlights}.ts` so Swift/Rust/TS agree on
//! the shape of a community, artifact, and highlight.

#[derive(Debug, Clone, uniffi::Record)]
pub struct CurrentUser {
    pub pubkey: String,
    pub npub: String,
}

/// Mirrors `CommunitySummary` in `web/src/lib/ndk/groups.ts:23-35`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct CommunitySummary {
    pub id: String,
    pub name: String,
    pub about: String,
    pub picture: String,
    /// "open" or "closed"
    pub access: String,
    /// "public" or "private"
    pub visibility: String,
    pub admin_pubkeys: Vec<String>,
    pub member_count: Option<u64>,
    pub relay_url: String,
    pub metadata_event_id: String,
    pub created_at: Option<u64>,
}

/// Mirrors `ArtifactPreview` in `web/src/lib/ndk/artifacts.ts:19-53`.
#[derive(Debug, Clone, uniffi::Record)]
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
    pub podcast_guid: String,
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
}

/// Mirrors `ArtifactRecord` in `web/src/lib/ndk/artifacts.ts`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ArtifactRecord {
    pub preview: ArtifactPreview,
    pub group_id: String,
    pub share_event_id: String,
    pub pubkey: String,
    pub created_at: Option<u64>,
    pub note: String,
}

/// Mirrors `RoomDiscussionRecord` in
/// `web/src/lib/features/discussions/roomDiscussion.ts` — a kind:11 thread
/// tagged `['t','discussion']`, optionally carrying an attached artifact.
#[derive(Debug, Clone, uniffi::Record)]
pub struct DiscussionRecord {
    /// Stable slug from the `d` tag (or event id if the author omitted one).
    pub id: String,
    pub event_id: String,
    pub group_id: String,
    pub pubkey: String,
    pub title: String,
    pub body: String,
    pub summary: String,
    pub created_at: Option<u64>,
    /// Present iff the thread references an artifact via `a | e | i+k`, or
    /// carries an `r` fallback URL. When set, consumers can render the
    /// attachment alongside the title.
    pub attachment: Option<DiscussionAttachment>,
}

/// A kind:11 discussion can reference an artifact the way a share does —
/// the tag shape matches `ArtifactPreview`'s reference fields exactly, so we
/// reuse the same vocabulary here.
#[derive(Debug, Clone, uniffi::Record)]
pub struct DiscussionAttachment {
    /// "a" | "e" | "i" | "r" (r-only means bare URL, no catalog reference).
    pub reference_tag_name: String,
    pub reference_tag_value: String,
    pub reference_kind: String,
    pub url: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub summary: String,
}

/// Mirrors `HighlightRecord` in `web/src/lib/ndk/highlights.ts:19-44`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightRecord {
    pub event_id: String,
    pub pubkey: String,
    pub quote: String,
    pub context: String,
    pub note: String,
    pub artifact_address: String,
    pub event_reference: String,
    pub source_url: String,
    pub source_reference_key: String,
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    pub clip_speaker: String,
    pub clip_transcript_segment_ids: Vec<String>,
    pub created_at: Option<u64>,
}

/// Highlight + its associated artifact (for feed rendering).
#[derive(Debug, Clone, uniffi::Record)]
pub struct HydratedHighlight {
    pub highlight: HighlightRecord,
    pub artifact: Option<ArtifactRecord>,
    /// If this highlight arrived via a kind:16 repost, this is the id of the repost event.
    pub shared_by_event_id: Option<String>,
    /// The author of the repost (may differ from highlight author).
    pub shared_by_pubkey: Option<String>,
}

/// A pending highlight to publish — text + optional context/note.
#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightDraft {
    pub quote: String,
    pub context: String,
    pub note: String,
    /// Optional clip bounds for audio/video (seconds).
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    /// Speaker name for audio clips — empty if unknown or N/A.
    pub clip_speaker: String,
    /// Transcript segment IDs that the clip spans. Empty for non-clip highlights.
    pub clip_transcript_segment_ids: Vec<String>,
    /// Optional photo accompanying the highlight (e.g. the page captured for an
    /// OCR'd book quote). When set, the published kind:9802 carries an
    /// `imeta` tag (NIP-92) referencing the Blossom-hosted blob.
    pub image: Option<BlossomUpload>,
}

/// Result of a successful Blossom upload — what to put in an `imeta` tag.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BlossomUpload {
    /// Public URL the server returned (e.g. `https://blossom.primal.net/<sha>.jpg`).
    pub url: String,
    /// Lowercase hex SHA-256 of the uploaded bytes.
    pub sha256_hex: String,
    /// MIME type the client uploaded as.
    pub mime: String,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    /// Optional alt text — for OCR captures, the recognized text. Empty if none.
    pub alt: String,
}

/// A pending NIP-68 picture (kind:20) to publish into a community.
/// Used as the OCR-fallback path: when the user couldn't or didn't want to
/// extract a highlight quote from the captured photo.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PictureDraft {
    /// The Blossom upload to attach (must already have been uploaded).
    pub image: BlossomUpload,
    /// Free-form note from the user — populates event content.
    pub note: String,
    /// Optional book/article context. When present, an `a`/`e`/`i` reference
    /// tag is included so the picture is discoverable next to that artifact.
    pub artifact: Option<ArtifactRecord>,
    /// NIP-29 group id this picture is being shared into. `None` publishes the
    /// picture as a standalone event (no `h` tag, not scoped to any community).
    pub target_group_id: Option<String>,
}

/// Published kind:20 picture event record returned to the client.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PictureRecord {
    pub event_id: String,
    pub pubkey: String,
    pub group_id: String,
    pub note: String,
    pub image_url: String,
    pub image_sha256: String,
    /// Address/id/url of the artifact this picture references — empty when
    /// the picture stands alone.
    pub artifact_reference_key: String,
    pub created_at: Option<u64>,
}

/// NIP-01 kind:0 profile metadata. Mirrors the fields the web profile page
/// reads from `NDKUser.profile`. Empty strings for missing fields so Swift
/// call sites don't deal with `Option` everywhere.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileMetadata {
    pub pubkey: String,
    pub name: String,
    pub display_name: String,
    pub about: String,
    pub picture: String,
    pub banner: String,
    pub nip05: String,
    pub website: String,
    pub lud16: String,
    /// created_at of the kind:0 event this came from (seconds since epoch).
    pub created_at: Option<u64>,
}

/// NIP-23 long-form article (kind:30023). Dedupe happens by `d` tag with the
/// newest `created_at` winning, matching how the web app renders.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleRecord {
    pub event_id: String,
    pub pubkey: String,
    /// `d` tag — stable identifier. Combined with pubkey forms the addressable id.
    pub identifier: String,
    pub title: String,
    pub summary: String,
    pub image: String,
    /// Markdown body from the event content.
    pub content: String,
    pub hashtags: Vec<String>,
    /// `published_at` tag (seconds since epoch) if present; otherwise falls back to `created_at`.
    pub published_at: Option<u64>,
    pub created_at: Option<u64>,
}

/// One entry in the Following Reads feed — a NIP-23 article surfaced via
/// the user's NIP-02 follow graph, either because a follow authored it or
/// because a follow interacted with it (reaction, repost, reply, NIP-22
/// comment). The `interactor_pubkeys` list lets the UI render a social
/// byline ("Discussed by @alice + 3 others") under the article card.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ReadingFeedItem {
    pub article: ArticleRecord,
    /// The article's author is someone the user follows.
    pub author_followed: bool,
    /// Hex pubkeys of follows who interacted with the article. Deduped.
    /// Empty when the only surfacing signal is `author_followed`.
    pub interactor_pubkeys: Vec<String>,
    /// Most recent timestamp among the article and all interactions — drives
    /// feed sort order. Seconds since epoch.
    pub latest_activity_at: u64,
}

/// Options for initiating a `nostrconnect://` outgoing pairing.
/// Matches Olas's `NDKBunkerSigner.NostrConnectOptions`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NostrConnectOptions {
    pub name: String,
    pub url: String,
    pub image: String,
    /// e.g. "sign_event:11,sign_event:9802,nip44_encrypt"
    pub perms: String,
}

impl Default for NostrConnectOptions {
    fn default() -> Self {
        Self {
            name: "Highlighter".into(),
            url: "https://highlighter.com".into(),
            image: "https://highlighter.com/icon.png".into(),
            perms: crate::relays::DEFAULT_NOSTR_CONNECT_PERMS.into(),
        }
    }
}
