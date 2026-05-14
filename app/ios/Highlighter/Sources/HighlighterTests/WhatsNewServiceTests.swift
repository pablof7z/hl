import XCTest
@testable import Highlighter

@MainActor
final class WhatsNewServiceTests: XCTestCase {

    // MARK: - Fixture

    private let fixtureJSON = #"""
    {
      "schema_version": 1,
      "entries": [
        {
          "shipped_at": "2026-05-10T22:00:00Z",
          "lines": ["Newest line"]
        },
        {
          "shipped_at": "2026-05-09T12:00:00Z",
          "lines": ["Middle line A", "Middle line B"]
        },
        {
          "shipped_at": "2026-05-08T08:00:00Z",
          "lines": ["Oldest line"]
        }
      ]
    }
    """#

    private func fixtureEntries() throws -> [WhatsNewEntry] {
        let data = Data(fixtureJSON.utf8)
        return try WhatsNewService.decode(data)
    }

    private func date(_ iso: String) -> Date {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f.date(from: iso)!
    }

    // MARK: - Decoding

    func testFixtureJSONDecodes() throws {
        let entries = try fixtureEntries()
        XCTAssertEqual(entries.count, 3)
        XCTAssertEqual(entries[0].lines, ["Newest line"])
        XCTAssertEqual(entries[1].lines.count, 2)
    }

    /// The bundled `whats-new.json` must remain well-formed — a parse failure
    /// would silently disable the sheet for every user.
    func testBundledChangelogParses() {
        let entries = WhatsNewService.loadEntries()
        XCTAssertFalse(entries.isEmpty, "Bundled whats-new.json must ship with at least one entry.")
        XCTAssertFalse(entries.contains { $0.lines.isEmpty }, "Every entry needs at least one line.")
        let timestamps = entries.map(\.shippedAt)
        XCTAssertEqual(Set(timestamps).count, timestamps.count, "Every entry needs a unique shipped_at timestamp.")
    }

    // MARK: - unseenEntries

    func testUnseenEntriesEmptyOnFreshInstall() throws {
        let entries = try fixtureEntries()
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: nil, entries: entries)
        XCTAssertTrue(unseen.isEmpty, "Fresh install: unseenEntries with nil marker must return empty.")
    }

    func testUnseenEntriesEmptyWhenMarkerIsNewest() throws {
        let entries = try fixtureEntries()
        let marker = date("2026-05-10T22:00:00Z")
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        XCTAssertTrue(unseen.isEmpty, "User has already seen the newest entry — nothing to show.")
    }

    func testUnseenEntriesReturnsNewerSliceWhenMarkerIsMiddle() throws {
        let entries = try fixtureEntries()
        let marker = date("2026-05-09T12:00:00Z")
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        XCTAssertEqual(unseen.map(\.lines), [["Newest line"]])
    }

    func testUnseenEntriesReturnsAllNewerWhenMarkerIsOldest() throws {
        let entries = try fixtureEntries()
        let marker = date("2026-05-08T08:00:00Z")
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        XCTAssertEqual(unseen.map(\.lines), [["Newest line"], ["Middle line A", "Middle line B"]])
    }

    func testUnseenEntriesEmptyWhenMarkerIsAfterEverything() throws {
        let entries = try fixtureEntries()
        let marker = date("2030-01-01T00:00:00Z")
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        XCTAssertTrue(unseen.isEmpty)
    }

    func testUnseenEntriesAreNewestFirst() throws {
        let entries = try fixtureEntries()
        let marker = date("2026-05-08T08:00:00Z")
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        let dates = unseen.map(\.shippedAt)
        XCTAssertEqual(dates, dates.sorted(by: >), "Unseen entries must be newest-first.")
    }

    // MARK: - seedIfNeeded

    func testSeedIfNeededSetsMarkerToNewestEntry() throws {
        let entries = try fixtureEntries()
        let key = WhatsNewService.lastSeenAtKey
        UserDefaults.standard.removeObject(forKey: key)
        defer { UserDefaults.standard.removeObject(forKey: key) }

        WhatsNewService.seedIfNeeded(entries: entries)

        let marker = WhatsNewService.lastSeenAt
        XCTAssertNotNil(marker)
        // After seeding, unseenEntries must be empty (nothing newer than newest).
        let unseen = WhatsNewService.unseenEntries(lastSeenAt: marker, entries: entries)
        XCTAssertTrue(unseen.isEmpty, "After seeding, no entries should be marked as unseen.")
    }

    func testSeedIfNeededIsIdempotent() throws {
        let entries = try fixtureEntries()
        let key = WhatsNewService.lastSeenAtKey
        let oldDate = date("2026-01-01T00:00:00Z")
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        UserDefaults.standard.set(f.string(from: oldDate), forKey: key)
        defer { UserDefaults.standard.removeObject(forKey: key) }

        WhatsNewService.seedIfNeeded(entries: entries)

        // Marker must not have changed — seedIfNeeded is a no-op when a marker exists.
        XCTAssertEqual(WhatsNewService.lastSeenAt, oldDate)
    }
}
