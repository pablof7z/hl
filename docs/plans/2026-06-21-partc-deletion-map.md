# Part C Deletion Map — bespoke-lane teardown blueprint

Status: **pre-Part-C audit (read-only)**. Branch `wf/phase7-teardown`, HEAD `ee3dc234`.
Purpose: the concrete blueprint for deleting the bespoke `HighlighterCore` lane +
dropping `nostr-sdk`/`nostrdb` — what deletes NOW vs what is GATED on which gate,
for the user's Part-C gate decision.

## Headline counts

- **iOS bespoke-FFI call-sites remaining: ~235** across **~83 Swift files** (`safeCore.`/`.core.`, excluding the generated bindings + the `SafeHighlighterCore` wrapper defs).
- **Screens already kernel-cut: 27** (primary data + all writes on the kernel; only presentation-projection / coexist-reads remain → mechanically stripped at Part C).
- **Gated remnant clusters: 8** (2 awaiting in-flight kernel cherry-picks, 3 nmp-issue-gated, podcast lane, room-admin, share-flow) **+ 1 session/data foundation** (removed with the auth-flip at Part C).
- **Rust bespoke modules: ~50** (`app/core/src/*.rs`, ~51k LOC). At a FULL Part-C delete: all ~50 go. Under a GATED-PARTIAL delete: **~18 modules must stay** (the ones the live remnants still call); **~32 deletable** once the cut screens' coexist-reads are stripped.
- **`nostr-sdk` referenced in 39 `app/core/src` files; `nostrdb`/`ndb` in 36.** Kernel production code: **zero** nostr-sdk deps (the only hit, `kernel/domains/articles.rs:586`, is a `#[cfg(test)]` parity import).

---

## 1. iOS bespoke-FFI usage — classified

### 1a. CUT screens (kernel-backed; only coexist-reads / presentation-projections remain)
These 27 stores/views read their PRIMARY data + do ALL writes via the kernel. The
residual `safeCore.` calls are pure presentation projections (`projectXxx`) and a
few coexist-reads that Part C strips mechanically (no new kernel work):

| Screen / store | Kernel source | Residual bespoke (delete at Part C) |
|---|---|---|
| Chat (ChatStore/ChatView) | roomChat snapshot + postChat | projection helpers |
| Comments (CommentsStore/ThreadView/CommentRow/CommentsAttachment) | commentThread + postComment/react/bookmark | projectCommentThread/Node/ActionChrome |
| Discussions (DiscussionStore/List/Detail/Composer) | roomDiscussions + postDiscussion | projection helpers |
| Feedback (FeedbackStore/ThreadStore/Views) | feedbackThreads/Thread + post | projection helpers |
| HomeFeed/Highlights (HomeFeedStore/HighlightsTab/HighlightDetail/HighlightFeedCard/ReadingFeedCard) | homeFeed snapshot | presentation projections |
| Profile (ProfileStore/ProfileView) | openProfile + projectProfileRelationship + follow/unfollow | articles/highlights tabs = live (per lead) |
| RoomExplorer | roomExplorer snapshot | discovery projections |
| ArticleReader (ArticleReaderStore/View) | articleReader snapshot (overlay highlights) + publishHighlight | **BODY read = GATED #1695** (see 1b) |
| Search (SearchStore/View/SeeAll) | search snapshot (articles/highlights/communities) | **profiles bucket = GATED #1697** |
| Bookmarks (BookmarkStore/View) | bookmarks snapshot (articles pane) | **sets/web panes = GATED #1653** |
| NetworkSettings (Store/View) | relay add/remove/role + rooms-relay-list | reconnect/NIP-11/cache = NATIVE (by design); projections |
| Auth (LoginView routing + auth_error) | appRoot.routeKind + auth_error | sign-in = live until the **auth-flip at Part C** (§3) |

### 1b. GATED-REMNANT clusters (bespoke kept until the gate resolves)

**(A) Awaiting in-flight kernel cherry-picks** (lead is landing these; then I do the Swift cut):
- **room-home-agg** → `RoomStore`, `RoomHomeView` (library lanes), `RoomLibraryArticle/Book/PodcastCardView`, `ArtifactDetailView`, the discussion artifact-attachment chip (#1). Bespoke `getRoomHomeSnapshot` (artifact+highlight+comment aggregation; kernel `RoomLaneRow` is raw kind:9/11 only). Needs: `room_library.rs`, `room_lanes.rs`, `room_state.rs`, `artifact_detail.rs`, `room_preview.rs`.
- **capture-publish-parity** → `CaptureStore` (publish), `CapturePageView`, `CaptureMetadataSheet`, `BookScannerView`, `BookPicker`, `CommunityPicker`, `ManualISBNEntryView`. Bespoke `publishCapture` (multi-event: artifact + kind:9802 + kind:16 share, or kind:20 picture + imeta). Needs: `capture.rs`, `pictures.rs`, `blossom.rs`, `ocr.rs`, `isbn_lookup.rs`, `recent_books.rs`.

**(B) nmp-issue-gated** (build-upstream-vs-accept-remnant = the user's call):
- **#1695 (article body)** → `ArticleBodyView` + `MarkdownRenderer` + `ArticleReaderView` body read (select-to-highlight/overlay/footnotes; nmp `NostrContentView` is read-only). Needs: `article_reader.rs` (body), `articles.rs` (markdown).
- **#1653 (bookmarks sets/curation + web)** → `BookmarkMenuButton`, `SetDetailView`, Bookmarks sets/web panes. Needs: `lists.rs`, `curation.rs`, `bookmarks.rs` (sets), `web_metadata.rs`.
- **#1697 (search profiles)** → Search profiles bucket (kind:0 local scan). Needs: `search.rs` (profile scan), `profile.rs`.

**(C) Podcast lane** (Phase 5; not yet kernel-cut — its own gate): `PodcastPlayerStore`, `PodcastListeningView`, `MiniPlayerView`, `ClipComposerSheet`, `ClipThreadView`, `MemberClipRow`, `WaveformExtractor`. Needs: `podcast_playback.rs`, `podcast_transcript.rs`, `podcast_position.rs`, `comments.rs` (clip comments), `blossom.rs`.

**(D) Room admin** (create/invite — not cut): `CreateRoomSheet` (createRoom), `RoomInviteView` (sendRoomInvites), `RoomShareCard`. Needs: `groups.rs`, `room_invites.rs`.

**(E) Share flow**: `ShareToCommunitySheet` (publishArtifact/shareHighlightToRoom), `ShareQueueProcessor` (publishShareQueueItem). Needs: `share_targets.rs`, `outbox.rs`. (Note: kernel `shareToRoom` facade exists — a future small cut could move `shareHighlightToRoom`.)

**(F) Misc still-live**: `SettingsView`, `KeysView`, `MediaSettingsView`, `EditProfileSheet` (profile edit write), `WebReaderView` (web_metadata/nip05), `NostrRichText`/`nostr_entities`, `RoomCoverCard`/`FriendsOnRoomCard`/`RoomSquareTile`/`RoomPreviewSheet` (room previews), `RecommendationsTab` (recommendations.rs), `GlobalUserToolbar`. Local-UI selection helpers (`toggleImportRelaySelection`, `toggleOnboardingInterestSelection`) are pure UI state — not nostr writes.

### 1c. SHOULD-BE-GONE (leftovers a completed cut missed)
**None found — verified at BOTH the write AND read level.**

*Write level:* every remaining bespoke WRITE is either gated (capture/podcast/curation),
an un-cut screen (room-admin/share), the app-level article-bookmark toggle
(`HighlighterStore.toggleArticleBookmarkSnapshot` — a not-yet-cut write, candidate for
a future small kernel cut, not a bug), or a local-UI toggle (not a nostr write). No
cut screen retains a stray bespoke publish.

*Read level (per-cut-screen `subscribe*` / `getXxxSnapshot` scan):* every residual
bespoke read in a cut-screen store maps to a DOCUMENTED gated-remnant, not a stray
leftover:
- `ProfileStore.subscribeUserProfile` / `getProfilePageSnapshot` → the articles/highlights
  tabs stay LIVE per lead (only metadata+relationship was cut).
- `ArticleReaderStore.subscribeArticle` → the BODY read (gated nmp #1695); overlay+publish are kernel.
- `BookmarkStore.subscribeBookmarkSets/FollowingCurationSets/WebBookmarks` + `getBookmarkLibrarySnapshot` → the sets/web panes (gated nmp #1653); only the articles pane was cut.
- `SearchStore.getSearchResultsSnapshot` (profiles bucket, gated nmp #1697) + `getSearchChromeSnapshot` (searchRelays footnote chrome).
No cut-screen store retains a stray primary-data read. (Cut screens otherwise hold only
`projectXxx` presentation projections, all removed at Part C.)

---

## 2. Rust bespoke-lane inventory (`app/core/src`, ~50 modules / ~51k LOC)

**Foundation (deleted at Part C with the session/data layer):** `client.rs` (4516),
`nostr_runtime.rs` (1567), `subscriptions.rs` (2235), `session.rs` (751), `nip46.rs`,
`outbox.rs`, `models.rs` (relocate the few kernel-shared types — already partly done
in `kernel::models`), `lib.rs` (bespoke exports).

**Already-cut domains (deletable once the cut screens' coexist-reads are stripped — the mechanical Part-C strip, no gate):** `chat.rs`, `discussions.rs`, `feedback.rs`,
`comments.rs` (non-clip), `reactions.rs`, `follows.rs`, `reads.rs`, `relays.rs`
(writes cut; projections + NIP-11 native), `relay_polish.rs`, `recommendations.rs`,
`nostr_entities.rs`, `whats_new.rs`, `onboarding.rs`, `nip05.rs`, `discovery.rs`,
`room_state.rs`(?), `highlights.rs` (home feed cut; room/clip uses gated),
`articles.rs`/`article_reader.rs` (overlay cut; body gated), `search.rs` (buckets cut;
profiles gated), `bookmarks.rs` (articles cut; sets gated), `blossom.rs` (capture gated),
`reactions.rs`.

**GATED — must stay until their gate resolves (~18):** `room_library.rs`,
`room_lanes.rs`, `artifact_detail.rs`, `room_preview.rs` (room-home-agg);
`capture.rs`, `pictures.rs`, `ocr.rs`, `isbn_lookup.rs`, `recent_books.rs` (capture);
`article_reader.rs`(body)/`articles.rs`(markdown) (#1695); `lists.rs`, `curation.rs`,
`web_metadata.rs` (#1653); `profile.rs`(search-scan) (#1697); `podcast_playback.rs`,
`podcast_transcript.rs`, `podcast_position.rs` (podcast); `groups.rs`, `room_invites.rs`
(room admin); `share_targets.rs` (share).

(`podcast_transcript.rs` parsing already has Rust tests + the kernel `loadPodcastTranscript`
domain — the bespoke copy is podcast-UI-gated, not kernel-blocked.)

---

## 3. Auth-ownership-flip at Part C (deferred here; folds into deletion)

When the bespoke lane deletes, kernel-sole-session-ownership becomes automatic — no
throwaway two-store bridge:
- **Remove:** `RootSceneView.task { store.bootstrap() }`; the `onChange(store.isLoggedIn)→kernel.logout` + `onChange(store.isOnboardingComplete)→kernel.completeOnboarding` mirrors; `AppSessionStore` (entire file); `App.swift` live-lane bootstrap bits; `LoginView` live-lane sign-in (`safeCore.loginNsec/pairBunker/startDefaultNostrConnect` + `store.completeLogin`) → dispatch `hl.auth.sign_in_nsec`/`pair_bunker`/`start_nostr_connect`/`create_account`; `HighlighterStore.bootstrap/completeLogin/logout`.
- **Restore-on-launch (verified works):** nmp auto-restores its keyring on boot →
  `IdentityChanged(Some)` → `SessionState::Present` → `appRoot.routeKind = RootShell`.
  No Swift-`KeychainService` bridge needed (today's two-store split — nmp keyring vs
  Swift keychain — is exactly what made the flip unsafe DURING coexistence; deletion
  dissolves it). `KeychainService` (Swift) + the `LoadSession/ClearSession` keychain
  capability can be deleted; nmp owns persistence.
- **auth_error** (shipped `ee3dc234`) already gives LoginView its kernel error source.

---

## 4. Grep-proof targets

- **FULL deletion (all gates resolved):**
  `grep -rn "HighlighterCore\|nostr_sdk\|nostrdb" app/core/src` → **0**, and remove
  `nostr-sdk` / `nostr` / `nostr-ndb` / `nostrdb` from `app/core/Cargo.toml`
  (lines 17–20, 61). iOS: `grep -rn "safeCore\.\|SafeHighlighterCore\|HighlighterCore"
  app/ios/.../Sources` (excl. Generated) → 0; delete `SafeHighlighterCore.swift`,
  `AppSessionStore.swift`, `EventBridge.swift`.
- **GATED-PARTIAL deletion (today's reality):** the ~18 gated modules + their
  `nostr_sdk`/`nostrdb` usage remain; everything in §2's "already-cut domains" +
  "foundation" deletes. Expected residual grep hits = only the gated-remnant modules
  listed in §2.

---

## Recommended Part-C sequencing
1. Land the 2 in-flight kernel cherry-picks (room-home-agg, capture-publish-parity) → I do those 2 Swift cuts → clusters (A) clear.
2. User gate decision on the 3 nmp issues (#1695 / #1653 / #1697) + podcast lane + room-admin/share: build-upstream vs accept-remnant.
3. FULL delete if all gates cleared (grep-proof = 0); else GATED-PARTIAL delete (foundation + already-cut domains) leaving only the gated modules, and re-run this map.
4. The auth-flip (§3) executes as part of the foundation deletion.
