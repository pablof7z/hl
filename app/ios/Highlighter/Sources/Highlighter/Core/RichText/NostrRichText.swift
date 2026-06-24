import Kingfisher
import SwiftUI

/// Renders plain text that may contain `nostr:` URI mentions and event
/// references. Used by surfaces that don't run full markdown — profile
/// bios, room descriptions, chat messages, discussions. The article
/// reader (`MarkdownRenderer`) does its own pass and integrates the
/// inline components from this file directly.
///
/// Strategy:
///   1. Tokenise the input into a sequence of `.text("…")` runs and
///      `.entity(ref)` runs by scanning for `nostr:` URI prefixes.
///   2. Group consecutive runs into paragraphs split at event-ref
///      runs — mentions stay inline (concatenated into the surrounding
///      `Text`), event refs become block cards.
///   3. Each block renders the appropriate per-kind card (article,
///      note, highlight, profile-callout) by resolving the entity
///      against Rust's local projection and falling back to a backfill REQ
///      when it isn't there yet.
struct NostrRichText: View {
    let content: String
    /// Base font for plain text + inline mentions. Defaults to body.
    var font: Font = .body
    /// Tint applied to inline mention chips.
    var accent: Color = .highlighterAccent
    var ink: Color = .highlighterInkStrong
    var muted: Color = .highlighterInkMuted

    @Environment(HighlighterStore.self) private var appStore

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                switch block {
                case .paragraph(let runs):
                    paragraph(runs)
                case .eventRef(let ref):
                    NostrEntityCard(entity: ref)
                }
            }
        }
    }

    // MARK: - Paragraph rendering

    @ViewBuilder
    private func paragraph(_ runs: [Run]) -> some View {
        // Concatenate runs into a single Text so wrapping behaves like
        // a normal paragraph. Mentions render with the projected display
        // name when available.
        runs.reduce(Text(""), { acc, run in
            switch run {
            case .text(let s):
                let a = (try? AttributedString(
                    markdown: s,
                    options: AttributedString.MarkdownParsingOptions(
                        interpretedSyntax: .inlineOnlyPreservingWhitespace
                    )
                )) ?? AttributedString(s)
                return acc + Text(a)
            case .entity(let ref):
                guard case .profile(let pubkey, _) = ref else {
                    // Event refs at this layer are guaranteed to be the
                    // first run of an `eventRef` block via `blocks`,
                    // so this case is unreachable in paragraphs.
                    return acc
                }
                let label = mentionLabel(for: pubkey)
                return acc + Text("@\(label)")
                    .foregroundStyle(accent)
                    .font(font.weight(.medium))
            }
        })
        .font(font)
        .foregroundStyle(ink)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func mentionLabel(for pubkeyHex: String) -> String {
        let snapshot = appStore.profileSnapshots[pubkeyHex]
        let needsProfileRefresh = snapshot == nil
            || (snapshot?.displayName.isEmpty == true && snapshot?.name.isEmpty == true)
        if needsProfileRefresh {
            Task { await appStore.requestProfile(pubkeyHex: pubkeyHex) }
        }
        let name: String
        if let s = snapshot, !s.displayName.isEmpty { name = s.displayName }
        else if let s = snapshot, !s.name.isEmpty { name = s.name }
        else { name = String(pubkeyHex.prefix(8)) }
        return name
    }

    // MARK: - Tokenisation + blocking

    private var blocks: [Block] {
        let json = tokenizeNostrContent(content: content)
        guard let data = json.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ok = root["ok"] as? Bool, ok,
              let tree = root["tree"] as? [String: Any],
              let rawBlocks = tree["blocks"] as? [[String: Any]] else {
            return [.paragraph([.text(content)])]
        }

        var blocks: [Block] = []
        for rawBlock in rawBlocks {
            var currentRuns: [Run] = []
            if let inlines = rawBlock["inlines"] as? [[String: Any]] {
                for inline in inlines {
                    let type_ = inline["type"] as? String ?? ""
                    if type_ == "text", let text = inline["text"] as? String {
                        currentRuns.append(.text(text))
                    } else if type_ == "nostr_ref", let key = inline["key"] as? String {
                        if let ref = nostrEntityRef(fromKey: key) {
                            switch ref {
                            case .profile:
                                currentRuns.append(.entity(ref))
                            case .event, .address:
                                if !currentRuns.isEmpty {
                                    blocks.append(.paragraph(currentRuns))
                                    currentRuns.removeAll()
                                }
                                blocks.append(.eventRef(ref))
                            }
                        }
                    }
                }
            }
            if !currentRuns.isEmpty {
                blocks.append(.paragraph(currentRuns))
            }
        }
        return blocks
    }

    private func nostrEntityRef(fromKey key: String) -> NostrEntityRef? {
        if key.hasPrefix("p:") {
            let pubkey = String(key.dropFirst(2))
            return .profile(pubkeyHex: pubkey, relays: [])
        } else if key.hasPrefix("e:") {
            let eventId = String(key.dropFirst(2))
            return .event(eventIdHex: eventId, relays: [], authorHintHex: nil, kindHint: nil)
        } else if key.hasPrefix("a:") {
            let parts = key.dropFirst(2).split(separator: ":", maxSplits: 2).map(String.init)
            guard parts.count == 3, let kind = UInt32(parts[0]) else { return nil }
            return .address(kind: kind, pubkeyHex: parts[1], dTag: parts[2], relays: [])
        }
        return nil
    }

    // MARK: - Run / Block models

    private enum Run {
        case text(String)
        case entity(NostrEntityRef)
    }

    private enum Block {
        case paragraph([Run])
        case eventRef(NostrEntityRef)
    }
}

// MARK: - Card

@MainActor
final class NostrEntityCardStore {
    @ObservationIgnored let entity: NostrEntityRef
    @ObservationIgnored let kernel: HighlighterAppKernel

    init(entity: NostrEntityRef, kernel: HighlighterAppKernel) {
        self.entity = entity
        self.kernel = kernel
    }

    func start() {
        kernel.resolveEntityRef(key: entityKey)
    }

    func stop() {
        kernel.releaseEntityRef(key: entityKey)
    }

    var entityKey: String {
        switch entity {
        case .profile(let pubkeyHex, _): return "p:\(pubkeyHex)"
        case .event(let eventIdHex, _, _, _): return "e:\(eventIdHex)"
        case .address(let kind, let pubkeyHex, let dTag, _): return "a:\(kind):\(pubkeyHex):\(dTag)"
        }
    }
}

/// Block-level card for `nevent1…` / `naddr1…` references. Resolves
/// via the kernel entity projection. Per-entity rendering is selected by kind.
struct NostrEntityCard: View {
    let entity: NostrEntityRef

    @Environment(HighlighterStore.self) private var appStore
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store: NostrEntityCardStore?

    var body: some View {
        Group {
            if let snapshot = kernel.entitySnapshots[entityIdentityKey],
               let event = nostrEntityEvent(from: snapshot) {
                resolvedCard(event)
            } else {
                placeholder
            }
        }
        .task(id: entityIdentityKey) { start() }
        .onDisappear { stop() }
    }

    private var entityIdentityKey: String {
        switch entity {
        case .profile(let pubkeyHex, _): return "p:\(pubkeyHex)"
        case .event(let eventIdHex, _, _, _): return "e:\(eventIdHex)"
        case .address(let kind, let pubkeyHex, let dTag, _): return "a:\(kind):\(pubkeyHex):\(dTag)"
        }
    }

    private func nostrEntityEvent(from snapshot: KernelEntitySnapshot) -> NostrEntityEvent? {
        let renderKind: NostrEntityRenderKind
        switch snapshot.kind {
        case 0: renderKind = .profile
        case 1: renderKind = .note
        case 9802: renderKind = .highlight
        case 30023: renderKind = .article
        default: renderKind = .generic
        }
        return NostrEntityEvent(
            eventIdHex: snapshot.key,
            kind: snapshot.kind,
            renderKind: renderKind,
            pubkeyHex: snapshot.pubkeyHex,
            content: snapshot.content,
            createdAt: snapshot.createdAt,
            tagsJson: snapshot.tagsJson
        )
    }

    @ViewBuilder
    private func resolvedCard(_ event: NostrEntityEvent) -> some View {
        switch event.renderKind {
        case .article: ArticleEntityCard(event: event)
        case .note: NoteEntityCard(event: event)
        case .highlight: HighlightEntityCard(event: event)
        case .profile: ProfileCalloutCard(event: event)
        case .generic: GenericEntityCard(event: event)
        }
    }

    private var placeholder: some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            Text(entityLabel)
                .font(.caption)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 0)
        }
        .padding(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private var entityLabel: String {
        switch entity {
        case .profile(let pubkeyHex, _):
            return "Profile · \(pubkeyHex.prefix(12))…"
        case .event(let eventIdHex, _, _, let kindHint):
            if let kind = kindHint {
                return "Event kind \(kind) · \(eventIdHex.prefix(12))…"
            } else {
                return "Event · \(eventIdHex.prefix(12))…"
            }
        case .address(let kind, _, let dTag, _):
            return "Kind \(kind) · \(dTag)"
        }
    }

    private func start() {
        store?.stop()
        let next = NostrEntityCardStore(entity: entity, kernel: kernel)
        store = next
        next.start()
    }

    private func stop() {
        store?.stop()
        store = nil
    }
}

// MARK: - Entity cards

/// Long-form article. Compact magazine card.
private struct ArticleEntityCard: View {
    let event: NostrEntityEvent
    @Environment(HighlighterStore.self) private var appStore
    @State private var profile: ProfileMetadata?

    var body: some View {
        let projection = articleCardProjection
        let target = projection.readerRoute.map { ArticleReaderTarget(route: $0) }
        return Group {
            if let target {
                NavigationLink(value: target) {
                    cardContent(projection)
                }
            } else {
                cardContent(projection)
            }
        }
        .buttonStyle(.plain)
        .task {
            profile = appStore.profileSnapshots[event.pubkeyHex]
            if profile == nil {
                await appStore.requestProfile(pubkeyHex: event.pubkeyHex)
                profile = appStore.profileSnapshots[event.pubkeyHex]
            }
        }
    }

    private func cardContent(_ projection: NostrEntityArticleCardProjection) -> some View {
        let author = authorDisplay
        return HStack(alignment: .top, spacing: 12) {
            if let image = projection.imageUrl, let url = URL(string: image) {
                KFImage(url)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 88, height: 88)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color.highlighterTintPale)
                    .frame(width: 88, height: 88)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(projection.displayTitle)
                    .font(.system(.headline, design: .default))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                if let summary = projection.summary {
                    Text(summary)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
                HStack(spacing: 6) {
                    AuthorAvatar(
                        pubkey: event.pubkeyHex,
                        pictureURL: author.pictureUrl,
                        displayInitial: author.displayInitial.uppercased(),
                        size: 16,
                        ringWidth: 0
                    )
                    Text(author.displayName.uppercased())
                        .font(.caption2.weight(.bold))
                        .tracking(0.6)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
                .padding(.top, 2)
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private var articleCardProjection: NostrEntityArticleCardProjection {
        // D1: mirrors nostr_entities::article_card_projection
        let tags = (try? JSONDecoder().decode([[String]].self, from: Data(event.tagsJson.utf8))) ?? []
        func tagValue(_ name: String) -> String {
            tags.first { $0.first == name && $0.count > 1 }?[1] ?? ""
        }
        let dTag = tagValue("d")
        let title = tagValue("title")
        let pubkey = event.pubkeyHex.trimmingCharacters(in: .whitespaces)
        let dTagTrimmed = dTag.trimmingCharacters(in: .whitespaces)
        let readerRoute: ArticleReaderRoute? = (!pubkey.isEmpty && !dTagTrimmed.isEmpty)
            ? ArticleReaderRoute(address: "30023:\(pubkey):\(dTagTrimmed)", pubkey: pubkey, dTag: dTagTrimmed)
            : nil
        let imageRaw = tagValue("image")
        let summaryRaw = tagValue("summary")
        return NostrEntityArticleCardProjection(
            displayTitle: title.isEmpty ? "Untitled" : title,
            imageUrl: imageRaw.isEmpty ? nil : imageRaw,
            summary: summaryRaw.isEmpty ? nil : summaryRaw,
            readerRoute: readerRoute
        )
    }

    private var authorDisplay: ProfileDisplayProjection {
        {
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(event.pubkeyHex.prefix(8)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }
}

/// Short note. Tweet-like card with author header + content.
private struct NoteEntityCard: View {
    let event: NostrEntityEvent
    @Environment(HighlighterStore.self) private var appStore
    @State private var profile: ProfileMetadata?

    var body: some View {
        let author = authorDisplay
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                AuthorAvatar(
                    pubkey: event.pubkeyHex,
                    pictureURL: author.pictureUrl,
                    displayInitial: author.displayInitial,
                    size: 26,
                    ringWidth: 0
                )
                Text(author.displayName)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                Spacer(minLength: 0)
                Text(relativeDate(event.createdAt))
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            Text(event.content)
                .font(.body)
                .foregroundStyle(Color.highlighterInkStrong)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
        .task {
            profile = appStore.profileSnapshots[event.pubkeyHex]
            if profile == nil {
                await appStore.requestProfile(pubkeyHex: event.pubkeyHex)
                profile = appStore.profileSnapshots[event.pubkeyHex]
            }
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        {
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(event.pubkeyHex.prefix(8)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }
}

/// Highlight. Pull-quote treatment.
private struct HighlightEntityCard: View {
    let event: NostrEntityEvent
    @Environment(HighlighterStore.self) private var appStore
    @State private var profile: ProfileMetadata?

    var body: some View {
        let author = authorDisplay
        HStack(alignment: .top, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
                .frame(maxHeight: .infinity)
            VStack(alignment: .leading, spacing: 8) {
                Text(event.content)
                    .font(.system(.body, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                Text("— \(author.displayName)")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
        }
        .padding(.vertical, 10)
        .padding(.horizontal, 4)
        .task {
            profile = appStore.profileSnapshots[event.pubkeyHex]
            if profile == nil {
                await appStore.requestProfile(pubkeyHex: event.pubkeyHex)
                profile = appStore.profileSnapshots[event.pubkeyHex]
            }
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        {
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(event.pubkeyHex.prefix(8)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }
}

/// Profile metadata. Compact callout.
private struct ProfileCalloutCard: View {
    let event: NostrEntityEvent

    var body: some View {
        // The content is JSON; we let the upstream profileSnapshots supply
        // the parsed data via NavigationLink. Render the avatar +
        // name from the projection if present.
        ProfileCalloutFromSnapshot(pubkey: event.pubkeyHex)
    }
}

private struct ProfileCalloutFromSnapshot: View {
    let pubkey: String
    @Environment(HighlighterStore.self) private var appStore
    @State private var profile: ProfileMetadata?

    var body: some View {
        let display = profileDisplay
        NavigationLink(value: ProfileDestination.pubkey(pubkey)) {
            HStack(spacing: 10) {
                AuthorAvatar(
                    pubkey: pubkey,
                    pictureURL: display.pictureUrl,
                    displayInitial: display.displayInitial,
                    size: 36,
                    ringWidth: 0
                )
                VStack(alignment: .leading, spacing: 2) {
                    Text(display.displayName)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                    if let about = profile?.about, !about.isEmpty {
                        Text(about)
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(2)
                    }
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(12)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color.highlighterRule, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .task {
            profile = appStore.profileSnapshots[pubkey]
            if profile == nil {
                await appStore.requestProfile(pubkeyHex: pubkey)
                profile = appStore.profileSnapshots[pubkey]
            }
        }
    }

    private var profileDisplay: ProfileDisplayProjection {
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(pubkey.prefix(8)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }
}

/// Fallback: any other kind. Show the kind, content snippet, author.
private struct GenericEntityCard: View {
    let event: NostrEntityEvent
    @Environment(HighlighterStore.self) private var appStore

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("KIND \(event.kind)")
                .font(.caption2.weight(.semibold))
                .tracking(0.8)
                .foregroundStyle(Color.highlighterInkMuted)
            if !event.content.isEmpty {
                Text(event.content)
                    .font(.callout)
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(4)
            }
            Text("\(authorFallback)…")
                .font(.caption.monospaced())
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .padding(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private var authorFallback: String {
        String(event.pubkeyHex.prefix(12))
    }
}

// MARK: - Helpers

private func relativeDate(_ secondsSinceEpoch: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(secondsSinceEpoch))
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .abbreviated
    return formatter.localizedString(for: date, relativeTo: Date())
}
