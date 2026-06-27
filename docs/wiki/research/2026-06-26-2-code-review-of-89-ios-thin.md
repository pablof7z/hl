---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 3288-3305
agent_attribution: review89
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Code review of #89 iOS thin shell: evaluated joinRoom relay derivation correctness, fail-closed safety, test genuineness, and removal safety"
captured_at: 2026-06-26T20:40:12Z
---

Code review of #89 iOS thin shell: evaluated joinRoom relay derivation correctness, fail-closed safety, test genuineness, and removal safety

---

VERDICT: APPROVE WITH NITS. No blocking issues.

(a) joinRoom derive — CORRECT. reduce_action_join_room (room_home.rs:264) resolves via host_relay_for_discovered_or_joined (room_home.rs:294): checks discovered_groups FIRST (match g.group_id==group_id, .trim(), filter non-empty), then falls back to host_relay_for_joined_group (communities, same match+trim+non-empty). Fail-closed returns vec![] with trace log (room_home.rs:269-275), matching #108's fail-closed contract. Both dispatch arms (typed AppAction actor.rs:397 + "hl.room.join" envelope actor.rs:756) updated consistently; JoinRoomPayload + Swift dict both drop host_relay_url — no shape mismatch.

(b) Fail-closed forecloses NO real path today — confirmed statically. All 3 Swift join sites pass inviteCode:nil and derive groupId from a displayed discovered/community row (RoomExplorerView.swift:105 guards on newNoteworthy/featured; RoomBrowseAllView.swift:79 + ProfileView.swift:464 off room.id). Only onOpenURL handler (App.swift:104) is share-handoff + nip46 — no join-by-link consumer exists; invite-link code is all minting/sharing. Not a regression.

(c) 3 new tests GENUINE. 89-T1/T2 decode the emitted effect and assert parsed["group"]["host_relay_url"]==TEST_RELAY (room_home.rs:1533/1559); T1 seeds only discovered_groups, T2 only communities — exercises both branches. 89-T3 asserts effects.is_empty() (room_home.rs:1586). Updated existing tests seed state then drop the arg, still assert payload — not weakened.

(d) startRoomDiscovery removal SAFE. No live Swift caller (only comments). Auto-start intact: lifecycle_effects_for_view_open wired at actor.rs:2957; reduce_action_start_room_discovery kept (test calls it directly). No dangling code ref to the removed variant.

(e) Grep-gate hits are TRUE non-violations. HighlighterAppKernel.swift:728/747/766 map kernel rows → CommunitySummary.relayUrl for display (presentation, not routing); AddRelaySheet relayUrl is a NIP-11 fetch param; RoomBrowseAllView:78 is a comment. HighlighterStore.core deletion clean — no .core usages remain (only unrelated .coreCandidate). Rust HighlighterCore export kept for Android (acceptable).

Non-blocking nits:
1. Stale doc comments referencing the removed AppAction::StartRoomDiscovery at effect.rs:146, app.rs:135, discovery.rs:131 — reword to "lifecycle discovery".
2. Latent (not a bug): invite_code is plumbed but relay derivation is unreachable for a group absent from both sources, so a future invite-link join of an undiscovered closed group would silently no-op (trace only). Suggest a // TODO at the fail-closed branch.

Note: I did NOT independently re-run cargo test / xcodebuild; the 718-pass / BUILD SUCCEEDED figures are as-reported. Code changes are consistent with them compiling.
</teammate-message>
