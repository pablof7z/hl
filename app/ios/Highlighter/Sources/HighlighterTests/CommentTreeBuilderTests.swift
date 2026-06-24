import Foundation
import Testing
@testable import Highlighter

/// Pure-logic coverage for `CommentTreeBuilder` (Phase 7 flat-rows API). The
/// kernel emits raw `[CommentRecordRow]` with NIP-22 `parent_tag_value` links;
/// Swift owns only the tree assembly. These tests pin that assembly: child
/// linking by parent event id, root selection (top-level OR orphan promotion so
/// nothing is dropped), and oldest-first ordering of siblings and roots by
/// `created_at`. Display chrome (reply counts, most-recent-reply) is kernel-owned
/// now (`CommentNodeChromeProjection`) and is not this builder's concern.
struct CommentTreeBuilderTests {

    // MARK: - Fixtures

    private static let rootValue = "root"

    /// A kind:1111 comment row. `parent == nil` makes it top-level (its
    /// `parentTagValue` equals the root value, matching the kernel's
    /// `is_top_level` rule); otherwise `parent` is the parent event id.
    private func row(_ id: String, parent: String? = nil, createdAt: UInt64) -> CommentRecordRow {
        CommentRecordRow(
            eventId: id,
            authorPubkey: "pk-\(id)",
            body: "body-\(id)",
            rootTagName: "E",
            rootTagValue: Self.rootValue,
            rootKind: "1",
            parentTagName: parent == nil ? "E" : "e",
            parentTagValue: parent ?? Self.rootValue,
            parentKind: "1",
            createdAt: createdAt,
            isTopLevel: parent == nil,
            likeCount: 0,
            viewerReacted: false,
            bookmarked: false
        )
    }

    // MARK: - Empty / trivial

    @Test func emptyRowsProduceEmptyForest() {
        #expect(CommentTreeBuilder.build(from: []).isEmpty)
    }

    @Test func singleTopLevelCommentHasNoChildren() {
        let forest = CommentTreeBuilder.build(from: [row("a", createdAt: 100)])
        #expect(forest.count == 1)
        #expect(forest[0].record.eventId == "a")
        #expect(forest[0].children.isEmpty)
    }

    // MARK: - Nesting

    @Test func buildsNestedThreeLevelThread() {
        // a (top) -> b (parent a) -> c (parent b)
        let forest = CommentTreeBuilder.build(from: [
            row("a", createdAt: 100),
            row("b", parent: "a", createdAt: 200),
            row("c", parent: "b", createdAt: 300),
        ])
        #expect(forest.count == 1)
        let a = forest[0]
        #expect(a.children.map(\.id) == ["b"])
        #expect(a.children[0].children.map(\.id) == ["c"])
    }

    @Test func siblingsAreSortedOldestFirstByCreatedAt() {
        // Kernel ordering is by created_at; input order must not leak through.
        let forest = CommentTreeBuilder.build(from: [
            row("a", createdAt: 100),
            row("c", parent: "a", createdAt: 300),
            row("b", parent: "a", createdAt: 200),
            row("d", parent: "a", createdAt: 400),
        ])
        #expect(forest[0].children.map(\.id) == ["b", "c", "d"])
    }

    @Test func rootsAreSortedOldestFirstByCreatedAt() {
        let forest = CommentTreeBuilder.build(from: [
            row("b", createdAt: 200),
            row("a", createdAt: 100),
        ])
        #expect(forest.map(\.id) == ["a", "b"])
    }

    // MARK: - Orphan promotion (nothing is dropped)

    @Test func orphanReplyWhoseParentIsMissingIsPromotedToRoot() {
        // "b" replies to a comment that isn't in the visible window. It must be
        // promoted to a root rather than silently dropped.
        let forest = CommentTreeBuilder.build(from: [
            row("a", createdAt: 100),
            row("b", parent: "ghost", createdAt: 200),
        ])
        #expect(forest.map(\.id) == ["a", "b"])
        #expect(forest.allSatisfy { $0.children.isEmpty })
    }
}
