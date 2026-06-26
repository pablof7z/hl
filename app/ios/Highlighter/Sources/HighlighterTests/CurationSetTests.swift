import Foundation
import Testing
@testable import Highlighter

// ── BookmarkSetRow.setCoordinate ──────────────────────────────────────────────

struct BookmarkSetCoordinateTests {

    private func makeRow(dTag: String, pubkey: String, kind: UInt32) -> BookmarkSetRow {
        BookmarkSetRow(
            dTag: dTag,
            pubkey: pubkey,
            kind: kind,
            title: nil,
            description: nil,
            image: nil,
            articleAddresses: [],
            noteIds: [],
            rRefs: [],
            topics: [],
            rawTags: [],
            content: "",
            createdAt: 1_000
        )
    }

    @Test func setCoordinateFormatsCorrectly() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001"
        let row = makeRow(dTag: "my-list", pubkey: pk, kind: 30004)
        #expect(row.setCoordinate == "30004:\(pk):my-list")
    }

    @Test func setCoordinateKind30003() {
        let pk = "bbbb000000000000000000000000000000000000000000000000000000000002"
        let row = makeRow(dTag: "bm-set", pubkey: pk, kind: 30003)
        #expect(row.setCoordinate == "30003:\(pk):bm-set")
    }

    @Test func setCoordinateDTagWithSpecialChars() {
        let pk = "cccc000000000000000000000000000000000000000000000000000000000003"
        let row = makeRow(dTag: "my-reading-list-1234567890", pubkey: pk, kind: 30004)
        #expect(row.setCoordinate == "30004:\(pk):my-reading-list-1234567890")
    }
}

// ── CurationSetShareUrlSnapshot ───────────────────────────────────────────────

struct CurationSetShareTests {

    private let validPubkey =
        "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"

    @Test func validCoordinateReturnsUrl() {
        let coordinate = "30004:\(validPubkey):my-reading-list"
        let result = curationSetShareUrlSnapshot(coordinate: coordinate)
        // URL must be non-empty and error must be empty on success
        #expect(!result.url.isEmpty, "url must be non-empty for a valid coordinate")
        #expect(result.error.isEmpty, "error must be empty on success")
        // URL must start with the canonical base
        #expect(
            result.url.hasPrefix("https://highlighter.com/a/naddr1"),
            "URL must use the canonical https://highlighter.com/a/ route with naddr bech32"
        )
    }

    @Test func validCoordinateContainsNaddr() {
        let coordinate = "30004:\(validPubkey):test-set"
        let result = curationSetShareUrlSnapshot(coordinate: coordinate)
        #expect(!result.url.isEmpty)
        // The naddr part must be extractable
        guard let base = URL(string: result.url),
              let naddrPath = base.pathComponents.last,
              naddrPath.hasPrefix("naddr1")
        else {
            Issue.record("URL must contain an naddr bech32 as the last path component")
            return
        }
        _ = naddrPath // present and valid
    }

    @Test func non30004KindReturnsError() {
        // kind:30003 (bookmark set) must be rejected
        let coordinate = "30003:\(validPubkey):my-set"
        let result = curationSetShareUrlSnapshot(coordinate: coordinate)
        #expect(result.url.isEmpty, "url must be empty for a non-30004 coordinate")
        #expect(!result.error.isEmpty, "error must be non-empty for a non-30004 coordinate")
    }

    @Test func articleKindReturnsError() {
        // kind:30023 (article) must be rejected
        let coordinate = "30023:\(validPubkey):my-article"
        let result = curationSetShareUrlSnapshot(coordinate: coordinate)
        #expect(result.url.isEmpty)
        #expect(!result.error.isEmpty)
    }

    @Test func malformedCoordinateReturnsError() {
        let result = curationSetShareUrlSnapshot(coordinate: "not-a-coordinate")
        #expect(result.url.isEmpty)
        #expect(!result.error.isEmpty)
    }

    @Test func badPubkeyReturnsError() {
        let result = curationSetShareUrlSnapshot(coordinate: "30004:bad-pubkey:my-set")
        #expect(result.url.isEmpty)
        #expect(!result.error.isEmpty)
    }

    @Test func emptyDTagReturnsError() {
        let result = curationSetShareUrlSnapshot(coordinate: "30004:\(validPubkey):")
        #expect(result.url.isEmpty)
        #expect(!result.error.isEmpty)
    }
}
