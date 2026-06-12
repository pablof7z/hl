import Foundation

/// One node of a NIP-22 comment thread — a `CommentRecord` plus its
/// recursively built replies, sorted oldest-first within each level so
/// reading flows top-to-bottom chronologically.
struct CommentNode: Identifiable, Hashable {
    let record: CommentRecord
    var children: [CommentNode]

    var id: String { record.eventId }

    static func == (lhs: CommentNode, rhs: CommentNode) -> Bool {
        lhs.record.eventId == rhs.record.eventId
            && lhs.children.count == rhs.children.count
            && zip(lhs.children, rhs.children).allSatisfy { $0 == $1 }
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(record.eventId)
        hasher.combine(children.count)
    }
}

enum CommentTreeBuilder {
    /// Build a nested display forest from Rust-owned thread links. Rust owns
    /// NIP-22 parentage, orphan promotion, and ordering; Swift only assembles
    /// ids into view nodes.
    static func build(snapshot: HighlighterCommentsSnapshot) -> [CommentNode] {
        let recordsById = Dictionary(uniqueKeysWithValues: snapshot.records.map { ($0.eventId, $0) })
        let childrenById = Dictionary(uniqueKeysWithValues: snapshot.childLinks.map {
            ($0.eventId, $0.childEventIds)
        })

        func node(for id: String) -> CommentNode? {
            guard let record = recordsById[id] else { return nil }
            let children = (childrenById[id] ?? []).compactMap { node(for: $0) }
            return CommentNode(record: record, children: children)
        }

        return snapshot.topLevelEventIds.compactMap { node(for: $0) }
    }
}

extension CommentNode {
    /// Total comment count under this node, inclusive of self.
    var totalCount: Int {
        1 + children.reduce(0) { $0 + $1.totalCount }
    }

    /// Most-recent reply (chronologically last child) — used for the
    /// inline depth-1 preview in the sheet root list. `nil` when the
    /// node has no replies.
    var mostRecentReply: CommentNode? {
        children.last
    }
}
