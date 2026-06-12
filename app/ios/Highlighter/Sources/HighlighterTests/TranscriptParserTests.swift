import Foundation
import Testing
@testable import Highlighter

/// Pure-logic coverage for `TranscriptParser` — the best-effort VTT / SRT /
/// JSON transcript decoder. The parser's contract is "never throw, never crash,
/// return `[]` on anything it can't understand", so the edge cases (empty,
/// malformed, header-only, missing timecodes) matter as much as the happy path.
struct TranscriptParserTests {

    // MARK: - Helpers

    private func parse(_ string: String, contentType: String? = nil, ext: String? = nil) -> [TranscriptSegment] {
        TranscriptParser.parse(data: Data(string.utf8), contentType: contentType, fileExtension: ext)
    }

    // MARK: - Empty / malformed input

    @Test func emptyDataReturnsNoSegments() {
        #expect(TranscriptParser.parse(data: Data(), contentType: nil, fileExtension: nil).isEmpty)
    }

    @Test func garbageReturnsNoSegments() {
        #expect(parse("this is not a transcript at all").isEmpty)
    }

    @Test func vttHeaderOnlyReturnsNoSegments() {
        // A WEBVTT file with no cues must yield zero segments, not crash on the
        // missing body after the header strip.
        #expect(parse("WEBVTT\n\n").isEmpty)
        #expect(parse("WEBVTT").isEmpty)
    }

    @Test func blockWithoutTimecodeIsSkipped() {
        let srt = """
        1
        Some text with no arrow timing line
        """
        #expect(parse(srt, ext: "srt").isEmpty)
    }

    // MARK: - Format detection

    @Test func detectsVttBySniffingHeader() {
        let segments = parse("""
        WEBVTT

        00:00:01.000 --> 00:00:02.500
        Hello world
        """)
        #expect(segments.count == 1)
        #expect(segments.first?.text == "Hello world")
    }

    @Test func detectsJsonByContentTypeOverSniff() {
        // Leading "[" would also sniff as JSON, but content-type must win and
        // route through the JSON path regardless.
        let json = #"[{"start": 1.0, "end": 2.0, "text": "hi"}]"#
        let segments = parse(json, contentType: "application/json")
        #expect(segments.count == 1)
        #expect(segments.first?.start == 1.0)
    }

    // MARK: - VTT

    @Test func vttExtractsVoiceTagSpeakerAndStripsTags() {
        let segments = parse("""
        WEBVTT

        00:00:00.000 --> 00:00:03.000
        <v Alice>Good <b>morning</b></v>
        """)
        #expect(segments.count == 1)
        #expect(segments.first?.speaker == "Alice")
        #expect(segments.first?.text == "Good morning")
    }

    @Test func vttMultipleCuesPreserveOrderAndTiming() {
        let segments = parse("""
        WEBVTT

        00:00:00.000 --> 00:00:01.000
        First

        00:00:01.000 --> 00:00:02.000
        Second
        """)
        #expect(segments.map(\.text) == ["First", "Second"])
        #expect(segments[1].start == 1.0)
        #expect(segments[1].end == 2.0)
    }

    // MARK: - SRT

    @Test func srtParsesSequenceCommaTimecodesAndSpeaker() {
        let segments = parse("""
        1
        00:00:05,000 --> 00:00:08,000
        Bob: Let's begin.
        """, ext: "srt")
        #expect(segments.count == 1)
        let seg = segments.first
        #expect(seg?.start == 5.0)
        #expect(seg?.end == 8.0)
        #expect(seg?.speaker == "Bob")
        #expect(seg?.text == "Let's begin.")
        // SRT prefers the sequence number from the block for its id.
        #expect(seg?.id == "srt-1")
    }

    // MARK: - JSON shapes

    @Test func jsonFindsNestedSegmentsUnderKnownKey() {
        let json = #"""
        {"results": {"segments": [
            {"start": 0, "end": 1.5, "text": "alpha", "speaker": "Sam"},
            {"startTime": 1.5, "endTime": 3, "value": "beta"}
        ]}}
        """#
        let segments = parse(json, ext: "json")
        #expect(segments.count == 2)
        #expect(segments[0].speaker == "Sam")
        // Alternate key names (startTime / value) must still be picked up.
        #expect(segments[1].start == 1.5)
        #expect(segments[1].text == "beta")
    }

    @Test func jsonEntriesWithoutTextAreDropped() {
        let json = #"[{"start": 0, "end": 1}, {"start": 1, "end": 2, "text": "kept"}]"#
        let segments = parse(json, ext: "json")
        #expect(segments.count == 1)
        #expect(segments.first?.text == "kept")
    }

    // MARK: - Timestamp parsing

    @Test func parseTimestampHandlesHoursMinutesAndMillis() {
        #expect(TranscriptParser.parseTimestamp("01:02:03.500") == 3723.5)
    }

    @Test func parseTimestampHandlesMinuteOnlyForm() {
        // No hour group -> MM:SS interpretation.
        #expect(TranscriptParser.parseTimestamp("02:30") == 150)
    }

    @Test func parseTimestampHandlesCommaDecimal() {
        #expect(TranscriptParser.parseTimestamp("00:00:01,250") == 1.25)
    }

    @Test func parseTimestampRejectsGarbage() {
        #expect(TranscriptParser.parseTimestamp("not-a-time") == nil)
    }
}
