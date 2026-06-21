# hl → nmp migration — Part C handoff (cross-machine resume)

**Date:** 2026-06-21. **Reason:** work moving to a different computer. This doc is the
durable resume point (local `~/.claude` memory does NOT travel). Read this + the
deletion map (`docs/plans/2026-06-21-partc-deletion-map.md`) to continue.

## Where we are

All WRITE-bearing content screens are kernel-cut — the kernel is the **sole writer** of
every nostr event. The remaining work is **Part C**: build out the last gate areas, then
delete the bespoke `HighlighterCore` lane (foundation + cut domains) and drop
`nostr-sdk`/`nostrdb`, leaving (per user) only the deferred podcast lane.

## Integration trunk

- **`origin/wf/phase7-teardown` @ `3576a611`** is THE trunk — everything integrated:
  roomhome-agg full-surface matching (`f05469fb`), Capture-UI β cut (`2fdea74e`, kernel
  sole writer), capture artifact-setters (`bec363da`), AS-T1/AS-T2 publish-carries-artifact
  tests (`3576a611`), auth_error projection (`ee3dc234`), Part-C audit (`dbf4a213`+`ce0cb3e4`).
  Gate state at 3576a611: `cargo test --lib` 1071 pass, clippy `-D warnings` clean, iOS build green.
- Resume by checking out `wf/phase7-teardown` on the new machine and merge-training the
  gate branches (below) onto it.

## User decisions (LOCKED — 2026-06-21)

1. **Scope:** build ALL gates, then FULL delete (bespoke-free end state). NOT partial.
2. **Article-body (#1695):** native Swift select-to-highlight/overlay/footnotes layer on
   top of nmp `content_tree` rendering (NOT upstream nmp). Kernel owns the published
   kind:9802. Delete bespoke `article_reader` body + `articles` markdown reads.
3. **Build now:** Search-profiles #1697 + Bookmarks-sets #1653 + Room-admin/share.
4. **Podcast lane:** DEFERRED — the one tracked bespoke remnant (user did NOT select it).
   So the "full" delete = everything EXCEPT podcast modules. **Confirm podcast timing with
   the user before declaring the migration complete.**

## In-flight gate branches (pushed to origin; merge-train onto teardown)

Each was started off `3576a611` in its own worktree (gotcha #8: one agent per worktree).
On the new machine, `git fetch`, then RE-CODEX each from its worktree before merge-training
(gotcha #7b/#7c: parity must compare IDENTITIES **and** the payload the CONSUMER reads, not
counts), resolve shared-enum conflicts keep-both, then cherry-pick onto teardown.

| Branch | Task | Scope | Status |
|---|---|---|---|
| `wf/phase7-teardown` | #24 | Room-home kernel **rich-enrich** (KernelArtifactRecord = full port of `artifacts::artifact_record_from_event` from kind:11 tags, reuse `ArtifactMatchSurface` in `room_home.rs`; +thin `artifact_previews` slice on `RoomDiscussionsSnapshot` for discussion-chip #1) + field-level parity (podcast+book fixtures) → THEN RoomHome-iOS cut | WIP on trunk (committed "WIP:" if incomplete — check `git log`) |
| `wf/search-profiles` | #19 | Kernel profiles bucket (kind:0 local scan, mirror communities bucket `76a727bf`) + identity parity vs bespoke profile-scan + iOS SearchStore profiles cut | WIP pushed |
| `wf/bookmarks-sets` | #20 | Kernel sets/web panes + kernel-sole-writer set-write (kind:30001/30003) + identity parity vs bespoke lists/curation/web_metadata + iOS BookmarkStore/MenuButton/SetDetail cut | WIP pushed |

## Why room-home needed the enrich (don't repeat the gap)

The roomhome-agg slice gave `KernelRoomLane` only a THIN `ArtifactPreviewRow`
(title/image/summary). But iOS RoomHome library cards + `ArtifactDetailView` consume the
RICH bespoke `ArtifactRecord` (~25 fields: audio_url/transcript_url/podcast guids/chapters/
catalog_id/source/reference_tag*). Cutting to thin data would functionally break podcast
playback (`PodcastListeningView` needs audio_url) + book detail (`BookView` needs catalog_id).
The kernel parity tests passed because they compared highlight/comment identities but NOT the
artifact payload richness → **gotcha #7c**: verify the consumer's field needs before a read-cut.

## Remaining Part-C work (tasks #21, #22, #23)

- **#21 Room-admin + share:** kernel slices for create-room/invite (`groups.rs`/`room_invites.rs`)
  + share-to-room/share-queue (`share_targets.rs`/`outbox.rs`) + iOS cuts (CreateRoomSheet/
  RoomInviteView/RoomShareCard/ShareToCommunitySheet/ShareQueueProcessor). Kernel sole writer.
- **#22 Article-body overlay:** native Swift select-to-highlight on nmp content_tree (per
  user decision above). Delete bespoke `article_reader` body + `articles` markdown.
- **#23 FULL deletion (blocked on #19,#20,#21,#22,#24):** delete bespoke `HighlighterCore`
  lane — foundation (`client.rs`/`nostr_runtime.rs`/`subscriptions.rs`/`session.rs`/`nip46.rs`/
  `outbox.rs`/`models.rs` relocate-shared/`lib.rs`) + the ~32 already-cut domains + the 4 newly
  built gate modules + room-home bespoke (`room_library.rs`/`room_lanes.rs`/`artifact_detail.rs`/
  `room_preview.rs`). **Auth-flip folds in** (remove `AppSessionStore`/`EventBridge`/`bootstrap`/
  LoginView live sign-in → dispatch `hl.auth.*`; nmp keyring auto-restore → `SessionState::Present`,
  no Swift-keychain bridge; delete `KeychainService` + LoadSession/ClearSession capability).
  Drop `nostr-sdk`/`nostr`/`nostr-ndb`/`nostrdb` from `app/core/Cargo.toml` (lines 17-20, 61)
  where unused. **PODCAST modules STAY** (deferred remnant).
  - grep-proof: `grep -rn "HighlighterCore\|nostr_sdk\|nostrdb" app/core/src` → ONLY podcast
    modules. iOS: `grep -rn "safeCore\.\|SafeHighlighterCore\|HighlighterCore" app/ios/.../Sources`
    (excl. Generated) → only podcast.
  - Final gate adds `cargo clippy --all-targets` (~14 pre-existing TEST-target lints across
    room_home/search/feed/ocr/reactions/share/tests to clean — never hit the default gate).
  - Full-migration **codex review** → merge.

## Process invariants (do not drop)

- nmp pinned at rev **`d16aea60`** in `app/core/Cargo.toml`. Don't bump as a side effect.
- `codex exec --skip-git-repo-check` from INSIDE each worktree before merging any port slice
  (it has caught data-drops on nearly every slice).
- Gotchas (full list in the prior `~/.claude` memory `hl-nmp-slice-gotchas`, NOT on this
  machine): #1 read PINNED nmp source not the working tree; #5 reads coexist till Part C but
  writes are kernel-sole immediately; #7b count-only parity is still fake; #7c parity must
  cover the consumer's payload; #8 one agent per worktree, lead merge-trains, keep-both on
  shared-enum conflicts.
- App/device state (resume positions, wifi-only, scroll, UI toggles) stays native — never
  published. Only real nostr facts become kernel events.
- nmp is user-governed: file issues with full context for needed nmp changes; user controls
  merges to nmp master.

## Stale worktrees to clean on the new machine

`hl-cap-setters`, `hl-search-comm`, `hl-setter-tests`, `hl-handoff` (this one),
plus per-slice worktrees once merge-trained: `hl-search-profiles`, `hl-bookmarks-sets`,
`hl-roomhome-agg`.
