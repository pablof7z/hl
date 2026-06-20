//! Transcript fetch and parse — Phase 5I.
//!
//! ## Responsibilities
//!
//! * **HTTP FETCH** — `fetch_and_parse(url)` fetches the transcript document
//!   (8 MiB cap, 20 s timeout) and parses it. Pure Rust — no native capability.
//!   DEVICE-LOCAL: fetched per session, never published to nostr.
//!
//! * **FORMAT DETECTION** — detects VTT / SRT / JSON from Content-Type header,
//!   file-extension hint, and content sniff (first 200 chars). Unknown format →
//!   empty segment list (D6).
//!
//! * **PARSERS** — three format parsers: `parse_vtt`, `parse_srt`, `parse_json`.
//!   Ported from the bespoke `app/core/src/podcast_transcript.rs` (live lane
//!   UNTOUCHED). The kernel module is the sole authoritative parser in the kernel
//!   lane; tests use the SAME fixture bytes so results are identical.
//!
//! ## Device-local vs nostr
//!
//! Everything produced here is DEVICE-LOCAL: transcript segments live in
//! `AppState::podcast.current.transcript_segments`. The ONLY nostr fact from
//! the podcast clip flow is the published kind:9802 event (Phase 5J).

use std::sync::OnceLock;
use std::time::Duration;

use ::url::Url;
use futures::StreamExt;
use regex::Regex;

use crate::errors::CoreError;
use crate::kernel::domains::podcast::TranscriptSegment;

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

// ─── Format detection ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Vtt,
    Srt,
    Json,
    Unknown,
}

fn detect_format(source: &str, content_type: Option<&str>, file_extension: Option<&str>) -> Format {
    let content_type = content_type.unwrap_or("").to_lowercase();
    let ext = file_extension.unwrap_or("").to_lowercase();

    if content_type.contains("json") || ext == "json" {
        return Format::Json;
    }
    if content_type.contains("vtt") || ext == "vtt" {
        return Format::Vtt;
    }
    if content_type.contains("srt") || ext == "srt" {
        return Format::Srt;
    }

    let sniff = source.chars().take(200).collect::<String>();
    let sniff = sniff.trim();
    if sniff.starts_with("WEBVTT") {
        return Format::Vtt;
    }
    if sniff.starts_with('[') || sniff.starts_with('{') {
        return Format::Json;
    }
    if sniff.contains("-->") {
        return Format::Srt;
    }
    Format::Unknown
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Fetch a transcript document from `url` and parse it into segments.
///
/// Returns `Err` only on fetch/network failure. A parse failure (unknown
/// format, malformed content) returns `Ok(Vec::new())` — the caller maps an
/// empty list to `TranscriptAvailability::Unavailable` (D6).
pub(crate) async fn fetch_and_parse(url: &str) -> Result<Vec<TranscriptSegment>, CoreError> {
    let url = validate_http_url(url)?;
    let client = http_client();
    let response = client
        .get(url.clone())
        .header(
            reqwest::header::ACCEPT,
            "text/vtt,application/x-subrip,application/json,text/plain;q=0.8,*/*;q=0.4",
        )
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("transcript fetch: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::Network(format!(
            "transcript fetch failed ({})",
            status.as_u16()
        )));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited(response, MAX_TRANSCRIPT_BYTES, "transcript").await?;
    let file_extension = url
        .path_segments()
        .and_then(|mut segs| segs.next_back())
        .and_then(|last| last.rsplit_once('.').map(|(_, ext)| ext.to_string()));

    Ok(parse_bytes(
        &bytes,
        content_type.as_deref(),
        file_extension.as_deref(),
    ))
}

/// Parse raw transcript bytes into segments.
///
/// Public for test injection — callers pass fixture bytes directly without
/// a real HTTP fetch. Mirrors the bespoke `parse_transcript_bytes` exactly
/// so the same fixture bytes produce identical output (parity gate).
pub(crate) fn parse_bytes(
    bytes: &[u8],
    content_type: Option<&str>,
    file_extension: Option<&str>,
) -> Vec<TranscriptSegment> {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    match detect_format(source, content_type, file_extension) {
        Format::Json => parse_json(source),
        Format::Vtt => parse_vtt(source),
        Format::Srt => parse_srt(source),
        Format::Unknown => Vec::new(),
    }
}

// ─── VTT parser ──────────────────────────────────────────────────────────────

fn parse_vtt(source: &str) -> Vec<TranscriptSegment> {
    let normalized = source.replace('\r', "");
    let mut body = normalized.as_str();
    if body.starts_with("WEBVTT") {
        body = body.find("\n\n").map(|idx| &body[idx + 2..]).unwrap_or("");
    }

    let mut segments = Vec::new();
    for block in body.split("\n\n") {
        let lines: Vec<String> = block
            .split('\n')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if lines.len() < 2 {
            continue;
        }
        let Some(time_idx) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = split_timecode(&lines[time_idx]) else {
            continue;
        };

        let raw_text = lines[time_idx + 1..].join("\n");
        let cleaned = strip_vtt_tags(&raw_text);
        if cleaned.is_empty() {
            continue;
        }

        let speaker = extract_vtt_speaker(&lines[time_idx + 1..].join(" "))
            .or_else(|| extract_speaker(&cleaned))
            .unwrap_or_default();
        let text = strip_speaker_prefix(&cleaned);

        segments.push(TranscriptSegment {
            id: format!("vtt-{}", segments.len()),
            start,
            end,
            speaker,
            text,
        });
    }
    segments
}

// ─── SRT parser ──────────────────────────────────────────────────────────────

fn parse_srt(source: &str) -> Vec<TranscriptSegment> {
    let normalized = source.replace('\r', "");
    let mut segments = Vec::new();

    for block in normalized.split("\n\n") {
        let lines: Vec<String> = block
            .split('\n')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if lines.len() < 2 {
            continue;
        }
        let Some(time_idx) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = split_timecode(&lines[time_idx]) else {
            continue;
        };

        let seq = lines
            .first()
            .filter(|line| line.parse::<u64>().is_ok())
            .cloned()
            .unwrap_or_else(|| segments.len().to_string());
        let raw_text = lines[time_idx + 1..].join("\n");
        let cleaned = normalize_whitespace(&raw_text);
        if cleaned.is_empty() {
            continue;
        }

        let speaker = extract_speaker(&cleaned).unwrap_or_default();
        let text = strip_speaker_prefix(&cleaned);

        segments.push(TranscriptSegment {
            id: format!("srt-{seq}"),
            start,
            end,
            speaker,
            text,
        });
    }
    segments
}

// ─── JSON parser ─────────────────────────────────────────────────────────────

fn parse_json(source: &str) -> Vec<TranscriptSegment> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    find_json_segments(&value)
}

fn find_json_segments(value: &serde_json::Value) -> Vec<TranscriptSegment> {
    match value {
        serde_json::Value::Array(items) => {
            let direct: Vec<TranscriptSegment> = items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| json_segment(item, idx))
                .collect();
            if !direct.is_empty() {
                return direct;
            }
            for item in items {
                let nested = find_json_segments(item);
                if !nested.is_empty() {
                    return nested;
                }
            }
            Vec::new()
        }
        serde_json::Value::Object(map) => {
            for key in ["segments", "results", "items", "captions", "transcript"] {
                if let Some(sub) = map.get(key) {
                    let nested = find_json_segments(sub);
                    if !nested.is_empty() {
                        return nested;
                    }
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn json_segment(value: &serde_json::Value, index: usize) -> Option<TranscriptSegment> {
    let dict = value.as_object()?;
    let text = first_string(dict, &["text", "value", "caption", "body"]);
    if text.is_empty() {
        return None;
    }

    let start = first_f64(dict, &["start", "startTime", "start_time", "offset"]).unwrap_or(0.0);
    let end = first_f64(dict, &["end", "endTime", "end_time"]).unwrap_or(start);
    let speaker = first_string(dict, &["speaker", "speakerName", "speaker_name"]);
    let id = first_string(dict, &["id"]);

    Some(TranscriptSegment {
        id: if id.is_empty() {
            format!("json-{index}")
        } else {
            id
        },
        start,
        end,
        speaker,
        text: normalize_whitespace(&text),
    })
}

// ─── Parsing utilities ────────────────────────────────────────────────────────

fn first_string(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    String::new()
}

fn first_f64(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let Some(value) = map.get(*key) else {
            continue;
        };
        if let Some(n) = value.as_f64() {
            return Some(n);
        }
        if let Some(s) = value.as_str().and_then(|s| s.parse::<f64>().ok()) {
            return Some(s);
        }
    }
    None
}

fn split_timecode(line: &str) -> Option<(f64, f64)> {
    let (start, end) = line.split_once("-->")?;
    Some((parse_timestamp(start.trim())?, parse_timestamp(end.trim())?))
}

fn parse_timestamp(raw: &str) -> Option<f64> {
    static TIMESTAMP_RE: OnceLock<Regex> = OnceLock::new();
    let re = TIMESTAMP_RE
        .get_or_init(|| Regex::new(r"(\d{1,2}):(\d{2})(?::(\d{2}))?(?:[.,](\d{1,3}))?").unwrap());
    let captures = re.captures(raw)?;
    let first = captures.get(1)?.as_str().parse::<f64>().ok()?;
    let second = captures.get(2)?.as_str().parse::<f64>().ok()?;
    let third = captures.get(3).and_then(|m| m.as_str().parse::<f64>().ok());
    let millis = captures
        .get(4)
        .and_then(|m| {
            let mut value = m.as_str().to_string();
            while value.len() < 3 {
                value.push('0');
            }
            value.parse::<f64>().ok()
        })
        .unwrap_or(0.0);

    if let Some(third) = third {
        Some(first * 3600.0 + second * 60.0 + third + millis / 1000.0)
    } else {
        Some(first * 60.0 + second + millis / 1000.0)
    }
}

fn extract_vtt_speaker(raw: &str) -> Option<String> {
    static VTT_SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = VTT_SPEAKER_RE.get_or_init(|| Regex::new(r"(?i)<v\s+([^>]+)>").unwrap());
    let speaker = re.captures(raw)?.get(1)?.as_str().trim();
    (!speaker.is_empty()).then(|| speaker.to_string())
}

fn strip_vtt_tags(raw: &str) -> String {
    static VTT_VOICE_RE: OnceLock<Regex> = OnceLock::new();
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let voice_re = VTT_VOICE_RE.get_or_init(|| Regex::new(r"(?is)<v[^>]*>(.*?)</v>").unwrap());
    let tag_re = TAG_RE.get_or_init(|| Regex::new(r"(?is)<[^>]+>").unwrap());
    let without_voice = voice_re.replace_all(raw, "$1");
    let without_tags = tag_re.replace_all(&without_voice, "");
    normalize_whitespace(&without_tags)
}

fn extract_speaker(raw: &str) -> Option<String> {
    static SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPEAKER_RE.get_or_init(|| {
        Regex::new(r"^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+").unwrap()
    });
    let speaker = re.captures(raw)?.get(1)?.as_str().trim();
    (!speaker.is_empty()).then(|| speaker.to_string())
}

fn strip_speaker_prefix(raw: &str) -> String {
    static SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPEAKER_RE.get_or_init(|| {
        Regex::new(r"^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+").unwrap()
    });
    re.replace(raw, "").to_string()
}

fn normalize_whitespace(raw: &str) -> String {
    static WHITESPACE_RE: OnceLock<Regex> = OnceLock::new();
    let re = WHITESPACE_RE.get_or_init(|| Regex::new(r"\s+").unwrap());
    re.replace_all(raw, " ")
        .trim_matches(|c: char| c.is_whitespace())
        .to_string()
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn validate_http_url(raw: &str) -> Result<Url, CoreError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput(
            "transcript URL must not be empty".into(),
        ));
    }
    let url = Url::parse(trimmed)
        .map_err(|e| CoreError::InvalidInput(format!("invalid transcript URL: {e}")))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(CoreError::InvalidInput(format!(
            "unsupported transcript URL scheme: {scheme}"
        ))),
    }
}

async fn read_limited(
    response: reqwest::Response,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, CoreError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes as u64 {
            return Err(CoreError::Network(format!(
                "{label} is too large ({length} bytes)"
            )));
        }
    }
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| CoreError::Network(format!("{label} read: {e}")))?;
        if out.len().saturating_add(chunk.len()) > max_bytes {
            return Err(CoreError::Network(format!(
                "{label} exceeds {max_bytes} bytes"
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parity helper ────────────────────────────────────────────────────────
    //
    // Maps any segment type (kernel or live) to a comparable tuple so we can
    // assert_eq! across the two crate boundaries without unified types.
    fn seg_tuple(
        id: &str,
        start: f64,
        end: f64,
        speaker: &str,
        text: &str,
    ) -> (String, f64, f64, String, String) {
        (
            id.to_string(),
            start,
            end,
            speaker.to_string(),
            text.to_string(),
        )
    }

    fn kernel_to_tuples(segs: &[TranscriptSegment]) -> Vec<(String, f64, f64, String, String)> {
        segs.iter()
            .map(|s| seg_tuple(&s.id, s.start, s.end, &s.speaker, &s.text))
            .collect()
    }

    fn live_to_tuples(
        segs: &[crate::podcast_transcript::TranscriptSegment],
    ) -> Vec<(String, f64, f64, String, String)> {
        segs.iter()
            .map(|s| seg_tuple(&s.id, s.start, s.end, &s.speaker, &s.text))
            .collect()
    }

    // 5I-T1: transcript_parses_vtt_to_segments
    //
    // Calls BOTH the kernel parser and the live bespoke parser on the same
    // fixture bytes, then asserts they produce identical output (parity gate).
    // Also anchors the expected values directly so any single-side drift fails.
    #[test]
    fn transcript_parses_vtt_to_segments() {
        let bytes = br#"WEBVTT

00:00:01.000 --> 00:00:03.500
<v Alice>Welcome to the show.</v>

00:00:04.000 --> 00:00:05.000
Bob: Thanks.
"#;

        let kernel_segs = parse_bytes(bytes, Some("text/vtt"), None);
        let live_segs =
            crate::podcast_transcript::parse_transcript_bytes(bytes, Some("text/vtt"), None);

        // Parity: kernel and live must produce identical output.
        assert_eq!(
            kernel_to_tuples(&kernel_segs),
            live_to_tuples(&live_segs),
            "kernel VTT parser must match live parser output"
        );

        // Anchor expected values so any single-side drift also fails.
        assert_eq!(kernel_segs.len(), 2);
        assert_eq!(kernel_segs[0].id, "vtt-0");
        assert_eq!(kernel_segs[0].start, 1.0);
        assert_eq!(kernel_segs[0].end, 3.5);
        assert_eq!(kernel_segs[0].speaker, "Alice");
        assert_eq!(kernel_segs[0].text, "Welcome to the show.");
        assert_eq!(kernel_segs[1].speaker, "Bob");
        assert_eq!(kernel_segs[1].text, "Thanks.");
    }

    // 5I-T2: transcript_parses_srt_to_segments
    //
    // Calls BOTH parsers on the same SRT fixture and asserts identical output.
    #[test]
    fn transcript_parses_srt_to_segments() {
        let bytes = br#"12
00:01:02,250 --> 00:01:04,000
HOST: Segment text.
"#;

        let kernel_segs = parse_bytes(bytes, None, Some("srt"));
        let live_segs = crate::podcast_transcript::parse_transcript_bytes(bytes, None, Some("srt"));

        assert_eq!(
            kernel_to_tuples(&kernel_segs),
            live_to_tuples(&live_segs),
            "kernel SRT parser must match live parser output"
        );

        assert_eq!(kernel_segs.len(), 1);
        assert_eq!(kernel_segs[0].id, "srt-12");
        assert_eq!(kernel_segs[0].start, 62.25);
        assert_eq!(kernel_segs[0].end, 64.0);
        assert_eq!(kernel_segs[0].speaker, "HOST");
        assert_eq!(kernel_segs[0].text, "Segment text.");
    }

    // 5I-T3: transcript_parses_json_to_segments
    //
    // Calls BOTH parsers on the same JSON fixture and asserts identical output.
    #[test]
    fn transcript_parses_json_to_segments() {
        let bytes = br#"{
  "results": [
    {"id": "a", "startTime": "1.5", "end_time": 2, "speakerName": "Ada", "text": "hello\nworld"}
  ]
}"#;

        let kernel_segs = parse_bytes(bytes, Some("application/json"), None);
        let live_segs = crate::podcast_transcript::parse_transcript_bytes(
            bytes,
            Some("application/json"),
            None,
        );

        assert_eq!(
            kernel_to_tuples(&kernel_segs),
            live_to_tuples(&live_segs),
            "kernel JSON parser must match live parser output"
        );

        assert_eq!(kernel_segs.len(), 1);
        assert_eq!(kernel_segs[0].id, "a");
        assert_eq!(kernel_segs[0].start, 1.5);
        assert_eq!(kernel_segs[0].end, 2.0);
        assert_eq!(kernel_segs[0].speaker, "Ada");
        assert_eq!(kernel_segs[0].text, "hello world");
    }

    // 5I-T4: malformed_transcript_is_empty_no_op
    //
    // Non-UTF8 bytes → empty. Unknown format → empty. D6: no panic.
    #[test]
    fn malformed_transcript_is_empty_no_op() {
        assert!(parse_bytes(b"not a transcript", None, None).is_empty());
        assert!(parse_bytes(&[0xff, 0xfe], Some("text/vtt"), None).is_empty());
    }

    // 5I-T5: transcript_segments_raw_no_timestamp_formatting
    //
    // D1: segments carry raw f64 start/end, not "X:XX" strings.
    #[test]
    fn transcript_segments_raw_no_timestamp_formatting() {
        let bytes = br#"WEBVTT

00:00:01.000 --> 00:00:03.500
Hello.
"#;
        let segments = parse_bytes(bytes, Some("text/vtt"), None);
        assert_eq!(segments.len(), 1);
        // start/end are raw f64, not formatted strings
        assert_eq!(segments[0].start, 1.0_f64);
        assert_eq!(segments[0].end, 3.5_f64);
        // No "1:00" or "0:01" label present — the struct only has f64 fields.
        // (This is enforced by the type: TranscriptSegment has no String timestamp field.)
    }

    // 5I-T6: transcript_attaches_to_episode (reducer test)
    //
    // After dispatching hl.transcript.load (injected via TranscriptReady), segments
    // are present in AppState and the snapshot carries them.
    #[test]
    fn transcript_attaches_to_episode() {
        use crate::kernel::action::{AppAction, KernelEvent};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};
        use crate::models::{ArtifactPreview, Chapter};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = crate::models::ArtifactRecord {
            preview: ArtifactPreview {
                id: "pod-1".into(),
                url: "https://podcast.example/episode".into(),
                title: "Test Episode".into(),
                author: "Host".into(),
                image: "https://podcast.example/art.jpg".into(),
                description: String::new(),
                source: "podcast".into(),
                domain: "podcast.example".into(),
                catalog_id: "podcast:item:guid:ep-guid".into(),
                catalog_kind: "podcast:item:guid".into(),
                podcast_guid: "feed-guid".into(),
                podcast_item_guid: "ep-guid".into(),
                podcast_show_title: "Test Show".into(),
                audio_url: "https://cdn.example/ep.mp3".into(),
                audio_preview_url: String::new(),
                transcript_url: "https://example.com/ep.vtt".into(),
                feed_url: "https://podcast.example/feed.xml".into(),
                published_at: String::new(),
                duration_seconds: Some(3600),
                reference_tag_name: "i".into(),
                reference_tag_value: "podcast:item:guid:ep-guid".into(),
                reference_kind: "podcast:item:guid".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "podcast:item:guid:ep-guid".into(),
                highlight_reference_key: "i:podcast:item:guid:ep-guid".into(),
                chapters: Vec::<Chapter>::new(),
            },
            group_id: "group".into(),
            share_event_id: "share-1".into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: String::new(),
        };

        // Load an episode.
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        // Inject transcript segments directly (bypasses HTTP fetch).
        let segments = vec![
            TranscriptSegment {
                id: "vtt-0".into(),
                start: 1.0,
                end: 3.5,
                speaker: "Alice".into(),
                text: "Welcome.".into(),
            },
            TranscriptSegment {
                id: "vtt-1".into(),
                start: 4.0,
                end: 5.0,
                speaker: "Bob".into(),
                text: "Thanks.".into(),
            },
        ];
        reduce(
            &mut state,
            Cmd::Event(KernelEvent::TranscriptReady {
                segments: segments.clone(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        let ep = state.podcast.current.as_ref().unwrap();
        assert_eq!(ep.transcript_segments.len(), 2);
        assert_eq!(ep.transcript_segments[0].id, "vtt-0");
        assert_eq!(
            ep.transcript_availability,
            crate::kernel::domains::podcast::TranscriptAvailability::Available
        );

        // Snapshot carries raw segments.
        let snap = crate::kernel::domains::podcast::project_podcast_listening_snapshot(&state);
        if let Some(crate::kernel::snapshot::ViewSnapshot::PodcastListening(s)) = snap {
            assert_eq!(s.transcript_segments.len(), 2);
            assert_eq!(
                s.transcript_availability,
                crate::kernel::snapshot::KernelTranscriptAvailability::Available
            );
        } else {
            panic!("expected PodcastListening snapshot");
        }
    }

    // 5I-T7: clip_mark_in_and_out_update_selection
    #[test]
    fn clip_mark_in_and_out_update_selection() {
        use crate::kernel::action::{AppAction, AppActionEnvelope};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        // mark_in at 10.0
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_in".into(),
                json: serde_json::json!({"current_time": 10.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert_eq!(sel.clip_start_seconds, Some(10.0));
        assert_eq!(sel.clip_end_seconds, None);

        // mark_out at 20.0
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_out".into(),
                json: serde_json::json!({"current_time": 20.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert_eq!(sel.clip_start_seconds, Some(10.0));
        assert_eq!(sel.clip_end_seconds, Some(20.0));

        // mark_in at 25.0 → end (20.0) is before new start → end cleared
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_in".into(),
                json: serde_json::json!({"current_time": 25.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert_eq!(sel.clip_start_seconds, Some(25.0));
        assert_eq!(sel.clip_end_seconds, None);
    }

    // 5I-T8: clip_extend_segment_expands_bounds
    #[test]
    fn clip_extend_segment_expands_bounds() {
        use crate::kernel::action::{AppAction, AppActionEnvelope, KernelEvent};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        // Inject segments.
        reduce(
            &mut state,
            Cmd::Event(KernelEvent::TranscriptReady {
                segments: vec![
                    TranscriptSegment {
                        id: "seg-a".into(),
                        start: 5.0,
                        end: 10.0,
                        speaker: "Ada".into(),
                        text: "hello".into(),
                    },
                    TranscriptSegment {
                        id: "seg-b".into(),
                        start: 15.0,
                        end: 20.0,
                        speaker: "Bob".into(),
                        text: "world".into(),
                    },
                ],
            }),
            KClock::now_unix_seconds(&clk),
        );

        // Extend to seg-a.
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_extend_segment".into(),
                json: serde_json::json!({"segment_id": "seg-a"}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        // Extend to seg-b.
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_extend_segment".into(),
                json: serde_json::json!({"segment_id": "seg-b"}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert_eq!(sel.clip_start_seconds, Some(5.0));
        assert_eq!(sel.clip_end_seconds, Some(20.0));
        assert_eq!(sel.speaker, "Ada");
        assert_eq!(sel.selected_segment_ids, vec!["seg-a", "seg-b"]);
    }

    // 5I-T9: clip_clear_resets_selection
    #[test]
    fn clip_clear_resets_selection() {
        use crate::kernel::action::{AppAction, AppActionEnvelope};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_in".into(),
                json: serde_json::json!({"current_time": 10.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        assert!(state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .is_some());

        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_clear".into(),
                json: "{}".to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        assert!(state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .is_none());
    }

    // 5I-T10: transcript_fetch_device_local_not_published
    //
    // TranscriptReady must not emit any nostr publish effects (DEVICE-LOCAL rule).
    #[test]
    fn transcript_fetch_device_local_not_published() {
        use crate::kernel::action::{AppAction, KernelEvent};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};
        use crate::kernel::effect::Effect;

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        let effects = reduce(
            &mut state,
            Cmd::Event(KernelEvent::TranscriptReady {
                segments: vec![TranscriptSegment {
                    id: "s0".into(),
                    start: 0.0,
                    end: 1.0,
                    speaker: "Host".into(),
                    text: "Hello.".into(),
                }],
            }),
            KClock::now_unix_seconds(&clk),
        );

        let nostr: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. }
                        | Effect::PublishRoomsRelayList { .. }
                        | Effect::DispatchNip29Action { .. }
                        | Effect::DispatchFollowAction { .. }
                        | Effect::DispatchShareToRoom { .. }
                        | Effect::DispatchBookmarkAction { .. }
                        | Effect::DispatchReactAction { .. }
                )
            })
            .collect();
        assert!(
            nostr.is_empty(),
            "TranscriptReady MUST NOT emit nostr publish effects (device-local): {nostr:?}"
        );
    }

    // 5I-T11: clip_set_end_negative_is_clamped
    //
    // clip_set_end with a negative value and no start must clamp to 0, not store
    // a negative end time (D6 — kernel never stores invalid state).
    #[test]
    fn clip_set_end_negative_is_clamped() {
        use crate::kernel::action::{AppAction, AppActionEnvelope};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        // Set end to a negative value — must clamp to 0.
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_set_end".into(),
                json: serde_json::json!({"value": -5.0, "duration_seconds": 60.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert!(
            sel.clip_end_seconds.unwrap() >= 0.0,
            "clip_end_seconds must not be negative after clip_set_end(-5)"
        );
    }

    // 5I-T12: clip_times_clamped_to_duration
    //
    // clip_mark_in / clip_mark_out with negative values must be clamped to 0 (≥ 0 rule).
    #[test]
    fn clip_times_clamped_to_duration() {
        use crate::kernel::action::{AppAction, AppActionEnvelope};
        use crate::kernel::actor::{reduce, Cmd};
        use crate::kernel::app::AppState;
        use crate::kernel::clock::{Clock as KClock, ManualClock};

        let mut state = AppState::default();
        let clk = ManualClock::default();
        clk.set(1_000);

        let artifact = minimal_artifact("ep-guid", "https://cdn.example/ep.mp3");
        reduce(
            &mut state,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&artifact).unwrap(),
            }),
            KClock::now_unix_seconds(&clk),
        );

        // mark_in at negative time → clamped to 0.
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_in".into(),
                json: serde_json::json!({"current_time": -3.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert_eq!(
            sel.clip_start_seconds,
            Some(0.0),
            "clip_start from negative current_time must clamp to 0"
        );

        // mark_out at negative time → clamped to 0 (start stays at 0, end = 0, reversed rule fires).
        // Reset first.
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_clear".into(),
                json: "{}".into(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        reduce(
            &mut state,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.audio.clip_mark_out".into(),
                json: serde_json::json!({"current_time": -2.0}).to_string(),
            }),
            KClock::now_unix_seconds(&clk),
        );
        let sel = state
            .podcast
            .current
            .as_ref()
            .unwrap()
            .clip_selection
            .as_ref()
            .unwrap();
        assert!(
            sel.clip_end_seconds.unwrap() >= 0.0,
            "clip_end from negative current_time must clamp to 0"
        );
    }

    fn minimal_artifact(guid: &str, audio_url: &str) -> crate::models::ArtifactRecord {
        use crate::models::{ArtifactPreview, Chapter};
        crate::models::ArtifactRecord {
            preview: ArtifactPreview {
                id: "pod-1".into(),
                url: "https://podcast.example/episode".into(),
                title: "Test Episode".into(),
                author: "Host".into(),
                image: "https://podcast.example/art.jpg".into(),
                description: String::new(),
                source: "podcast".into(),
                domain: "podcast.example".into(),
                catalog_id: format!("podcast:item:guid:{guid}"),
                catalog_kind: "podcast:item:guid".into(),
                podcast_guid: "feed-guid".into(),
                podcast_item_guid: guid.into(),
                podcast_show_title: "Test Show".into(),
                audio_url: audio_url.into(),
                audio_preview_url: String::new(),
                transcript_url: "https://example.com/ep.vtt".into(),
                feed_url: "https://podcast.example/feed.xml".into(),
                published_at: String::new(),
                duration_seconds: Some(3600),
                reference_tag_name: "i".into(),
                reference_tag_value: format!("podcast:item:guid:{guid}"),
                reference_kind: "podcast:item:guid".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: format!("podcast:item:guid:{guid}"),
                highlight_reference_key: format!("i:podcast:item:guid:{guid}"),
                chapters: Vec::<Chapter>::new(),
            },
            group_id: "group".into(),
            share_event_id: "share-1".into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: String::new(),
        }
    }
}
