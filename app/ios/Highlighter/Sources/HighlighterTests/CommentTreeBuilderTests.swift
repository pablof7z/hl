import Foundation
import Testing
@testable import Highlighter

/// Pure-logic coverage for `CommentTreeBuilder`. Rust owns NIP-22 parentage and
/// ordering; Swift's job is only to assemble flat records + child-link lists
/// into a display forest. These tests pin that assembly: nesting, ordering as
/// given, orphan handling (a child id with no record is silently dropped), and
/// the derived helpers (`totalCount`, `mostRecentReply`).
struct CommentTreeBuilderTests {

    // MARK: - Fixtures

    private func record(_ id: String, body: String = "") -> CommentRecord {
        CommentRecord(
            eventId: id,
            pubkey: "pk-\(id)",
            body: body.isEmpty ? "body-\(id)" : body,
            rootTagName: "E",
            rootTagValue: "root",
            parentTagName: "e",
            parentTagValue: "root",
            rootKind: "1",
            createdAt: nil
        )
    }

    private func snapshot(
        records: [CommentRecord],
        topLevel: [String],
        childLinks: [(String, [String])]
    ) -> HighlighterCommentsSnapshot {
        HighlighterCommentsSnapshot(
            rootTagName: "E",
            rootTagValue: "root",
            rootKind: 1,
            records: records,
            recordCount: UInt64(records.count),
            topLevelEventIds: topLevel,
            childLinks: childLinks.map { HighlighterCommentChildLinks(eventId: $0.0, childEventIds: $0.1) },
            interactions: [],
            drafts: [],
            isLoading: false,
            errorMessage: nil,
            isPublishing: false,
            publishErrorMessage: nil,
            lastPublishedEventId: nil,
            interactionErrorMessage: nil
        )
    }

    // MARK: - Empty / trivial

    @Test func emptySnapshotProducesEmptyForest() {
        let forest = CommentTreeBuilder.build(snapshot: snapshot(records: [], topLevel: [], childLinks: []))
        #expect(forest.isEmpty)
    }

    @Test func singleTopLevelCommentHasNoChildren() {
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a")],
            topLevel: ["a"],
            childLinks: []
        ))
        #expect(forest.count == 1)
        #expect(forest[0].record.eventId == "a")
        #expect(forest[0].children.isEmpty)
        #expect(forest[0].totalCount == 1)
        #expect(forest[0].mostRecentReply == nil)
    }

    // MARK: - Nesting

    @Test func buildsNestedThreeLevelThread() {
        // a -> b -> c
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a"), record("b"), record("c")],
            topLevel: ["a"],
            childLinks: [("a", ["b"]), ("b", ["c"])]
        ))
        #expect(forest.count == 1)
        let a = forest[0]
        #expect(a.children.map(\.id) == ["b"])
        #expect(a.children[0].children.map(\.id) == ["c"])
        // totalCount is inclusive of self at every level.
        #expect(a.totalCount == 3)
        #expect(a.children[0].totalCount == 2)
    }

    @Test func preservesChildLinkOrderAsGiven() {
        // Swift must not reorder; ordering is Rust's responsibility.
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a"), record("b"), record("c"), record("d")],
            topLevel: ["a"],
            childLinks: [("a", ["c", "b", "d"])]
        ))
        #expect(forest[0].children.map(\.id) == ["c", "b", "d"])
        // mostRecentReply is the chronologically-last child (as ordered).
        #expect(forest[0].mostRecentReply?.id == "d")
    }

    @Test func multipleTopLevelCommentsKeepTheirOrder() {
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a"), record("b")],
            topLevel: ["b", "a"],
            childLinks: []
        ))
        #expect(forest.map(\.id) == ["b", "a"])
    }

    // MARK: - Malformed / dangling references

    @Test func danglingChildIdIsDroppedNotCrashed() {
        // "ghost" is referenced as a child but has no record — it must be
        // skipped via compactMap rather than producing a phantom node.
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a"), record("b")],
            topLevel: ["a"],
            childLinks: [("a", ["ghost", "b"])]
        ))
        #expect(forest[0].children.map(\.id) == ["b"])
        #expect(forest[0].totalCount == 2)
    }

    @Test func topLevelIdWithoutRecordIsDropped() {
        let forest = CommentTreeBuilder.build(snapshot: snapshot(
            records: [record("a")],
            topLevel: ["missing", "a"],
            childLinks: []
        ))
        #expect(forest.map(\.id) == ["a"])
    }
}
