// Swift-native replacements for bespoke Rust projection types removed in
// Phase 7 Part C. These were previously generated from UniFFI #[uniffi::Record]
// or #[uniffi::Enum] types; all usages construct or switch on them locally in
// Swift, so native Swift types are a direct drop-in with no Rust round-trip.

// MARK: - Keys / Identity

struct SecretKeySettingsSnapshot {
    let hasSecretKey: Bool
    let displayValue: String
    let copyValue: String?
}

struct PublicKeyDisplayProjection {
    let compactLabel: String
}

// MARK: - Comments

struct CommentComposerProjection {
    let submitBody: String
    let canSubmit: Bool
}

struct CommentToolbarProjection {
    let count: UInt32
    let showsCount: Bool
    let countLabel: String
    let accessibilityLabel: String
}

struct CommentActionChromeProjection {
    let showsFooter: Bool
    let footerSystemImage: String
    let footerIsAccented: Bool
    let showsFooterCount: Bool
    let footerCountLabel: String
    let likeTitle: String
    let likeSystemImage: String
    let bookmarkTitle: String
    let bookmarkSystemImage: String
}

struct CommentNodeChromeProjection {
    let replyCount: UInt32
    let showsReplyChevron: Bool
    let mostRecentReply: CommentThreadNode?
    let hasMoreReplies: Bool
    let moreRepliesLabel: String
    let isMostRecentAuthorReply: Bool
}

struct CommentThreadViewProjection {
    let focused: CommentThreadNode?
    let children: [CommentThreadNode]
    let navTitle: String
    let emptyStateLabel: String
    let composerPlaceholder: String
    let replyCountLabel: String
}

// MARK: - Communities

struct CommunityRowProjection {
    let displayName: String
    let pictureUrl: String?
    let subtitle: String?
}

struct RoomAvatarProjection {
    let pictureUrl: String
    let displayInitial: String
}

struct SearchCommunityRowProjection {
    let displayName: String
    let about: String?
    let visibilityLabel: String
    let accessLabel: String
    let memberCountLabel: String?
}

// MARK: - Bookmarks

enum BookmarkLibraryScope: Equatable, Hashable {
    case mine
    case explore
}

enum BookmarkLibraryFilter: Equatable, Hashable {
    case articles
    case collections
    case web
}

enum BookmarkLibraryPane: Equatable, Hashable {
    case articles
    case collections
    case web
    case explore
}

struct BookmarkLibraryScopeOptionProjection {
    let scope: BookmarkLibraryScope
    let label: String
}

struct BookmarkLibraryFilterChipProjection {
    let filter: BookmarkLibraryFilter
    let label: String
    let iconSystemName: String
}

struct BookmarkLibraryProjection {
    let scopeOptions: [BookmarkLibraryScopeOptionProjection]
    let filterChips: [BookmarkLibraryFilterChipProjection]
    let selectedPane: BookmarkLibraryPane
    let isEmpty: Bool
    let emptyIconSystemName: String
    let emptyTitle: String
    let emptyMessage: String
}

struct BookmarkedArticleRowProjection {
    let title: String
    let summary: String?
    let imageUrl: String?
    let displayUnixSeconds: UInt64?
}

struct BookmarkSetRowProjection {
    let displayTitle: String
    let kindLabel: String
    let kindIconSystemName: String
    let itemCountLabel: String?
}

struct WebBookmarkRowProjection {
    let displayTitle: String
    let host: String?
    let description: String?
    let displayUnixSeconds: UInt64?
}

// MARK: - Web Metadata

struct WebMetadata: Equatable {
    let url: String
    let title: String
    let description: String
    let image: String
    let siteName: String
    let author: String
    let favicon: String
    let fetchedAt: UInt64
}

// MARK: - ISBN lookup helpers
//
// findExistingBookForIsbn and buildEditedBookPreview were UniFFI-exported from
// the deleted isbn_lookup.rs. Re-implemented here as pure Swift so BookPicker
// keeps its early-exit ISBN dedup and manual-title flow without a Rust round-trip.

struct EditedBookPreviewProjection {
    let preview: ArtifactPreview?
    let error: String
}

func findExistingBookForIsbn(isbn: String, records: [ArtifactRecord]) -> ArtifactRecord? {
    guard let isbn13 = normalizeIsbn(isbn) else { return nil }
    let ref = "isbn:\(isbn13)"
    return records.first { r in
        let p = r.preview
        return p.catalogId == ref
            || p.referenceTagValue == ref
            || p.highlightTagValue == ref
            || p.highlightReferenceKey == "i:\(ref)"
    }
}

func buildEditedBookPreview(
    isbn: String,
    basePreview: ArtifactPreview?,
    title: String,
    author: String
) -> EditedBookPreviewProjection {
    guard let isbn13 = normalizeIsbn(isbn) else {
        return EditedBookPreviewProjection(preview: nil, error: "ISBN must be a valid Bookland ISBN-13 or ISBN-10")
    }
    let catalogId = "isbn:\(isbn13)"
    let highlightRefKey = "i:\(catalogId)"
    let stableId = "c\(String(format: "%x", isbnFnv1a("i:\(catalogId)")))"

    var preview = ArtifactPreview(
        id: stableId, url: "", title: title.trimmingCharacters(in: .whitespaces),
        author: author.trimmingCharacters(in: .whitespaces), image: "", description: "",
        source: "book", domain: "", catalogId: catalogId, catalogKind: "isbn",
        podcastGuid: "", podcastItemGuid: "", podcastShowTitle: "", audioUrl: "",
        audioPreviewUrl: "", transcriptUrl: "", feedUrl: "", publishedAt: "",
        durationSeconds: nil, referenceTagName: "i", referenceTagValue: catalogId,
        referenceKind: "isbn", highlightTagName: "i", highlightTagValue: catalogId,
        highlightReferenceKey: highlightRefKey, chapters: []
    )
    if let base = basePreview {
        if !base.id.trimmingCharacters(in: .whitespaces).isEmpty { preview.id = base.id }
        preview.url = base.url
        preview.image = base.image
        preview.description = base.description
        preview.domain = base.domain
        preview.publishedAt = base.publishedAt
    }
    return EditedBookPreviewProjection(preview: preview, error: "")
}

func normalizeIsbn(_ raw: String) -> String? {
    let digits = String(raw.filter { !$0.isWhitespace && $0 != "-" })

    if isbnIsValidBookland13(digits) {
        return digits
    }
    if isbnIsValid10(digits) {
        return isbn10To13(digits)
    }
    return nil
}

private func isbnIsAsciiDigit(_ c: Character) -> Bool {
    c.isASCII && c.isNumber
}

private func isbnIsValidBookland13(_ digits: String) -> Bool {
    guard digits.count == 13,
          digits.allSatisfy(isbnIsAsciiDigit),
          digits.hasPrefix("978") || digits.hasPrefix("979")
    else { return false }
    return isbnIsValid13Checksum(digits)
}

private func isbnIsValid13Checksum(_ digits: String) -> Bool {
    guard digits.count == 13 else { return false }
    var sum = 0
    for (i, c) in digits.enumerated() {
        guard isbnIsAsciiDigit(c), let d = c.wholeNumberValue else { return false }
        sum += i % 2 == 0 ? d : d * 3
    }
    return sum % 10 == 0
}

private func isbnIsValid10(_ digits: String) -> Bool {
    guard digits.count == 10 else { return false }
    var sum = 0
    for (i, c) in digits.enumerated() {
        let value: Int
        if (c == "X" || c == "x"), i == 9 {
            value = 10
        } else if isbnIsAsciiDigit(c), let d = c.wholeNumberValue {
            value = d
        } else {
            return false
        }
        sum += value * (10 - i)
    }
    return sum % 11 == 0
}

private func isbn10To13(_ isbn10: String) -> String {
    let prefix = "978" + String(isbn10.prefix(9))
    return prefix + String(isbnCompute13CheckDigit(prefix))
}

private func isbnCompute13CheckDigit(_ prefix12: String) -> Character {
    var sum = 0
    for (i, c) in prefix12.enumerated() {
        let d = c.wholeNumberValue ?? 0
        sum += i % 2 == 0 ? d : d * 3
    }
    let check = (10 - (sum % 10)) % 10
    return Character(String(check))
}

private func isbnFnv1a(_ s: String) -> UInt32 {
    let prime: UInt32 = 16777619
    var hash: UInt32 = 2166136261
    for byte in s.utf8 { hash = (hash ^ UInt32(byte)) &* prime }
    return hash
}

// MARK: - Articles

struct ArticleBookmarkChromeProjection {
    let toolbarSystemImage: String
    let usesAccentColor: Bool
    let accessibilityLabel: String
    let swipeTitle: String
    let menuTitle: String
    let actionSystemImage: String
}

struct ArticleProfileCardProjection {
    let title: String
    let titleIsFallback: Bool
    let displayUnixSeconds: UInt64?
    let hashtagSummary: String?
}

struct ArticleReaderHeaderProjection {
    let title: String
    let hashtagLabels: [String]
    let displayUnixSeconds: UInt64?
    let readTimeMinutes: UInt32?
}

// MARK: - BookPicker

struct BookPickerQueryProjection {
    let searchQuery: String
    let hasQuery: Bool
    let normalizedIsbn: String?
}

struct IsbnManualPreviewProjection {
    let title: String
    let author: String
    let canUse: Bool
}

// MARK: - Capture

struct CaptureBookDisplayProjection {
    let displayTitle: String
    let author: String?
    let imageUrl: String?
}

struct CaptureCommunitySelectionProjection {
    let displayName: String
    let hasSelection: Bool
}

// MARK: - Chat

struct ChatComposerProjection {
    let submitBody: String
    let canSend: Bool
}

struct ChatMessageRowProjection {
    let message: ChatMessageRecord
    let showHeader: Bool
    let replyToMessage: ChatMessageRecord?
}

// MARK: - Rooms

enum RoomVisibility {
    case `public`
    case `private`
}

enum RoomAccess {
    case open
    case closed
}

struct CreateRoomVisibilityOption {
    let id: String
    let title: String
    let summary: String
    let glyph: String
    let visibility: RoomVisibility
    let access: RoomAccess
    let isSelected: Bool
}

struct CreateRoomProjection {
    let canCreate: Bool
    let createName: String
    let createAbout: String
    let visibilityGlyph: String
    let visibilitySummary: String
    let visibilityOptions: [CreateRoomVisibilityOption]
}

struct RoomCoverCardProjection {
    let subtitle: String
}

// MARK: - Discussions

struct DiscussionAttachmentProjection {
    let label: String?
    let imageUrl: String?
    let author: String?
}

struct DiscussionComposerProjection {
    let submitTitle: String
    let submitBody: String
    let submitAttachmentUrl: String?
    let canPublish: Bool
}

// MARK: - NostrEntities

struct NostrEntityArticleCardProjection {
    let displayTitle: String
    let imageUrl: String?
    let summary: String?
    let readerRoute: ArticleReaderRoute?
}

// MARK: - Reading

struct ReadingFeedCardProjection {
    let displayTitle: String
    let titleIsFallback: Bool
    let imageUrl: String?
    let metaText: String?
    let showSocialSignal: Bool
    let visibleInteractorPubkeys: [String]
    let primaryInteractorPubkey: String?
    let socialText: String
    let relativeUnixSeconds: UInt64?
}

// MARK: - Room Invite

enum RoomInviteCandidateSource {
    case follow
    case paste
}

enum RoomInviteSelectionAction {
    case add
    case toggle
    case remove
}

struct RoomInviteCandidate {
    let pubkeyHex: String
    let source: RoomInviteCandidateSource
}

struct RoomInviteChip {
    let pubkeyHex: String
    let source: RoomInviteCandidateSource
    let displayName: String
}

struct RoomInviteSuggestion {
    let pubkeyHex: String
    let source: RoomInviteCandidateSource
    let secondaryLabel: String
    let displayName: String
    let isSelected: Bool
}

struct RoomInviteResolvedCandidate {
    let pubkeyHex: String
    let label: String
    let source: RoomInviteCandidateSource
    let displayName: String
    let isSelected: Bool
}

struct RoomInviteProjection {
    let selectedChips: [RoomInviteChip]
    let visibleFollows: [RoomInviteSuggestion]
    let resolvedCandidate: RoomInviteResolvedCandidate?
    let showEmptyFollowMessage: Bool
}

struct RoomInviteSnapshot {
    let projection: RoomInviteProjection
    let profilePubkeysToRequest: [String]
    let error: String
}

struct RoomInviteAvatarProjection {
    let pictureUrl: String
    let displayInitial: String
}
