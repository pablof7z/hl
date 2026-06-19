//! Articles domain — NIP-23 long-form article projection (slice 4A).
//!
//! ## Responsibilities
//!
//! * **READ** — decode `"nmp.nip23.articles"` typed-sidecar frames into
//!   `AppState::articles` (raw `ArticleRow` records keyed by address). Called
//!   from `projections::dispatch_typed_frame` when the `schema_id` arm matches.
//!
//! * **VIEW** — `ViewId::ArticleReader{address}` / `ViewRoute::ArticleReader` /
//!   `ViewSnapshot::ArticleReader(KernelArticleReaderSnapshot)`. The snapshot
//!   is computed by `project_article_reader_snapshot` directly from
//!   `AppState::articles`; no per-address NMP claim is needed because the
//!   longform projection already carries full `ArticleProjection` documents
//!   (including `content_tree`) for every article the kernel has seen this
//!   session.
//!
//! ## NMP projection seam
//!
//! `nmp-defaults::register_longform_projection` is called by
//! `nmp_defaults::register_defaults` (which hl calls at boot in `start_nmp_app`)
//! with the default `NmpDefaults { longform: true, .. }`. The projection
//! observer accumulates kind:30023 events; the typed FlatBuffers sidecar arrives
//! via the update callback as `KernelEvent::NmpSnapshotFrame` with
//! `schema_id == "nmp.nip23.articles"`. No separate registration call is needed
//! from hl — the default boot sequence is sufficient.
//!
//! ## No WRITE side in 4A
//!
//! Slice 4A is read-path only. Write actions (publish/edit an article) are not
//! in scope for Phase 4A and will be added in a later slice.
//!
//! ## Threading
//!
//! `apply_articles` runs on the **actor thread** (inside
//! `projections::dispatch_typed_frame`, called from `reduce_event`). It is
//! synchronous and non-blocking (FlatBuffers decode only). D6: decode errors
//! leave `AppState::articles` unchanged.
//!
//! ## Snapshot raw-data doctrine (D1)
//!
//! `ArticleRow` carries only raw protocol data — no `"Untitled"` title fallback,
//! no `"{minutes} min read"` formatted string, no `"#{tag}"` hashtag formatting.
//! All presentation strings are Swift / D1 responsibility.

use std::collections::BTreeMap;

use nmp_content::wire::longform_fb::decode_longform_articles;

use crate::kernel::app::AppState;
use crate::kernel::snapshot::{ArticleRow, KernelArticleReaderSnapshot, ViewSnapshot};
use crate::kernel::view::ViewId;

// Re-export so `projections.rs` can match without importing nmp_content directly.
pub(crate) use nmp_content::wire::longform_fb::SCHEMA_ID as ARTICLES_SCHEMA_ID;

// ─── READ side: projection frame apply ──────────────────────────────────────

/// Apply a decoded `"nmp.nip23.articles"` FlatBuffers payload to `state`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id ==
/// "nmp.nip23.articles"`. Replaces `AppState::articles` with the full
/// `ArticleFeedItem` + `ArticleProjection` document set from the sidecar.
///
/// D6: any decode error leaves `AppState::articles` unchanged (silent no-op).
/// D1: `ArticleRow` is raw protocol data only — no formatted strings.
///
/// Must be non-blocking — runs on the actor thread (FlatBuffers decode only).
pub(crate) fn apply_articles(state: &mut AppState, payload: &[u8]) {
    match decode_longform_articles(payload) {
        Ok(longform) => {
            // Rebuild the full articles map from the decoded projection.
            // The projection is authoritative for the entire known set —
            // it supersedes whatever was in AppState::articles (the sidecar
            // carries the live snapshot, not a diff).
            let mut articles: BTreeMap<String, ArticleRow> = BTreeMap::new();
            for (address, doc) in &longform.documents {
                articles.insert(
                    address.clone(),
                    ArticleRow {
                        address: address.clone(),
                        id: doc.id.clone(),
                        author_pubkey: doc.author_pubkey.clone(),
                        author_display_name: doc.author_display_name.clone(),
                        author_picture_url: doc.author_picture_url.clone(),
                        title: doc.title.clone(),
                        summary: doc.summary.clone(),
                        hero_image_url: doc.hero_image_url.clone(),
                        d_tag: doc.d_tag.clone(),
                        created_at: doc.created_at,
                        content_tree_bytes: nmp_content::wire::encode_content_tree(
                            &doc.content_tree,
                        ),
                    },
                );
            }
            // Merge feed items for addresses not already in documents
            // (ArticleFeedItem rows are trimmed — no content_tree — but they
            // carry the feed-list fields; we promote them if the full doc
            // arrived too so we do not clobber richer data). If a document
            // is absent, insert a row with empty content_tree_bytes so the
            // feed list is still populated.
            for item in &longform.articles {
                articles
                    .entry(item.address.clone())
                    .or_insert_with(|| ArticleRow {
                        address: item.address.clone(),
                        id: item.id.clone(),
                        author_pubkey: item.author_pubkey.clone(),
                        author_display_name: None,
                        author_picture_url: None,
                        // ArticleFeedItem uses String (empty = absent per D1);
                        // promote to Option for the full ArticleRow.
                        title: if item.title.is_empty() {
                            None
                        } else {
                            Some(item.title.clone())
                        },
                        summary: if item.summary.is_empty() {
                            None
                        } else {
                            Some(item.summary.clone())
                        },
                        hero_image_url: if item.hero_image_url.is_empty() {
                            None
                        } else {
                            Some(item.hero_image_url.clone())
                        },
                        d_tag: item.d_tag.clone(),
                        created_at: item.created_at,
                        content_tree_bytes: Vec::new(),
                    });
            }
            state.articles = articles;
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "articles::apply_articles: decode error — AppState::articles unchanged (D6)"
            );
        }
    }
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project an `KernelArticleReaderSnapshot` for `ViewId::ArticleReader{address}`.
///
/// Returns `None` when the article is not yet in `AppState::articles`
/// (the view is open but the sidecar has not arrived yet — normal for the
/// brief window between `OpenView` and the first NMP snapshot tick).
///
/// D1: raw fields only — no `"Untitled"` fallback, no formatted duration strings.
pub(crate) fn project_article_reader_snapshot(
    state: &AppState,
    address: &str,
) -> Option<ViewSnapshot> {
    let row = state.articles.get(address)?;
    Some(ViewSnapshot::ArticleReader(KernelArticleReaderSnapshot {
        address: row.address.clone(),
        id: row.id.clone(),
        author_pubkey: row.author_pubkey.clone(),
        author_display_name: row.author_display_name.clone(),
        author_picture_url: row.author_picture_url.clone(),
        title: row.title.clone(),
        summary: row.summary.clone(),
        hero_image_url: row.hero_image_url.clone(),
        d_tag: row.d_tag.clone(),
        created_at: row.created_at,
        content_tree_bytes: row.content_tree_bytes.clone(),
    }))
}

// ─── Lifecycle helpers ────────────────────────────────────────────────────────

/// Return lifecycle effects for `Cmd::OpenView(ViewId::ArticleReader{..})`.
///
/// In Phase 4A the longform projection is already flowing via the nmp-defaults
/// boot registration — no per-address claim effect is needed. Returns an empty
/// vec (reserved for future fetch-on-demand if the address is not yet in state).
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId) -> Vec<crate::kernel::effect::Effect> {
    match id {
        ViewId::ArticleReader { .. } => {
            // Longform projection auto-populates AppState::articles on every
            // NMP snapshot tick. No explicit claim is needed for 4A.
            vec![]
        }
        _ => vec![],
    }
}

/// Return lifecycle effects for `Cmd::CloseView(ViewId::ArticleReader{..})`.
///
/// No release effect is needed — the longform projection accumulates articles
/// for the session lifetime without per-address ref-counts.
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<crate::kernel::effect::Effect> {
    match id {
        ViewId::ArticleReader { .. } => vec![],
        _ => vec![],
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use nmp_content::longform::ArticleFeedItem;
    use nmp_content::wire::longform_fb::encode_longform_articles;
    use std::collections::BTreeMap;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    /// Build a minimal valid `nmp.nip23.articles` FlatBuffers payload.
    fn make_articles_payload(
        feed: &[ArticleFeedItem],
        docs: &BTreeMap<String, nmp_content::embed_projection::ArticleProjection>,
    ) -> Vec<u8> {
        encode_longform_articles(feed, docs)
    }

    /// Minimal ArticleFeedItem for tests — no formatted strings, no "Untitled".
    fn feed_item(
        address: &str,
        id: &str,
        pubkey: &str,
        d_tag: &str,
        created_at: u64,
    ) -> ArticleFeedItem {
        ArticleFeedItem {
            address: address.to_string(),
            id: id.to_string(),
            author_pubkey: pubkey.to_string(),
            title: String::new(), // absent — empty per D1 placeholder
            summary: String::new(),
            hero_image_url: String::new(),
            d_tag: d_tag.to_string(),
            created_at,
        }
    }

    // 4A-T1: articles_frame_updates_state_raw
    //
    // A valid `"nmp.nip23.articles"` FlatBuffers frame decoded via
    // `apply_articles` must populate `AppState::articles`. Verifies the raw
    // field round-trip (address/id/author_pubkey/d_tag/created_at present; no
    // formatted strings in the stored row).
    #[test]
    fn articles_frame_updates_state_raw() {
        let mut state = make_state();
        let address = "30023:aabbcc:my-article";
        let id = "deadbeef0000000000000000000000000000000000000000000000000000dead";
        let pubkey = "aabbcc0000000000000000000000000000000000000000000000000000000001";

        let feed = vec![feed_item(address, id, pubkey, "my-article", 1_700_000_000)];
        let docs = BTreeMap::new();
        let payload = make_articles_payload(&feed, &docs);

        apply_articles(&mut state, &payload);

        assert_eq!(state.articles.len(), 1, "one article stored");
        let row = state.articles.get(address).expect("article present");
        assert_eq!(row.address, address);
        assert_eq!(row.id, id);
        assert_eq!(row.author_pubkey, pubkey);
        assert_eq!(row.d_tag, "my-article");
        assert_eq!(row.created_at, 1_700_000_000);
        // D1: no formatted strings — no "Untitled", no "min read"
        // title/summary/hero are None when the feed item is empty
        assert!(
            row.title.is_none(),
            "empty feed item title must be stored as None (D1 — no Untitled fallback)"
        );
        assert!(
            !row.title.as_deref().unwrap_or("").contains("Untitled"),
            "no 'Untitled' fallback in raw snapshot (D1)"
        );
        assert!(
            !row.summary.as_deref().unwrap_or("").contains("min read"),
            "no 'min read' formatted string in raw snapshot (D1)"
        );
    }

    // 4A-T2: article_reader_snapshot_raw_fields_no_labels
    //
    // `project_article_reader_snapshot` must expose raw fields without
    // presentation formatting. Verifies no "Untitled", no "min read", no
    // "#{tag}" strings in the snapshot.
    #[test]
    fn article_reader_snapshot_raw_fields_no_labels() {
        let mut state = make_state();
        let address = "30023:cafebabe:reader-test";
        let id = "1111000000000000000000000000000000000000000000000000000000001111";
        let pubkey = "cafebabe00000000000000000000000000000000000000000000000000000001";

        // Insert a row with a real title — snapshot must return it verbatim.
        state.articles.insert(
            address.to_string(),
            ArticleRow {
                address: address.to_string(),
                id: id.to_string(),
                author_pubkey: pubkey.to_string(),
                author_display_name: None,
                author_picture_url: None,
                title: Some("My Real Title".to_string()),
                summary: Some("A summary.".to_string()),
                hero_image_url: None,
                d_tag: "reader-test".to_string(),
                created_at: 1_700_000_001,
                content_tree_bytes: Vec::new(),
            },
        );

        let snap = project_article_reader_snapshot(&state, address)
            .expect("snapshot present when article is in state");

        match snap {
            ViewSnapshot::ArticleReader(ref s) => {
                assert_eq!(s.address, address);
                assert_eq!(s.author_pubkey, pubkey);
                assert_eq!(s.title.as_deref(), Some("My Real Title"));
                assert!(
                    s.title.as_deref().unwrap_or("").ne("Untitled"),
                    "title must not be a fallback — raw from state (D1)"
                );
                // No "min read" label anywhere in the snapshot fields
                let debug = format!("{:?}", s);
                assert!(
                    !debug.contains("min read"),
                    "no 'min read' formatted string in KernelArticleReaderSnapshot (D1)"
                );
                assert!(
                    !debug.contains("Untitled"),
                    "no 'Untitled' fallback in KernelArticleReaderSnapshot (D1)"
                );
            }
            other => panic!("expected ArticleReader snapshot, got {:?}", other),
        }
    }

    // 4A-T3: malformed_articles_frame_no_ops
    //
    // Garbage bytes passed to `apply_articles` must not panic and must leave
    // `AppState::articles` unchanged (D6).
    #[test]
    fn malformed_articles_frame_no_ops() {
        let mut state = make_state();
        // Seed a real article so we can confirm no clobber on garbage decode.
        state.articles.insert(
            "existing".to_string(),
            ArticleRow {
                address: "existing".to_string(),
                id: "0000000000000000000000000000000000000000000000000000000000000001".to_string(),
                author_pubkey: "0000000000000000000000000000000000000000000000000000000000000002"
                    .to_string(),
                author_display_name: None,
                author_picture_url: None,
                title: None,
                summary: None,
                hero_image_url: None,
                d_tag: "d".to_string(),
                created_at: 0,
                content_tree_bytes: Vec::new(),
            },
        );

        apply_articles(
            &mut state,
            b"NOT A VALID NL23 FLATBUFFER AT ALL \x00\xFF\xFE",
        );

        assert_eq!(
            state.articles.len(),
            1,
            "malformed frame must not clobber AppState::articles (D6)"
        );
        assert!(
            state.articles.contains_key("existing"),
            "pre-existing article must survive a malformed frame (D6)"
        );
    }

    // 4A-T4: closed_article_view_emits_no_snapshot
    //
    // `project_article_reader_snapshot` returns `None` for an address not in
    // `AppState::articles` (view open but sidecar not yet arrived, or view
    // never connected — no snapshot emitted per Non-Negotiable #7).
    #[test]
    fn closed_article_view_emits_no_snapshot() {
        let state = make_state(); // articles is empty
        let snap = project_article_reader_snapshot(&state, "30023:absent:article");
        assert!(
            snap.is_none(),
            "must return None when article is not in AppState::articles"
        );
    }

    // 4A-T5: articles_cleared_on_logout
    //
    // After Logout (session_epoch bump + AppState::articles clear), the articles
    // map must be empty so stale data from the previous session never leaks.
    //
    // This test uses `KernelEvent::ArticlesUpdated` injected directly (no live NmpApp)
    // to confirm the reducer stores and then clears articles on Logout.
    #[test]
    fn articles_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Inject an ArticlesUpdated event directly (models the sidecar decode path).
        let row = ArticleRow {
            address: "30023:pubkey:d".to_string(),
            id: "2222000000000000000000000000000000000000000000000000000000002222".to_string(),
            author_pubkey: "3333000000000000000000000000000000000000000000000000000000003333"
                .to_string(),
            author_display_name: None,
            author_picture_url: None,
            title: Some("Test Article".to_string()),
            summary: None,
            hero_image_url: None,
            d_tag: "d".to_string(),
            created_at: 1_700_000_002,
            content_tree_bytes: Vec::new(),
        };
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ArticlesUpdated(vec![row])),
        );
        assert_eq!(state.articles.len(), 1, "article present before logout");

        // Logout must clear articles.
        step(
            &mut state,
            &clock,
            Cmd::Action(crate::kernel::action::AppAction::Logout),
        );
        assert!(
            state.articles.is_empty(),
            "AppState::articles must be cleared on Logout"
        );
    }

    // 4A-T6: article_reader_none_when_absent
    //
    // Redundant with T4 but exercises the ViewId dispatch path explicitly.
    #[test]
    fn article_reader_none_when_absent() {
        let id = ViewId::ArticleReader {
            address: "30023:nobody:nowhere".to_string(),
        };
        // Confirm lifecycle helpers return empty effects for the ViewId.
        let open_effects = lifecycle_effects_for_view_open(&id);
        assert!(open_effects.is_empty(), "no lifecycle effects for 4A open");
        let close_effects = lifecycle_effects_for_view_close(&id);
        assert!(
            close_effects.is_empty(),
            "no lifecycle effects for 4A close"
        );
    }
}
