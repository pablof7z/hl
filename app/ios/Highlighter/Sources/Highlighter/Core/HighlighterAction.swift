import Foundation

/// Typed Swift facade over the `AppActionEnvelope` FFI boundary.
///
/// Views and stores use `kernel.dispatch(.follow(pubkey:))` rather than
/// constructing raw namespace strings. This enum is the sole source of truth
/// for action namespaces on the Swift side — no raw string literals in views.
///
/// Each case mirrors one `hl.*` namespace defined in the Rust envelope router
/// (`kernel/actor.rs → reduce_action_envelope`). Serialization is done here
/// via `JSONSerialization` / `Encodable` helpers so the call sites stay clean.
///
/// Append-only: new actions add a case + `var envelope` arm below.
enum HighlighterAction {

    // ── Auth / session ────────────────────────────────────────────────────────
    case restoreSession
    case retryRestore
    case logout
    case signInNsec(nsec: String)
    case pairBunker(uri: String)
    case startNostrConnect
    case signInNip55
    case createAccount(profileName: String)

    // ── Onboarding / route ────────────────────────────────────────────────────
    case completeOnboarding
    case selectRootTab(tab: UInt8)
    case presentSheet(sheetId: String)
    case dismissSheet

    // ── Relays ────────────────────────────────────────────────────────────────
    case addRelay(url: String, role: String)
    case removeRelay(url: String)
    case setRelayRole(url: String, role: String)
    case setRelayConfigs(relays: [RelayConfig])
    // ── Follows ───────────────────────────────────────────────────────────────
    case follow(pubkey: String)
    case unfollow(pubkey: String)

    // ── Profiles (resolve / release refs) ─────────────────────────────────────
    case resolveProfileRef(pubkey: String)
    case releaseProfileRef(pubkey: String)

    // ── Room actions ──────────────────────────────────────────────────────────
    case joinRoom(groupId: String, inviteCode: String?)
    /// Leave a NIP-29 group (kind:9022 leave-request). Fire-and-forget.
    case leaveRoom(groupId: String, reason: String?)
    case createRoom(groupId: String, name: String, about: String?)
    case addRoomMember(groupId: String, pubkey: String, role: String?)
    case createRoomInvites(groupId: String, codes: [String])
    case shareToRoom(groupId: String, targetEventId: String, targetAuthorPubkey: String?, repost: Bool)

    // ── Share flow (#21) ────────────────────────────────────────────────────────
    /// Publish a kind:11 artifact/article/podcast share into a room. `previewJson`
    /// is the serde-JSON of an `ArtifactPreview` (via `captureArtifactPreviewJson`).
    case shareArtifactToRoom(groupId: String, previewJson: String, note: String)
    /// Publish a kind:16 generic repost of an existing highlight into a room.
    case shareHighlightToRoom(groupId: String, highlightEventId: String, highlightAuthorPubkey: String, relayHint: String)
    /// Mint `count` invite codes + publish kind:9009; read codes from the
    /// SharePublish snapshot.
    case shareMintInvite(groupId: String, count: UInt32)
    /// Clear a terminal share-publish state (sheet dismissed / reopened).
    case shareResetPublish

    // ── Bookmarks ─────────────────────────────────────────────────────────────
    case addBookmark(item: BookmarkRow)
    case removeBookmark(item: BookmarkRow)

    // ── Articles ──────────────────────────────────────────────────────────────
    case openArticle(address: String)
    case closeArticle(address: String)
    case loadMoreArticles

    // ── Reactions ─────────────────────────────────────────────────────────────
    case react(targetEventId: String, reaction: String, targetAuthorPubkey: String?)
    case unreact(reactionEventId: String)
    /// Like-or-unlike a target by event id. The kernel decides react vs unreact
    /// from its own viewer-reaction tracking (the reaction event id stays
    /// kernel-internal). Reused by every like-button.
    case toggleReaction(targetEventId: String, targetAuthorPubkey: String?)

    // ── Search ────────────────────────────────────────────────────────────────
    case runSearch(query: String, scope: HLSearchScope)
    /// Classify one omnibox / paste / search input through NMP's input-intent
    /// resolver (#1865) and route it. The resolved `OmniboxOutcome` is surfaced
    /// in `SearchSnapshot.omnibox`. Empty / whitespace input is a no-op (D6).
    case runOmnibox(query: String)
    case commitSearchRecentQuery(query: String)
    case clearSearchRecentQueries

    // ── What's New ────────────────────────────────────────────────────────────
    case prepareWhatsNew
    case markWhatsNewSeen(shippedAtUnix: UInt64)

    // ── Highlight feed ────────────────────────────────────────────────────────
    case drainHighlightFeed
    case publishHighlight(
        content: String,
        sourceReference: String,
        relayHint: String?,
        note: String?,
        context: String?
    )

    // ── ISBN / BookPicker ─────────────────────────────────────────────────────
    case lookupIsbn(isbn: String)
    case setBookPickerQuery(query: String, recentLimit: UInt32, searchLimit: UInt32)

    // ── Share queue ───────────────────────────────────────────────────────────
    case drainShareQueue

    // ── Audio / podcast (Phase 5H) ────────────────────────────────────────────
    /// Load and play the episode identified by `guid`.  Kernel looks up the
    /// saved resume position and emits `CapabilityRequest::Audio(.load)`.
    /// Pass `resumePositionSeconds` to override the kernel's saved position.
    case audioPlay(url: String, guid: String, artifactJson: String, resumePositionSeconds: Double?)
    /// Pause the currently-loaded player.
    case audioPause
    /// Resume the currently-loaded (paused) player without reloading.
    case audioResume
    /// Seek to an absolute position (kernel clamps to `[0, duration]`).
    case audioSeek(seconds: Double)
    /// Explicitly persist the current resume position (call on app resign-active).
    case audioSetResume(seconds: Double)
    /// Update the kernel-owned podcast clip start.
    case audioClipSetStart(seconds: Double)
    /// Update the kernel-owned podcast clip end.
    case audioClipSetEnd(seconds: Double, durationSeconds: Double)
    /// Clear the kernel-owned podcast clip selection and publish FSM.
    case audioClipClear
    /// Publish the current kernel-owned podcast clip selection as a kind:9802,
    /// optionally followed by a kind:16 repost into a target group.
    case podcastPublishClip(artifactJson: String, note: String?, targetGroupId: String?)

    // ── Capture draft (Phase 5F) ──────────────────────────────────────────────────
    case captureSetQuote(quote: String)
    case captureSetContext(context: String)
    case captureSetNote(note: String)
    case captureSelectWord(wordIndex: UInt64)
    case captureClearSelection
    case captureSetTargetGroup(groupId: String)
    case captureClearTargetGroup
    /// Set the book the capture references — an existing published artifact
    /// (`artifactJson` = serde-JSON of an ArtifactRecord, via captureArtifactRecordJson).
    case captureSetArtifactRecord(artifactJson: String)
    /// Set a pending book (`previewJson` = serde-JSON of an ArtifactPreview, via
    /// captureArtifactPreviewJson) — published kind:11-first on the pending-book path.
    case captureSetArtifactPreview(previewJson: String)
    /// Drop any selected book (standalone capture).
    case captureClearArtifact
    case capturePublish
    case captureReset

    // ── Capture capability triggers (Phase 7 cutover) ─────────────────────────────
    /// Start a document-scan capture (kernel emits the camera CapabilityRequest →
    /// CapturePresenter presents the scanner → provide_capability_result).
    case cameraCapturePage
    /// Start a barcode scan (→ ISBN lookup).
    case cameraScanBarcode
    /// Cancel an in-flight camera capability.
    case cameraCancel
    /// Run OCR on a native-written page image handle (kernel emits the OCR
    /// CapabilityRequest → Vision via the bridge → KernelCaptureSnapshot).
    case ocrRecognize(imageHandle: String)
    /// Upload a (native-rendered) image handle via Blossom; descriptor returns
    /// via the action-results projection.
    case blossomUpload(imageHandle: String, servers: [String])

    // ── Chat (Phase 7 cutover) ──────────────────────────────────────────────────
    /// Open a room's chat: wires the per-room ChatObserver (kernel is sole writer).
    case chatOpen(groupId: String)
    /// Close a room's chat: releases the room buffer.
    case chatClose(groupId: String)
    /// Expand the loaded chat window by one page (bounded by the kernel).
    case chatLoadMore(groupId: String)
    /// Publish a kind:9 chat message into the room (optional reply parent).
    case postChat(groupId: String, content: String, replyToEventId: String?)

    // ── Comments (Phase 7 cutover) ──────────────────────────────────────────────
    /// Publish a NIP-22 kind:1111 comment. `parentEventId == nil` posts a
    /// top-level comment (parent mirrors root); otherwise replies to that comment.
    case postComment(
        rootTagName: String,
        rootTagValue: String,
        rootKind: UInt32,
        parentEventId: String?,
        rootAuthorPubkey: String?,
        parentAuthorPubkey: String?,
        content: String
    )

    // ── Discussions (Phase 7 cutover) ───────────────────────────────────────────
    /// Publish a kind:11 discussion thread into a NIP-29 room.
    case postDiscussion(groupId: String, title: String, body: String, attachmentUrl: String?)

    // ── Curation sets (#1653) ────────────────────────────────────────────────────
    /// Add `itemCoordinate` (a NIP-33 address like `"30023:<pk>:<d>"`) to the
    /// kind:30004 curation set identified by `setCoordinate`.
    /// Kernel is the sole kind:30004 writer; fire-and-forget.
    case addToSet(setCoordinate: String, itemCoordinate: String)
    /// Remove `itemCoordinate` from the curation set identified by `setCoordinate`.
    case removeFromSet(setCoordinate: String, itemCoordinate: String)
    /// Create a brand-new kind:30004 curation set with `title` and immediately
    /// add `itemCoordinate` as its first member. Fire-and-forget.
    case createAndAddToSet(title: String, itemCoordinate: String)

    // ── Issue #63 curation set management ────────────────────────────────────────
    /// Rename the kind:30004 curation set identified by `setCoordinate`.
    /// Preserves all membership and metadata tags; only the title changes.
    /// Fire-and-forget (D6). No-op when the set is not found in kernel state.
    case renameSet(setCoordinate: String, title: String)
    /// Delete the kind:30004 curation set identified by `setCoordinate`.
    /// Publishes a NIP-09 kind:5 deletion event. Fire-and-forget (D6).
    /// No-op when the set is not found in kernel state.
    case deleteSet(setCoordinate: String)
    /// Create a brand-new empty kind:30004 curation set with `title`.
    /// No initial member is added. Fire-and-forget (D6).
    case createSet(title: String)

    // ── Profile update (Phase 7 Part C) ──────────────────────────────────────────
    /// Publish an updated kind:0 profile metadata event via the kernel.
    /// Fire-and-forget (D6). Rust preserves unknown kind:0 fields (round-trip safe).
    /// `nil` fields are omitted; `Some("")` clears a field.
    case updateProfile(
        displayName: String?,
        name: String?,
        about: String?,
        pictureUrl: String?,
        bannerUrl: String?,
        website: String?,
        nip05: String?,
        lightningAddress: String?
    )
    /// Inform the kernel that the iOS NWPathMonitor detected a path change.
    /// `wifiOnly` mirrors `UserDefaults["hl.network.wifi_only"]`. Fire-and-forget (D6).
    case applyNetworkPath(isWifi: Bool, wifiOnly: Bool)

    // ── Feedback / shake-to-share (Phase 7 cutover) ─────────────────────────────
    case feedbackOpenList
    case feedbackCloseList
    case feedbackOpenThread(rootEventId: String)
    case feedbackCloseThread
    /// Publish a new feedback root note (NIP-22 kind:1111 under the project root).
    case feedbackPostRoot(content: String)
    /// Reply into an open feedback thread.
    case feedbackPostReply(rootEventId: String, content: String, parentAuthorPubkey: String?)

    // MARK: - Envelope serialization

    /// Encodes this action as an `AppActionEnvelope` ready for `dispatchAction`.
    var envelope: AppActionEnvelope {
        switch self {

        // ── Auth / session ────────────────────────────────────────────────────
        case .restoreSession:
            return AppActionEnvelope(namespace: "hl.auth.restore_session", json: "{}")
        case .retryRestore:
            return AppActionEnvelope(namespace: "hl.auth.retry_restore", json: "{}")
        case .logout:
            return AppActionEnvelope(namespace: "hl.auth.logout", json: "{}")
        case .signInNsec(let nsec):
            return AppActionEnvelope(namespace: "hl.auth.sign_in_nsec",
                                     json: jsonObject(["nsec": nsec]))
        case .pairBunker(let uri):
            return AppActionEnvelope(namespace: "hl.auth.pair_bunker",
                                     json: jsonObject(["uri": uri]))
        case .startNostrConnect:
            return AppActionEnvelope(namespace: "hl.auth.start_nostr_connect", json: "{}")
        case .signInNip55:
            return AppActionEnvelope(namespace: "hl.auth.sign_in_nip55", json: "{}")
        case .createAccount(let profileName):
            return AppActionEnvelope(namespace: "hl.auth.create_account",
                                     json: jsonObject(["profile_name": profileName]))

        // ── Onboarding / route ────────────────────────────────────────────────
        case .completeOnboarding:
            return AppActionEnvelope(namespace: "hl.route.complete_onboarding", json: "{}")
        case .selectRootTab(let tab):
            return AppActionEnvelope(namespace: "hl.route.select_root_tab",
                                     json: jsonObject(["tab": tab]))
        case .presentSheet(let sheetId):
            return AppActionEnvelope(namespace: "hl.route.present_sheet",
                                     json: jsonObject(["sheet_id": sheetId]))
        case .dismissSheet:
            return AppActionEnvelope(namespace: "hl.route.dismiss_sheet", json: "{}")

        // ── Relays ────────────────────────────────────────────────────────────
        case .addRelay(let url, let role):
            return AppActionEnvelope(namespace: "hl.relay.add",
                                     json: jsonObject(["url": url, "role": role]))
        case .removeRelay(let url):
            return AppActionEnvelope(namespace: "hl.relay.remove",
                                     json: jsonObject(["url": url]))
        case .setRelayRole(let url, let role):
            return AppActionEnvelope(namespace: "hl.relay.set_role",
                                     json: jsonObject(["url": url, "role": role]))
        case .setRelayConfigs(let relays):
            let rows = relays.map {
                [
                    "url": $0.url,
                    "read": $0.read,
                    "write": $0.write,
                    "rooms": $0.rooms,
                    "indexer": $0.indexer
                ] as [String: Any]
            }
            return AppActionEnvelope(namespace: "hl.relay.set_configs",
                                     json: jsonAny(["relays": rows]))
        // ── Follows ───────────────────────────────────────────────────────────
        case .follow(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.follow",
                                     json: jsonObject(["pubkey": pubkey]))
        case .unfollow(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.unfollow",
                                     json: jsonObject(["pubkey": pubkey]))

        // ── Profiles ──────────────────────────────────────────────────────────
        case .resolveProfileRef(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.resolve_ref",
                                     json: jsonObject(["pubkey": pubkey]))
        case .releaseProfileRef(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.release_ref",
                                     json: jsonObject(["pubkey": pubkey]))

        // ── Room actions ──────────────────────────────────────────────────────
        case .joinRoom(let groupId, let inviteCode):
            var dict: [String: Any] = ["group_id": groupId]
            if let code = inviteCode { dict["invite_code"] = code }
            return AppActionEnvelope(namespace: "hl.room.join", json: jsonAny(dict))
        case .leaveRoom(let groupId, let reason):
            var dict: [String: Any] = ["group_id": groupId]
            if let reason = reason { dict["reason"] = reason }
            return AppActionEnvelope(namespace: "hl.room.leave", json: jsonAny(dict))
        case .createRoom(let groupId, let name, let about):
            var dict: [String: Any] = ["group_id": groupId, "name": name]
            if let about = about { dict["about"] = about }
            return AppActionEnvelope(namespace: "hl.room.create", json: jsonAny(dict))
        case .addRoomMember(let groupId, let pubkey, let role):
            var dict: [String: Any] = ["group_id": groupId, "pubkey": pubkey]
            if let role = role { dict["role"] = role }
            return AppActionEnvelope(namespace: "hl.room.add_member", json: jsonAny(dict))
        case .createRoomInvites(let groupId, let codes):
            let dict: [String: Any] = ["group_id": groupId, "codes": codes]
            return AppActionEnvelope(namespace: "hl.room.create_invites", json: jsonAny(dict))
        case .shareToRoom(let groupId, let targetEventId, let targetAuthorPubkey, let repost):
            var dict: [String: Any] = [
                "group_id": groupId,
                "target_event_id": targetEventId,
                "repost": repost,
            ]
            if let author = targetAuthorPubkey { dict["target_author_pubkey"] = author }
            return AppActionEnvelope(namespace: "hl.room.share_to_room", json: jsonAny(dict))

        // ── Share flow (#21) ────────────────────────────────────────────────────
        case .shareArtifactToRoom(let groupId, let previewJson, let note):
            // preview_json is the serde-JSON of an ArtifactPreview; embed it as a
            // nested object so the kernel deserializes `preview` directly.
            let json = """
            {"group_id":\(jsonString(groupId)),"preview":\(previewJson),"note":\(jsonString(note))}
            """
            return AppActionEnvelope(namespace: "hl.share.artifact_to_room", json: json)
        case .shareHighlightToRoom(let groupId, let eventId, let author, let relayHint):
            let dict: [String: Any] = [
                "group_id": groupId,
                "highlight_event_id": eventId,
                "highlight_author_pubkey": author,
                "relay_hint": relayHint,
            ]
            return AppActionEnvelope(namespace: "hl.share.highlight_to_room", json: jsonAny(dict))
        case .shareMintInvite(let groupId, let count):
            let dict: [String: Any] = [
                "group_id": groupId,
                "count": count,
            ]
            return AppActionEnvelope(namespace: "hl.share.mint_invite", json: jsonAny(dict))
        case .shareResetPublish:
            return AppActionEnvelope(namespace: "hl.share.reset_publish", json: "{}")

        // ── Bookmarks ─────────────────────────────────────────────────────────
        case .addBookmark(let item):
            return AppActionEnvelope(namespace: "hl.bookmark.add",
                                     json: bookmarkJson("item", row: item))
        case .removeBookmark(let item):
            return AppActionEnvelope(namespace: "hl.bookmark.remove",
                                     json: bookmarkJson("item", row: item))

        // ── Articles ──────────────────────────────────────────────────────────
        case .openArticle(let address):
            return AppActionEnvelope(namespace: "hl.article.open",
                                     json: jsonObject(["address": address]))
        case .closeArticle(let address):
            return AppActionEnvelope(namespace: "hl.article.close",
                                     json: jsonObject(["address": address]))
        case .loadMoreArticles:
            return AppActionEnvelope(namespace: "hl.article.load_more", json: "{}")

        // ── Reactions ─────────────────────────────────────────────────────────
        case .react(let targetEventId, let reaction, let targetAuthorPubkey):
            var dict: [String: Any] = ["target_event_id": targetEventId, "reaction": reaction]
            if let author = targetAuthorPubkey { dict["target_author_pubkey"] = author }
            return AppActionEnvelope(namespace: "hl.reaction.react", json: jsonAny(dict))
        case .unreact(let reactionEventId):
            return AppActionEnvelope(namespace: "hl.reaction.unreact",
                                     json: jsonObject(["reaction_event_id": reactionEventId]))
        case .toggleReaction(let targetEventId, let targetAuthorPubkey):
            var dict: [String: Any] = ["target_event_id": targetEventId]
            if let author = targetAuthorPubkey { dict["target_author_pubkey"] = author }
            return AppActionEnvelope(namespace: "hl.reaction.toggle", json: jsonAny(dict))

        // ── Search ────────────────────────────────────────────────────────────
        case .runSearch(let query, let scope):
            return AppActionEnvelope(namespace: "hl.search.run",
                                     json: jsonObject(["query": query, "scope": scope.rawValue]))
        case .runOmnibox(let query):
            return AppActionEnvelope(namespace: "hl.search.omnibox",
                                     json: jsonObject(["query": query]))
        case .commitSearchRecentQuery(let query):
            return AppActionEnvelope(namespace: "hl.search.commit_recent_query",
                                     json: jsonObject(["query": query]))
        case .clearSearchRecentQueries:
            return AppActionEnvelope(namespace: "hl.search.clear_recent_queries", json: "{}")

        // ── What's New ────────────────────────────────────────────────────────
        case .prepareWhatsNew:
            return AppActionEnvelope(namespace: "hl.whats_new.prepare", json: "{}")
        case .markWhatsNewSeen(let shippedAtUnix):
            return AppActionEnvelope(namespace: "hl.whats_new.mark_seen",
                                     json: jsonObject(["shipped_at_unix": shippedAtUnix]))

        // ── Highlight feed ────────────────────────────────────────────────────
        case .drainHighlightFeed:
            return AppActionEnvelope(namespace: "hl.highlight.drain_feed", json: "{}")
        case .publishHighlight(let content, let sourceReference, let relayHint, let note, let context):
            var dict: [String: Any] = ["content": content, "source_reference": sourceReference]
            if let hint = relayHint { dict["relay_hint"] = hint }
            if let note { dict["note"] = note }
            if let context { dict["context"] = context }
            return AppActionEnvelope(namespace: "hl.highlight.publish", json: jsonAny(dict))

        // ── ISBN / BookPicker ─────────────────────────────────────────────────
        case .lookupIsbn(let isbn):
            return AppActionEnvelope(namespace: "hl.isbn.lookup",
                                     json: jsonObject(["isbn": isbn]))
        case .setBookPickerQuery(let query, let recentLimit, let searchLimit):
            return AppActionEnvelope(namespace: "hl.book_picker.set_query",
                                     json: jsonAny(["query": query,
                                                    "recent_limit": recentLimit,
                                                    "search_limit": searchLimit]))

        // ── Share queue ───────────────────────────────────────────────────────
        case .drainShareQueue:
            return AppActionEnvelope(namespace: "hl.share.drain_queue", json: "{}")

        // ── Audio / podcast (Phase 5H) ────────────────────────────────────────
        case .audioPlay(let url, let guid, let artifactJson, let resumePositionSeconds):
            var dict: [String: Any] = ["url": url, "guid": guid, "artifact_json": artifactJson]
            if let pos = resumePositionSeconds { dict["resume_position_seconds"] = pos }
            return AppActionEnvelope(namespace: "hl.audio.play", json: jsonAny(dict))
        case .audioPause:
            return AppActionEnvelope(namespace: "hl.audio.pause", json: "{}")
        case .audioResume:
            return AppActionEnvelope(namespace: "hl.audio.resume", json: "{}")
        case .audioSeek(let seconds):
            return AppActionEnvelope(namespace: "hl.audio.seek",
                                     json: jsonAny(["seconds": seconds]))
        case .audioSetResume(let seconds):
            return AppActionEnvelope(namespace: "hl.audio.set_resume",
                                     json: jsonAny(["seconds": seconds]))
        case .audioClipSetStart(let seconds):
            return AppActionEnvelope(namespace: "hl.audio.clip_set_start",
                                     json: jsonAny(["value": seconds]))
        case .audioClipSetEnd(let seconds, let durationSeconds):
            return AppActionEnvelope(namespace: "hl.audio.clip_set_end",
                                     json: jsonAny(["value": seconds, "duration_seconds": durationSeconds]))
        case .audioClipClear:
            return AppActionEnvelope(namespace: "hl.audio.clip_clear", json: "{}")
        case .podcastPublishClip(let artifactJson, let note, let targetGroupId):
            var dict: [String: Any] = ["artifact_json": artifactJson]
            if let note { dict["note"] = note }
            if let targetGroupId { dict["target_group_id"] = targetGroupId }
            return AppActionEnvelope(namespace: "hl.podcast.publish_clip", json: jsonAny(dict))

        // ── Capture draft (Phase 5F) ──────────────────────────────────────────────────
        case .captureSetQuote(let quote):
            return AppActionEnvelope(namespace: "hl.capture.set_quote",
                                     json: jsonObject(["quote": quote]))
        case .captureSetContext(let context):
            return AppActionEnvelope(namespace: "hl.capture.set_context",
                                     json: jsonObject(["context": context]))
        case .captureSetNote(let note):
            return AppActionEnvelope(namespace: "hl.capture.set_note",
                                     json: jsonObject(["note": note]))
        case .captureSelectWord(let wordIndex):
            return AppActionEnvelope(namespace: "hl.capture.select_word",
                                     json: jsonAny(["word_index": wordIndex]))
        case .captureClearSelection:
            return AppActionEnvelope(namespace: "hl.capture.clear_selection", json: "{}")
        case .captureSetTargetGroup(let groupId):
            return AppActionEnvelope(namespace: "hl.capture.set_target_group",
                                     json: jsonObject(["group_id": groupId]))
        case .captureClearTargetGroup:
            return AppActionEnvelope(namespace: "hl.capture.clear_target_group", json: "{}")
        case .captureSetArtifactRecord(let artifactJson):
            return AppActionEnvelope(namespace: "hl.capture.set_artifact_record",
                                     json: jsonObject(["artifact_json": artifactJson]))
        case .captureSetArtifactPreview(let previewJson):
            return AppActionEnvelope(namespace: "hl.capture.set_artifact_preview",
                                     json: jsonObject(["preview_json": previewJson]))
        case .captureClearArtifact:
            return AppActionEnvelope(namespace: "hl.capture.clear_artifact", json: "{}")
        case .capturePublish:
            return AppActionEnvelope(namespace: "hl.capture.publish", json: "{}")
        case .captureReset:
            return AppActionEnvelope(namespace: "hl.capture.reset", json: "{}")
        case .cameraCapturePage:
            return AppActionEnvelope(namespace: "hl.camera.capture_page", json: "{}")
        case .cameraScanBarcode:
            return AppActionEnvelope(namespace: "hl.camera.scan_barcode", json: "{}")
        case .cameraCancel:
            return AppActionEnvelope(namespace: "hl.camera.cancel", json: "{}")
        case .ocrRecognize(let imageHandle):
            return AppActionEnvelope(
                namespace: "hl.ocr.recognize",
                json: jsonAny(["image_handle": imageHandle])
            )
        case .blossomUpload(let imageHandle, let servers):
            return AppActionEnvelope(
                namespace: "hl.blossom.upload",
                json: jsonAny(["image_handle": imageHandle, "servers": servers])
            )

        // ── Chat (Phase 7 cutover) ──────────────────────────────────────────
        case .chatOpen(let groupId):
            return AppActionEnvelope(namespace: "hl.chat.open",
                                     json: jsonObject(["group_id": groupId]))
        case .chatClose(let groupId):
            return AppActionEnvelope(namespace: "hl.chat.close",
                                     json: jsonObject(["group_id": groupId]))
        case .chatLoadMore(let groupId):
            return AppActionEnvelope(namespace: "hl.chat.load_more",
                                     json: jsonObject(["group_id": groupId]))
        case .postChat(let groupId, let content, let replyToEventId):
            var dict: [String: Any] = [
                "group_id": groupId,
                "content": content,
            ]
            if let replyTo = replyToEventId { dict["reply_to_event_id"] = replyTo }
            return AppActionEnvelope(namespace: "hl.chat.post", json: jsonAny(dict))

        // ── Comments (Phase 7 cutover) ──────────────────────────────────────
        case .postComment(let rootTagName, let rootTagValue, let rootKind,
                          let parentEventId, let rootAuthorPubkey,
                          let parentAuthorPubkey, let content):
            var dict: [String: Any] = [
                "root_tag_name": rootTagName,
                "root_tag_value": rootTagValue,
                "root_kind": rootKind,
                "content": content,
            ]
            if let parent = parentEventId { dict["parent_event_id"] = parent }
            if let rootAuthor = rootAuthorPubkey { dict["root_author_pubkey"] = rootAuthor }
            if let parentAuthor = parentAuthorPubkey { dict["parent_author_pubkey"] = parentAuthor }
            return AppActionEnvelope(namespace: "hl.comment.post", json: jsonAny(dict))

        // ── Discussions (Phase 7 cutover) ───────────────────────────────────
        case .postDiscussion(let groupId, let title, let body, let attachmentUrl):
            var dict: [String: Any] = ["group_id": groupId, "title": title, "body": body]
            if let url = attachmentUrl, !url.isEmpty { dict["attachment_url"] = url }
            return AppActionEnvelope(namespace: "hl.discussion.post", json: jsonAny(dict))

        // ── Feedback (Phase 7 cutover) ──────────────────────────────────────
        case .feedbackOpenList:
            return AppActionEnvelope(namespace: "hl.feedback.open_list", json: "{}")
        case .feedbackCloseList:
            return AppActionEnvelope(namespace: "hl.feedback.close_list", json: "{}")
        case .feedbackOpenThread(let rootEventId):
            return AppActionEnvelope(namespace: "hl.feedback.open_thread",
                                     json: jsonObject(["root_event_id": rootEventId]))
        case .feedbackCloseThread:
            return AppActionEnvelope(namespace: "hl.feedback.close_thread", json: "{}")
        case .feedbackPostRoot(let content):
            return AppActionEnvelope(namespace: "hl.feedback.post_root",
                                     json: jsonObject(["content": content]))
        case .feedbackPostReply(let rootEventId, let content, let parentAuthorPubkey):
            var dict: [String: Any] = ["root_event_id": rootEventId, "content": content]
            if let author = parentAuthorPubkey { dict["parent_author_pubkey"] = author }
            return AppActionEnvelope(namespace: "hl.feedback.post_reply", json: jsonAny(dict))

        // ── Curation sets (#1653) ─────────────────────────────────────────────
        case .addToSet(let setCoordinate, let itemCoordinate):
            return AppActionEnvelope(
                namespace: "hl.curation.add_to_set",
                json: jsonObject(["set_coordinate": setCoordinate, "item_coordinate": itemCoordinate])
            )
        case .removeFromSet(let setCoordinate, let itemCoordinate):
            return AppActionEnvelope(
                namespace: "hl.curation.remove_from_set",
                json: jsonObject(["set_coordinate": setCoordinate, "item_coordinate": itemCoordinate])
            )
        case .createAndAddToSet(let title, let itemCoordinate):
            return AppActionEnvelope(
                namespace: "hl.curation.create_and_add",
                json: jsonObject(["title": title, "item_coordinate": itemCoordinate])
            )

        // ── Issue #63 curation set management ────────────────────────────────
        case .renameSet(let setCoordinate, let title):
            return AppActionEnvelope(
                namespace: "hl.curation.rename_set",
                json: jsonObject(["set_coordinate": setCoordinate, "title": title])
            )
        case .deleteSet(let setCoordinate):
            return AppActionEnvelope(
                namespace: "hl.curation.delete_set",
                json: jsonObject(["set_coordinate": setCoordinate])
            )
        case .createSet(let title):
            return AppActionEnvelope(
                namespace: "hl.curation.create_set",
                json: jsonObject(["title": title])
            )

        // ── Profile update (Phase 7 Part C) ──────────────────────────────────
        case .updateProfile(let displayName, let name, let about, let pictureUrl,
                            let bannerUrl, let website, let nip05, let lightningAddress):
            var dict: [String: Any] = [:]
            if let v = displayName { dict["display_name"] = v }
            if let v = name { dict["name"] = v }
            if let v = about { dict["about"] = v }
            if let v = pictureUrl { dict["picture_url"] = v }
            if let v = bannerUrl { dict["banner_url"] = v }
            if let v = website { dict["website"] = v }
            if let v = nip05 { dict["nip05"] = v }
            if let v = lightningAddress { dict["lightning_address"] = v }
            return AppActionEnvelope(namespace: "hl.profile.update", json: jsonAny(dict))

        // ── Network (Phase 7 Part C) ──────────────────────────────────────────
        case .applyNetworkPath(let isWifi, let wifiOnly):
            return AppActionEnvelope(
                namespace: "hl.network.apply_path",
                json: jsonAny(["is_wifi": isWifi, "wifi_only": wifiOnly])
            )
        }
    }
}

// MARK: - Search scope (Swift side)

/// Search scope passed through the envelope. Maps to the Rust `SearchScope`
/// enum serialised as `"users"` / `"long_form"` / `"notes"`.
enum HLSearchScope: String {
    case users = "users"
    case longForm = "long_form"
    case notes = "notes"
    /// kind:30023 articles + kind:9802 highlights in one query — backs the
    /// unified search screen (Swift buckets the mixed hits by kind).
    case articlesAndHighlights = "articles_and_highlights"
    /// kind:0 + kind:9802 + kind:30023 in one NIP-50 query — unified search
    /// with People, Articles, and Highlights all from a single relay subscription.
    case articlesHighlightsAndUsers = "articles_highlights_and_users"
}

// MARK: - HighlighterApp dispatch facade

extension HighlighterApp {
    /// Typed dispatch convenience. Views call `kernel.app.dispatch(.follow(pubkey:))`
    /// rather than constructing raw `AppActionEnvelope` values.
    func dispatch(_ action: HighlighterAction) {
        dispatchAction(action: action.envelope)
    }
}

// MARK: - Private JSON helpers

/// Encode a `[String: String]` dict to a compact JSON string.
/// Only used inside `HighlighterAction.envelope` — not for arbitrary payloads.
private func jsonObject(_ dict: [String: String]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}

/// Encode a `[String: UInt8]` overload for the `selectRootTab` case.
private func jsonObject(_ dict: [String: UInt8]) -> String {
    let anyDict: [String: Any] = dict.mapValues { Int($0) }
    guard let data = try? JSONSerialization.data(withJSONObject: anyDict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}

/// Encode a `[String: UInt64]` overload for numeric payloads.
private func jsonObject(_ dict: [String: UInt64]) -> String {
    let anyDict: [String: Any] = dict.mapValues { UInt64($0) }
    guard let data = try? JSONSerialization.data(withJSONObject: anyDict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}

/// Encode a `[String: [String]]` payload (for relay list actions).
private func jsonArray(_ dict: [String: [String]]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}

/// Encode any `[String: Any]` payload (for actions with mixed or optional fields).
private func jsonAny(_ dict: [String: Any]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}

/// Encode a Swift `String` as a JSON string literal (with proper escaping).
/// Used when hand-building JSON that embeds a pre-serialized nested object
/// (e.g. `preview` in `hl.share.artifact_to_room`).
private func jsonString(_ value: String) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: [value], options: []),
          let arr = String(data: data, encoding: .utf8) else {
        return "\"\""
    }
    // arr is `["escaped"]`; strip the surrounding brackets to get the bare string.
    return String(arr.dropFirst().dropLast())
}

/// Encode a `BookmarkRow` as `{ "item": <serde-tagged-variant> }` JSON.
///
/// Rust's `serde` default for enums is "externally tagged":
///   `{"Event": {"event_id": "...", "relay": null}}`
///
/// This matches the `AddBookmarkPayload { item: BookmarkRow }` / `RemoveBookmarkPayload`
/// structs expected by the envelope router.
private func bookmarkJson(_ key: String, row: BookmarkRow) -> String {
    let variantDict: [String: Any]
    switch row {
    case .event(let eventId, let relay):
        var inner: [String: Any] = ["event_id": eventId]
        inner["relay"] = relay as Any? ?? NSNull()
        variantDict = ["Event": inner]
    case .address(let coordinate, let relay):
        var inner: [String: Any] = ["coordinate": coordinate]
        inner["relay"] = relay as Any? ?? NSNull()
        variantDict = ["Address": inner]
    case .url(let url):
        variantDict = ["Url": ["url": url]]
    case .hashtag(let hashtag):
        variantDict = ["Hashtag": ["hashtag": hashtag]]
    }
    let dict: [String: Any] = [key: variantDict]
    guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]),
          let str = String(data: data, encoding: .utf8) else {
        return "{}"
    }
    return str
}
