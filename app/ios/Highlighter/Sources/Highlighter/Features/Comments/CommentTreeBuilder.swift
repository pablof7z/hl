import Foundation

typealias CommentNode = CommentThreadNode

extension CommentThreadNode: Identifiable {
    public var id: String { record.eventId }
}

/// Builds the display tree (`[CommentThreadNode]`) from the kernel's flat
/// `[CommentRecordRow]` (Phase 7). The kernel emits raw rows with NIP-22
/// `parent_tag_value` links only (D1); Swift owns the tree construction.
///
/// Linking rule (NIP-22 §3): a comment's parent is the kind:1111 event whose
/// `event_id == parent_tag_value`. Top-level comments have
/// `parent_tag_value == root_tag_value` (`is_top_level == true`) and become the
/// tree roots. Replies whose parent is missing from the visible window are
/// promoted to roots so they are never dropped.
enum CommentTreeBuilder {
    /// Convert a kernel `CommentRecordRow` into the bespoke `CommentRecord`
    /// shape the existing comment views render (raw fields, 1:1 mapping).
    static func record(from row: CommentRecordRow) -> CommentRecord {
        CommentRecord(
            eventId: row.eventId,
            pubkey: row.authorPubkey,
            body: row.body,
            rootTagName: row.rootTagName,
            rootTagValue: row.rootTagValue,
            parentTagName: row.parentTagName,
            parentTagValue: row.parentTagValue,
            rootKind: row.rootKind,
            createdAt: row.createdAt
        )
    }

    /// Build the comment tree from flat kernel rows, oldest-first within each
    /// sibling group (stable: preserves the kernel's newest-first input order
    /// reversed so replies read top-to-bottom like the live lane did).
    static func build(from rows: [CommentRecordRow]) -> [CommentThreadNode] {
        guard !rows.isEmpty else { return [] }

        // Index children by their parent event id (parent_tag_value).
        var childrenByParent: [String: [CommentRecordRow]] = [:]
        var byId: [String: CommentRecordRow] = [:]
        for row in rows {
            byId[row.eventId] = row
        }
        for row in rows where !row.isTopLevel {
            childrenByParent[row.parentTagValue, default: []].append(row)
        }

        // Roots: top-level comments, plus orphan replies whose parent is not in
        // the visible window (promote so nothing is dropped).
        let roots = rows.filter { row in
            row.isTopLevel || byId[row.parentTagValue] == nil
        }

        func node(for row: CommentRecordRow) -> CommentThreadNode {
            let kids = (childrenByParent[row.eventId] ?? [])
                .sorted { $0.createdAt < $1.createdAt }
                .map { node(for: $0) }
            return CommentThreadNode(record: record(from: row), children: kids)
        }

        return roots
            .sorted { $0.createdAt < $1.createdAt }
            .map { node(for: $0) }
    }
}
