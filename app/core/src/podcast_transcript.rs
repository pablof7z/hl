//! Podcast transcript loading/parsing and podcast media byte fetches.
//!
//! Native shells own audio playback and platform media surfaces. The shared
//! core owns reusable transcript interpretation and bounded HTTP loading so
//! iOS and Android render the same segment model.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

use ::url::Url;
use futures::StreamExt;
use regex::Regex;

use crate::errors::CoreError;
use crate::models::{CommunitySummary, HighlightDraft};

const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, uniffi::Record)]
pub struct TranscriptSegment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastClipSelection {
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    pub speaker: String,
    pub selected_segment_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipComposerProjection {
    pub matching_segments: Vec<TranscriptSegment>,
    pub excerpt: String,
    pub speaker: String,
    pub duration_seconds: f64,
    pub has_transcript: bool,
    pub can_publish: bool,
    pub community_name: String,
    pub selected_segment_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipComposerInput {
    pub segments: Vec<TranscriptSegment>,
    pub transcript_available: bool,
    pub clip_start_seconds: f64,
    pub clip_end_seconds: f64,
    pub duration_seconds: f64,
    pub selected_group_id: Option<String>,
    pub joined_communities: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Vtt,
    Srt,
    Json,
    Unknown,
}

pub async fn fetch_transcript(url: &str) -> Result<Vec<TranscriptSegment>, CoreError> {
    let url = validate_http_url(url)?;
    let response = http_client()
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
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited(response, MAX_TRANSCRIPT_BYTES, "transcript").await?;
    let file_extension = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|last| last.rsplit_once('.').map(|(_, ext)| ext.to_string()));

    Ok(parse_transcript_bytes(
        &bytes,
        content_type.as_deref(),
        file_extension.as_deref(),
    ))
}

pub async fn download_artwork(url: &str) -> Result<Vec<u8>, CoreError> {
    let url = validate_http_url(url)?;
    let response = http_client()
        .get(url)
        .header(reqwest::header::ACCEPT, "image/*,*/*;q=0.3")
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("podcast artwork fetch: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::Network(format!(
            "podcast artwork fetch failed ({})",
            status.as_u16()
        )));
    }

    read_limited(response, MAX_ARTWORK_BYTES, "podcast artwork").await
}

pub fn parse_transcript_bytes(
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

/// Build the highlight draft for a podcast clip from the visible transcript
/// selection. Rust owns segment matching, chronological ordering, quote
/// assembly, and the protocol payload passed to highlight publishing.
pub fn clip_highlight_draft(
    segments: &[TranscriptSegment],
    selected_segment_ids: &[String],
    note: String,
    clip_start_seconds: Option<f64>,
    clip_end_seconds: Option<f64>,
    clip_speaker: String,
) -> HighlightDraft {
    let selected_ids: HashSet<&str> = selected_segment_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !id.is_empty())
        .collect();
    let mut selected: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|segment| selected_ids.contains(segment.id.as_str()))
        .collect();
    selected.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    HighlightDraft {
        quote: selected
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        context: String::new(),
        note,
        clip_start_seconds,
        clip_end_seconds,
        clip_speaker,
        clip_transcript_segment_ids: selected
            .iter()
            .map(|segment| segment.id.clone())
            .collect::<Vec<_>>(),
        image: None,
    }
}

pub fn clip_composer_projection(input: PodcastClipComposerInput) -> PodcastClipComposerProjection {
    let matching_segments = matching_clip_segments(
        &input.segments,
        input.transcript_available,
        input.clip_start_seconds,
        input.clip_end_seconds,
    );
    let excerpt = matching_segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let speaker = matching_segments
        .iter()
        .find(|segment| !segment.speaker.is_empty())
        .map(|segment| segment.speaker.clone())
        .unwrap_or_default();
    let selected_segment_ids = matching_segments
        .iter()
        .map(|segment| segment.id.clone())
        .collect::<Vec<_>>();

    PodcastClipComposerProjection {
        matching_segments,
        excerpt,
        speaker,
        duration_seconds: input.clip_end_seconds - input.clip_start_seconds,
        has_transcript: !selected_segment_ids.is_empty(),
        can_publish: input.clip_start_seconds.is_finite()
            && input.clip_end_seconds.is_finite()
            && input.duration_seconds.is_finite()
            && input.clip_start_seconds >= 0.0
            && input.clip_end_seconds <= input.duration_seconds
            && input.clip_start_seconds + 5.0 <= input.clip_end_seconds,
        community_name: input
            .selected_group_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| community_name_for_id(id, &input.joined_communities))
            .unwrap_or_default(),
        selected_segment_ids,
    }
}

pub fn clip_composer_highlight_draft(
    segments: &[TranscriptSegment],
    transcript_available: bool,
    context: String,
    clip_start_seconds: f64,
    clip_end_seconds: f64,
) -> HighlightDraft {
    let matching_segments = matching_clip_segments(
        segments,
        transcript_available,
        clip_start_seconds,
        clip_end_seconds,
    );

    HighlightDraft {
        quote: matching_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        context,
        note: String::new(),
        clip_start_seconds: Some(clip_start_seconds),
        clip_end_seconds: Some(clip_end_seconds),
        clip_speaker: matching_segments
            .iter()
            .find(|segment| !segment.speaker.is_empty())
            .map(|segment| segment.speaker.clone())
            .unwrap_or_default(),
        clip_transcript_segment_ids: matching_segments
            .iter()
            .map(|segment| segment.id.clone())
            .collect::<Vec<_>>(),
        image: None,
    }
}

fn matching_clip_segments(
    segments: &[TranscriptSegment],
    transcript_available: bool,
    clip_start_seconds: f64,
    clip_end_seconds: f64,
) -> Vec<TranscriptSegment> {
    if !transcript_available {
        return Vec::new();
    }

    segments
        .iter()
        .filter(|segment| segment.start < clip_end_seconds && segment.end > clip_start_seconds)
        .cloned()
        .collect()
}

fn community_name_for_id(id: &str, joined_communities: &[CommunitySummary]) -> String {
    joined_communities
        .iter()
        .find(|community| community.id == id)
        .map(|community| {
            if community.name.is_empty() {
                id.to_string()
            } else {
                community.name.clone()
            }
        })
        .unwrap_or_else(|| id.to_string())
}

pub fn clear_clip_selection() -> PodcastClipSelection {
    PodcastClipSelection {
        clip_start_seconds: None,
        clip_end_seconds: None,
        speaker: String::new(),
        selected_segment_ids: Vec::new(),
    }
}

pub fn mark_clip_in(selection: &PodcastClipSelection, current_time: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_start_seconds = Some(current_time);
    if next
        .clip_end_seconds
        .map(|end| end < current_time)
        .unwrap_or(false)
    {
        next.clip_end_seconds = None;
    }
    next
}

pub fn mark_clip_out(selection: &PodcastClipSelection, current_time: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_end_seconds = Some(current_time);
    if next
        .clip_start_seconds
        .map(|start| start > current_time)
        .unwrap_or(false)
    {
        next.clip_start_seconds = None;
    }
    next
}

pub fn extend_clip_to_segment(
    selection: &PodcastClipSelection,
    segment: &TranscriptSegment,
) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_start_seconds = Some(match next.clip_start_seconds {
        Some(start) => start.min(segment.start),
        None => segment.start,
    });
    next.clip_end_seconds = Some(match next.clip_end_seconds {
        Some(end) => end.max(segment.end),
        None => segment.end,
    });
    if !next.selected_segment_ids.iter().any(|id| id == &segment.id) {
        next.selected_segment_ids.push(segment.id.clone());
    }
    if next.speaker.is_empty() && !segment.speaker.is_empty() {
        next.speaker = segment.speaker.clone();
    }
    next
}

pub fn set_clip_start(selection: &PodcastClipSelection, value: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    let mut start = value.max(0.0);
    if let Some(end) = next.clip_end_seconds {
        start = start.min((end - 0.05).max(0.0));
    }
    next.clip_start_seconds = Some(start);
    next
}

pub fn set_clip_end(
    selection: &PodcastClipSelection,
    value: f64,
    duration_seconds: f64,
) -> PodcastClipSelection {
    let mut next = selection.clone();
    let mut end = if duration_seconds > 0.0 {
        value.min(duration_seconds)
    } else {
        value
    };
    if let Some(start) = next.clip_start_seconds {
        end = end.max(start + 0.05);
    }
    next.clip_end_seconds = Some(end);
    next
}

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
        return Err(CoreError::InvalidInput("URL must not be empty".into()));
    }
    let url =
        Url::parse(trimmed).map_err(|e| CoreError::InvalidInput(format!("invalid URL: {e}")))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(CoreError::InvalidInput(format!(
            "unsupported URL scheme: {scheme}"
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
                "{label} exceeds {} bytes",
                max_bytes
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
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

fn first_string(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|value| value.as_str()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vtt_with_voice_tags() {
        let bytes = br#"WEBVTT

00:00:01.000 --> 00:00:03.500
<v Alice>Welcome to the show.</v>

00:00:04.000 --> 00:00:05.000
Bob: Thanks.
"#;

        let segments = parse_transcript_bytes(bytes, Some("text/vtt"), None);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].id, "vtt-0");
        assert_eq!(segments[0].start, 1.0);
        assert_eq!(segments[0].end, 3.5);
        assert_eq!(segments[0].speaker, "Alice");
        assert_eq!(segments[0].text, "Welcome to the show.");
        assert_eq!(segments[1].speaker, "Bob");
        assert_eq!(segments[1].text, "Thanks.");
    }

    #[test]
    fn parses_srt_sequence_ids_and_comma_timestamps() {
        let bytes = br#"12
00:01:02,250 --> 00:01:04,000
HOST: Segment text.
"#;

        let segments = parse_transcript_bytes(bytes, None, Some("srt"));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "srt-12");
        assert_eq!(segments[0].start, 62.25);
        assert_eq!(segments[0].end, 64.0);
        assert_eq!(segments[0].speaker, "HOST");
        assert_eq!(segments[0].text, "Segment text.");
    }

    #[test]
    fn parses_nested_json_segments() {
        let bytes = br#"{
  "results": [
    {"id": "a", "startTime": "1.5", "end_time": 2, "speakerName": "Ada", "text": "hello\nworld"}
  ]
}"#;

        let segments = parse_transcript_bytes(bytes, Some("application/json"), None);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "a");
        assert_eq!(segments[0].start, 1.5);
        assert_eq!(segments[0].end, 2.0);
        assert_eq!(segments[0].speaker, "Ada");
        assert_eq!(segments[0].text, "hello world");
    }

    #[test]
    fn clip_highlight_draft_orders_selected_segments_and_builds_quote() {
        let segments = vec![
            segment("later", 20.0, "later line"),
            segment("earlier", 10.0, "earlier line"),
            segment("unused", 5.0, "unused line"),
        ];
        let selected = vec![
            "missing".to_string(),
            "later".to_string(),
            "earlier".to_string(),
        ];

        let draft = clip_highlight_draft(
            &segments,
            &selected,
            "note".into(),
            Some(9.0),
            Some(25.0),
            "Ada".into(),
        );

        assert_eq!(draft.quote, "earlier line later line");
        assert_eq!(draft.context, "");
        assert_eq!(draft.note, "note");
        assert_eq!(draft.clip_start_seconds, Some(9.0));
        assert_eq!(draft.clip_end_seconds, Some(25.0));
        assert_eq!(draft.clip_speaker, "Ada");
        assert_eq!(
            draft.clip_transcript_segment_ids,
            vec!["earlier".to_string(), "later".to_string()]
        );
        assert!(draft.image.is_none());
    }

    #[test]
    fn clip_highlight_draft_allows_empty_selection() {
        let draft = clip_highlight_draft(
            &[segment("a", 1.0, "hello")],
            &[],
            String::new(),
            None,
            None,
            String::new(),
        );

        assert_eq!(draft.quote, "");
        assert!(draft.clip_transcript_segment_ids.is_empty());
    }

    #[test]
    fn mark_clip_in_and_out_clear_reversed_bounds() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(30.0),
            clip_end_seconds: Some(10.0),
            speaker: "Ada".into(),
            selected_segment_ids: vec!["a".into()],
        };

        let marked_in = mark_clip_in(&selection, 20.0);
        assert_eq!(marked_in.clip_start_seconds, Some(20.0));
        assert_eq!(marked_in.clip_end_seconds, None);
        assert_eq!(marked_in.speaker, "Ada");
        assert_eq!(marked_in.selected_segment_ids, vec!["a".to_string()]);

        let marked_out = mark_clip_out(&selection, 20.0);
        assert_eq!(marked_out.clip_start_seconds, None);
        assert_eq!(marked_out.clip_end_seconds, Some(20.0));
    }

    #[test]
    fn extend_clip_to_segment_expands_bounds_dedupes_and_adopts_speaker() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(10.0),
            clip_end_seconds: Some(20.0),
            speaker: String::new(),
            selected_segment_ids: vec!["existing".into()],
        };
        let segment = TranscriptSegment {
            id: "existing".into(),
            start: 5.0,
            end: 30.0,
            speaker: "Ada".into(),
            text: "hello".into(),
        };

        let out = extend_clip_to_segment(&selection, &segment);

        assert_eq!(out.clip_start_seconds, Some(5.0));
        assert_eq!(out.clip_end_seconds, Some(30.0));
        assert_eq!(out.speaker, "Ada");
        assert_eq!(out.selected_segment_ids, vec!["existing".to_string()]);
    }

    #[test]
    fn set_clip_bounds_enforces_gap_and_duration() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(10.0),
            clip_end_seconds: Some(20.0),
            speaker: String::new(),
            selected_segment_ids: Vec::new(),
        };

        let start = set_clip_start(&selection, 30.0);
        assert_eq!(start.clip_start_seconds, Some(19.95));

        let end = set_clip_end(&selection, 30.0, 25.0);
        assert_eq!(end.clip_end_seconds, Some(25.0));

        let end_before_start = set_clip_end(&selection, 5.0, 25.0);
        assert_eq!(end_before_start.clip_end_seconds, Some(10.05));
    }

    #[test]
    fn clear_clip_selection_resets_everything() {
        assert_eq!(
            clear_clip_selection(),
            PodcastClipSelection {
                clip_start_seconds: None,
                clip_end_seconds: None,
                speaker: String::new(),
                selected_segment_ids: Vec::new()
            }
        );
    }

    #[test]
    fn clip_composer_projection_matches_segments_and_room_label() {
        let segments = vec![
            clip_segment("before", 0.0, 4.0, "", "before"),
            clip_segment("a", 4.0, 12.0, "", "alpha"),
            clip_segment("b", 11.0, 20.0, "Ada", "beta"),
            clip_segment("after", 20.0, 25.0, "Grace", "after"),
        ];

        let projection = clip_composer_projection(composer_input(
            segments,
            true,
            10.0,
            18.0,
            60.0,
            Some("room"),
            vec![community("room", "Room name")],
        ));

        assert_eq!(projection.duration_seconds, 8.0);
        assert_eq!(projection.excerpt, "alpha beta");
        assert_eq!(projection.speaker, "Ada");
        assert!(projection.has_transcript);
        assert!(projection.can_publish);
        assert_eq!(projection.community_name, "Room name");
        assert_eq!(
            projection.selected_segment_ids,
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            projection
                .matching_segments
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn clip_composer_projection_preserves_time_only_and_group_fallbacks() {
        let segments = vec![clip_segment("a", 4.0, 12.0, "Ada", "alpha")];

        let projection = clip_composer_projection(composer_input(
            segments.clone(),
            false,
            10.0,
            14.0,
            60.0,
            Some("missing-room"),
            vec![community("other", "")],
        ));

        assert_eq!(projection.excerpt, "");
        assert_eq!(projection.speaker, "");
        assert!(!projection.has_transcript);
        assert!(!projection.can_publish);
        assert_eq!(projection.community_name, "missing-room");
        assert!(projection.selected_segment_ids.is_empty());

        let personal = clip_composer_projection(composer_input(
            segments,
            true,
            0.0,
            6.0,
            60.0,
            None,
            Vec::new(),
        ));
        assert_eq!(personal.community_name, "");
    }

    #[test]
    fn clip_composer_draft_preserves_sheet_context_mapping() {
        let segments = vec![
            clip_segment("a", 4.0, 12.0, "", "alpha"),
            clip_segment("b", 11.0, 20.0, "Ada", "beta"),
        ];

        let draft = clip_composer_highlight_draft(&segments, true, "room note".into(), 10.0, 18.0);

        assert_eq!(draft.quote, "alpha beta");
        assert_eq!(draft.context, "room note");
        assert_eq!(draft.note, "");
        assert_eq!(draft.clip_start_seconds, Some(10.0));
        assert_eq!(draft.clip_end_seconds, Some(18.0));
        assert_eq!(draft.clip_speaker, "Ada");
        assert_eq!(
            draft.clip_transcript_segment_ids,
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(draft.image.is_none());
    }

    #[test]
    fn unknown_or_invalid_transcripts_are_empty() {
        assert!(parse_transcript_bytes(b"not a transcript", None, None).is_empty());
        assert!(parse_transcript_bytes(&[0xff, 0xfe], Some("text/vtt"), None).is_empty());
    }

    fn segment(id: &str, start: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start,
            end: start + 1.0,
            speaker: String::new(),
            text: text.into(),
        }
    }

    fn clip_segment(
        id: &str,
        start: f64,
        end: f64,
        speaker: &str,
        text: &str,
    ) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start,
            end,
            speaker: speaker.into(),
            text: text.into(),
        }
    }

    fn community(id: &str, name: &str) -> CommunitySummary {
        CommunitySummary {
            id: id.into(),
            name: name.into(),
            about: String::new(),
            picture: String::new(),
            access: "open".into(),
            visibility: "public".into(),
            admin_pubkeys: Vec::new(),
            member_count: None,
            relay_url: "wss://relay.example".into(),
            metadata_event_id: String::new(),
            created_at: Some(1),
        }
    }

    fn composer_input(
        segments: Vec<TranscriptSegment>,
        transcript_available: bool,
        clip_start_seconds: f64,
        clip_end_seconds: f64,
        duration_seconds: f64,
        selected_group_id: Option<&str>,
        joined_communities: Vec<CommunitySummary>,
    ) -> PodcastClipComposerInput {
        PodcastClipComposerInput {
            segments,
            transcript_available,
            clip_start_seconds,
            clip_end_seconds,
            duration_seconds,
            selected_group_id: selected_group_id.map(str::to_string),
            joined_communities,
        }
    }
}
