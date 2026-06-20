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
    case setRoomsRelayList(relayUrls: [String])

    // ── Follows ───────────────────────────────────────────────────────────────
    case follow(pubkey: String)
    case unfollow(pubkey: String)

    // ── Profiles (claim / release) ────────────────────────────────────────────
    case claimProfile(pubkey: String)
    case releaseProfile(pubkey: String)

    // ── Room discovery ────────────────────────────────────────────────────────
    case startRoomDiscovery(relayUrl: String)

    // ── Room actions ──────────────────────────────────────────────────────────
    case joinRoom(groupId: String, hostRelayUrl: String, inviteCode: String?)
    case createRoom(groupId: String, hostRelayUrl: String, name: String, about: String?)
    case addRoomMember(groupId: String, hostRelayUrl: String, pubkey: String, role: String?)
    case createRoomInvites(groupId: String, hostRelayUrl: String, codes: [String])
    case shareToRoom(groupId: String, hostRelayUrl: String, targetEventId: String, targetAuthorPubkey: String?, repost: Bool)

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

    // ── What's New ────────────────────────────────────────────────────────────
    case prepareWhatsNew
    case markWhatsNewSeen(shippedAtUnix: UInt64)

    // ── Highlight feed ────────────────────────────────────────────────────────
    case drainHighlightFeed
    case publishHighlight(content: String, sourceReference: String, relayHint: String?)

    // ── ISBN ──────────────────────────────────────────────────────────────────
    case lookupIsbn(isbn: String)

    // ── Share queue ───────────────────────────────────────────────────────────
    case drainShareQueue

    // ── Audio / podcast (Phase 5H) ────────────────────────────────────────────
    /// Load and play the episode identified by `guid`.  Kernel looks up the
    /// saved resume position and emits `CapabilityRequest::Audio(.load)`.
    case audioPlay(url: String, guid: String, artifactJson: String)
    /// Pause the currently-loaded player.
    case audioPause
    /// Seek to an absolute position (kernel clamps to `[0, duration]`).
    case audioSeek(seconds: Double)
    /// Explicitly persist the current resume position (call on app resign-active).
    case audioSetResume(seconds: Double)

    // ── Capture draft (Phase 5F) ──────────────────────────────────────────────────
    case captureSetQuote(quote: String)
    case captureSetContext(context: String)
    case captureSetNote(note: String)
    case captureSelectWord(wordIndex: UInt64)
    case captureClearSelection
    case captureSetTargetGroup(groupId: String)
    case captureClearTargetGroup
    case capturePublish
    case captureReset

    // ── Chat (Phase 7 cutover) ──────────────────────────────────────────────────
    /// Open a room's chat: wires the per-room ChatObserver (kernel is sole writer).
    case chatOpen(groupId: String, hostRelayUrl: String)
    /// Close a room's chat: releases the room buffer.
    case chatClose(groupId: String)
    /// Expand the loaded chat window by one page (bounded by the kernel).
    case chatLoadMore(groupId: String)
    /// Publish a kind:9 chat message into the room (optional reply parent).
    case postChat(groupId: String, hostRelayUrl: String, content: String, replyToEventId: String?)

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
        case .setRoomsRelayList(let relayUrls):
            return AppActionEnvelope(namespace: "hl.relay.set_rooms_relay_list",
                                     json: jsonArray(["relay_urls": relayUrls]))

        // ── Follows ───────────────────────────────────────────────────────────
        case .follow(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.follow",
                                     json: jsonObject(["pubkey": pubkey]))
        case .unfollow(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.unfollow",
                                     json: jsonObject(["pubkey": pubkey]))

        // ── Profiles ──────────────────────────────────────────────────────────
        case .claimProfile(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.claim",
                                     json: jsonObject(["pubkey": pubkey]))
        case .releaseProfile(let pubkey):
            return AppActionEnvelope(namespace: "hl.profile.release",
                                     json: jsonObject(["pubkey": pubkey]))

        // ── Room discovery ────────────────────────────────────────────────────
        case .startRoomDiscovery(let relayUrl):
            return AppActionEnvelope(namespace: "hl.room.start_discovery",
                                     json: jsonObject(["relay_url": relayUrl]))

        // ── Room actions ──────────────────────────────────────────────────────
        case .joinRoom(let groupId, let hostRelayUrl, let inviteCode):
            var dict: [String: Any] = ["group_id": groupId, "host_relay_url": hostRelayUrl]
            if let code = inviteCode { dict["invite_code"] = code }
            return AppActionEnvelope(namespace: "hl.room.join", json: jsonAny(dict))
        case .createRoom(let groupId, let hostRelayUrl, let name, let about):
            var dict: [String: Any] = ["group_id": groupId, "host_relay_url": hostRelayUrl, "name": name]
            if let about = about { dict["about"] = about }
            return AppActionEnvelope(namespace: "hl.room.create", json: jsonAny(dict))
        case .addRoomMember(let groupId, let hostRelayUrl, let pubkey, let role):
            var dict: [String: Any] = ["group_id": groupId, "host_relay_url": hostRelayUrl, "pubkey": pubkey]
            if let role = role { dict["role"] = role }
            return AppActionEnvelope(namespace: "hl.room.add_member", json: jsonAny(dict))
        case .createRoomInvites(let groupId, let hostRelayUrl, let codes):
            let dict: [String: Any] = ["group_id": groupId, "host_relay_url": hostRelayUrl, "codes": codes]
            return AppActionEnvelope(namespace: "hl.room.create_invites", json: jsonAny(dict))
        case .shareToRoom(let groupId, let hostRelayUrl, let targetEventId, let targetAuthorPubkey, let repost):
            var dict: [String: Any] = [
                "group_id": groupId,
                "host_relay_url": hostRelayUrl,
                "target_event_id": targetEventId,
                "repost": repost,
            ]
            if let author = targetAuthorPubkey { dict["target_author_pubkey"] = author }
            return AppActionEnvelope(namespace: "hl.room.share_to_room", json: jsonAny(dict))

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

        // ── What's New ────────────────────────────────────────────────────────
        case .prepareWhatsNew:
            return AppActionEnvelope(namespace: "hl.whats_new.prepare", json: "{}")
        case .markWhatsNewSeen(let shippedAtUnix):
            return AppActionEnvelope(namespace: "hl.whats_new.mark_seen",
                                     json: jsonObject(["shipped_at_unix": shippedAtUnix]))

        // ── Highlight feed ────────────────────────────────────────────────────
        case .drainHighlightFeed:
            return AppActionEnvelope(namespace: "hl.highlight.drain_feed", json: "{}")
        case .publishHighlight(let content, let sourceReference, let relayHint):
            var dict: [String: Any] = ["content": content, "source_reference": sourceReference]
            if let hint = relayHint { dict["relay_hint"] = hint }
            return AppActionEnvelope(namespace: "hl.highlight.publish", json: jsonAny(dict))

        // ── ISBN ──────────────────────────────────────────────────────────────
        case .lookupIsbn(let isbn):
            return AppActionEnvelope(namespace: "hl.isbn.lookup",
                                     json: jsonObject(["isbn": isbn]))

        // ── Share queue ───────────────────────────────────────────────────────
        case .drainShareQueue:
            return AppActionEnvelope(namespace: "hl.share.drain_queue", json: "{}")

        // ── Audio / podcast (Phase 5H) ────────────────────────────────────────
        case .audioPlay(let url, let guid, let artifactJson):
            return AppActionEnvelope(namespace: "hl.audio.play",
                                     json: jsonObject(["url": url, "guid": guid, "artifact_json": artifactJson]))
        case .audioPause:
            return AppActionEnvelope(namespace: "hl.audio.pause", json: "{}")
        case .audioSeek(let seconds):
            return AppActionEnvelope(namespace: "hl.audio.seek",
                                     json: jsonAny(["seconds": seconds]))
        case .audioSetResume(let seconds):
            return AppActionEnvelope(namespace: "hl.audio.set_resume",
                                     json: jsonAny(["seconds": seconds]))

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
        case .capturePublish:
            return AppActionEnvelope(namespace: "hl.capture.publish", json: "{}")
        case .captureReset:
            return AppActionEnvelope(namespace: "hl.capture.reset", json: "{}")

        // ── Chat (Phase 7 cutover) ──────────────────────────────────────────
        case .chatOpen(let groupId, let hostRelayUrl):
            return AppActionEnvelope(namespace: "hl.chat.open",
                                     json: jsonObject(["group_id": groupId, "host_relay_url": hostRelayUrl]))
        case .chatClose(let groupId):
            return AppActionEnvelope(namespace: "hl.chat.close",
                                     json: jsonObject(["group_id": groupId]))
        case .chatLoadMore(let groupId):
            return AppActionEnvelope(namespace: "hl.chat.load_more",
                                     json: jsonObject(["group_id": groupId]))
        case .postChat(let groupId, let hostRelayUrl, let content, let replyToEventId):
            var dict: [String: Any] = [
                "group_id": groupId,
                "host_relay_url": hostRelayUrl,
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
