//! OCR capture domain — Phase 5D (device-local, no nostr publish).
//!
//! ## Responsibilities
//!
//! * **READ** — project `ViewId::Capture` as `ViewSnapshot::Capture(KernelCaptureSnapshot)`
//!   from `AppState::ocr`. Raw fields only (D1): `image_handle`, `markdown`,
//!   `selectable_words`, `raw_lines`, `pending`.
//!
//! * **WRITE** — `hl.ocr.recognize` envelope dispatches a
//!   `CapabilityRequest::Ocr(OcrOp::RecognizeText)`, setting `pending = true`.
//!   When `CapabilityResult::Ocr(OcrResult::Lines(_))` arrives, the kernel
//!   runs `reconstruct_markdown` + `selectable_words` and updates `AppState::ocr`.
//!
//! ## No nostr publish
//!
//! OCR state is device-local (per `hl-app-state-vs-nostr-facts`). Tests verify
//! that no `Effect::Publish*` variants appear in the output.
//!
//! ## Ported logic
//!
//! All markdown-reconstruction and selectable-word logic is ported from the live
//! bespoke lane (`app/core/src/ocr.rs`). The live module is UNTOUCHED
//! (Non-Negotiable #6). Types come from `crate::ocr` (re-exported via
//! `crate::capabilities::ocr`). Helper geometry is implemented as crate-private
//! free functions here since the `crate::ocr::OcrRect` impl methods are private.

use crate::capabilities::ocr::{OcrLine, OcrRect, OcrResult, OcrWord};
use crate::capabilities::CapabilityRequest;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelCaptureSnapshot, ViewSnapshot};

// ─── AppState::ocr ────────────────────────────────────────────────────────────

/// OCR capture state — device-local, never published to Nostr.
///
/// `image_handle` is the temp-file path supplied to the capability; `markdown`
/// and `selectable_words` are the kernel-reconstructed outputs. `pending` is
/// `true` while a `VNRecognizeTextRequest` is in flight.
///
/// NOT cleared on Logout — the last captured image is per-device, not per-account.
/// The field is a small fixed-overhead struct (no unbounded list in flight) —
/// Non-Negotiable #7: `raw_lines` and `selectable_words` are bounded by the
/// image's text content, not by the event store.
#[derive(Debug, Clone, Default)]
pub struct OcrState {
    /// In-progress or last-completed OCR image handle (temp-file path).
    pub image_handle: Option<String>,
    /// Reconstructed markdown from the last completed OCR pass.
    pub markdown: String,
    /// Selectable words for drag-selection.
    pub selectable_words: Vec<OcrWord>,
    /// Raw OCR lines from the last completed OCR pass.
    pub raw_lines: Vec<OcrLine>,
    /// True while a VNRecognizeTextRequest is in flight.
    pub pending: bool,
}

// ─── Reducer (action envelope) ────────────────────────────────────────────────

/// Reduce `hl.ocr.recognize { image_handle }`:
///   1. Mark `ocr.pending = true`.
///   2. Record `ocr.image_handle`.
///   3. Emit `Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(...))`.
///
/// The round-trip completes when `CapabilityResult::Ocr(OcrResult::Lines(_))`
/// arrives via `provide_capability_result`.
pub(crate) fn reduce_action_ocr_recognize(
    state: &mut AppState,
    image_handle: String,
) -> Vec<Effect> {
    state.ocr.pending = true;
    state.ocr.image_handle = Some(image_handle.clone());
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(
        crate::capabilities::ocr::OcrOp::RecognizeText { image_handle },
    ))]
}

// ─── Reducer (capability result) ─────────────────────────────────────────────

/// Reduce `CapabilityResult::Ocr(OcrResult::Lines(lines))`:
///   1. Set `ocr.pending = false`.
///   2. Run `reconstruct_markdown` and `selectable_words` on the raw lines.
///   3. Store the results in `AppState::ocr`.
///
/// Device-local — never emits any publish effect.
pub(crate) fn reduce_event_ocr_result(state: &mut AppState, result: OcrResult) -> Vec<Effect> {
    state.ocr.pending = false;
    match result {
        OcrResult::Lines(lines) => {
            let markdown = reconstruct_markdown(&lines);
            let words = selectable_words(&lines);
            state.ocr.markdown = markdown;
            state.ocr.selectable_words = words;
            state.ocr.raw_lines = lines;
            vec![]
        }
        OcrResult::Error(msg) => {
            tracing::warn!(error = %msg, "OCR capability error — no-op (D6)");
            vec![]
        }
    }
}

/// Reduce `KernelEvent::OcrRecognitionComplete` (test-only injection path).
///
/// In the live path, state is updated by `reduce_event_ocr_result`. This arm
/// exists so tests can inject `KernelEvent::OcrRecognitionComplete` via
/// `Cmd::Event` without going through the capability round-trip. No state
/// change happens here because the state was already written when the capability
/// result was processed. (Same pattern as `KernelEvent::ShareQueueDrained`.)
pub(crate) fn reduce_event_ocr_recognition_complete(
    _state: &mut AppState,
    _image_handle: String,
    _markdown: String,
    _selectable_words: Vec<OcrWord>,
    _raw_lines: Vec<OcrLine>,
) -> Vec<Effect> {
    vec![]
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project `ViewId::Capture` from `AppState::ocr`.
///
/// D1: raw fields only — no formatted strings, no labels.
pub(crate) fn project_capture_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    Some(ViewSnapshot::Capture(KernelCaptureSnapshot {
        image_handle: state.ocr.image_handle.clone(),
        markdown: state.ocr.markdown.clone(),
        selectable_words: state.ocr.selectable_words.clone(),
        raw_lines: state.ocr.raw_lines.clone(),
        pending: state.ocr.pending,
    }))
}

// ─── Geometry helpers ─────────────────────────────────────────────────────────
//
// `crate::ocr::OcrRect` has private impl methods that are not accessible from
// this module. These free functions provide the same geometry without duplicating
// the impl block (which would cause method-ambiguity errors when both impls are
// in scope via the re-export).

#[inline]
fn rect_min_x(r: OcrRect) -> f64 {
    r.x
}
#[inline]
fn rect_min_y(r: OcrRect) -> f64 {
    r.y
}
#[inline]
fn rect_max_x(r: OcrRect) -> f64 {
    r.x + r.w
}
#[inline]
fn rect_max_y(r: OcrRect) -> f64 {
    r.y + r.h
}
#[inline]
fn rect_mid_x(r: OcrRect) -> f64 {
    r.x + r.w / 2.0
}
#[inline]
fn rect_mid_y(r: OcrRect) -> f64 {
    r.y + r.h / 2.0
}
// ─── Ported reconstruction logic ─────────────────────────────────────────────
//
// All logic below is ported from `app/core/src/ocr.rs` (the live bespoke lane).
// The live module is UNTOUCHED (Non-Negotiable #6). Private methods on OcrRect
// are replaced by the free-function geometry helpers above.

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockKind {
    Body,
    Heading { level: usize },
    ListItem { ordered: bool },
    BlockQuote,
}

struct Classified {
    kind: BlockKind,
    text: String,
}

struct PageStats {
    median_height: f64,
    body_left_edge: f64,
    body_right_edge: f64,
    page_center_x: f64,
}

impl PageStats {
    fn new(lines: &[OcrLine]) -> Self {
        let mut heights = lines.iter().map(|line| line.bbox.h).collect::<Vec<_>>();
        heights.sort_by(f64::total_cmp);
        let median_height = heights[heights.len() / 2];

        let lefts = lines
            .iter()
            .map(|line| rect_min_x(line.bbox))
            .collect::<Vec<_>>();
        let rights = lines
            .iter()
            .map(|line| rect_max_x(line.bbox))
            .collect::<Vec<_>>();
        let body_left_edge = mode_binned(&lefts, 0.05);
        let body_right_edge = mode_binned(&rights, 0.05);
        let page_center_x = (body_left_edge + body_right_edge) / 2.0;

        Self {
            median_height,
            body_left_edge,
            body_right_edge,
            page_center_x,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    SoftWrap,
    HardBreak,
}

/// Turns Vision's line-by-line output into structured markdown that preserves
/// paragraph flow, headings, lists, and block quotes. The native shell supplies
/// raw OCR observations; Rust owns the deterministic reconstruction policy.
pub fn reconstruct_markdown(lines: &[OcrLine]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let normalized = lines
        .iter()
        .filter_map(|line| {
            let text = normalize(&line.text);
            if text.is_empty() {
                None
            } else {
                Some(OcrLine {
                    text,
                    bbox: line.bbox,
                    confidence: line.confidence,
                    words: Vec::new(),
                })
            }
        })
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        return String::new();
    }

    let ordered = reading_order(&normalized);
    let stats = PageStats::new(&ordered);
    let trimmed = strip_running_headers_and_footers(&ordered, &stats);
    assemble_markdown(&trimmed, &stats)
}

/// Project OCR lines into the ordered word sequence used for drag selection.
pub fn selectable_words(lines: &[OcrLine]) -> Vec<OcrWord> {
    let mut sorted_lines = lines.to_vec();
    sorted_lines.sort_by(|lhs, rhs| {
        if (rect_mid_y(lhs.bbox) - rect_mid_y(rhs.bbox)).abs() < 0.006 {
            rect_min_x(lhs.bbox).total_cmp(&rect_min_x(rhs.bbox))
        } else {
            rect_mid_y(rhs.bbox).total_cmp(&rect_mid_y(lhs.bbox))
        }
    });

    sorted_lines
        .into_iter()
        .flat_map(|line| {
            let mut words = line
                .words
                .into_iter()
                .filter(|word| !word.text.trim().is_empty())
                .collect::<Vec<_>>();
            if words.is_empty() {
                vec![OcrWord {
                    text: line.text,
                    bbox: line.bbox,
                    confidence: line.confidence,
                }]
            } else {
                words.sort_by(|lhs, rhs| rect_min_x(lhs.bbox).total_cmp(&rect_min_x(rhs.bbox)));
                words
            }
        })
        .collect()
}

fn normalize(raw: &str) -> String {
    raw.replace('\u{FB00}', "ff")
        .replace('\u{FB01}', "fi")
        .replace('\u{FB02}', "fl")
        .replace('\u{FB03}', "ffi")
        .replace('\u{FB04}', "ffl")
        .replace('\u{200B}', "")
        .trim_matches(char::is_whitespace)
        .to_string()
}

fn reading_order(lines: &[OcrLine]) -> Vec<OcrLine> {
    let mut min_x = lines
        .iter()
        .map(|line| rect_min_x(line.bbox))
        .collect::<Vec<_>>();
    min_x.sort_by(f64::total_cmp);

    let (Some(lo), Some(hi)) = (min_x.first().copied(), min_x.last().copied()) else {
        return lines.to_vec();
    };
    let spread = hi - lo;

    if spread > 0.25 {
        let mut split = (lo + hi) / 2.0;
        for _ in 0..6 {
            let left = min_x
                .iter()
                .copied()
                .filter(|value| *value < split)
                .collect::<Vec<_>>();
            let right = min_x
                .iter()
                .copied()
                .filter(|value| *value >= split)
                .collect::<Vec<_>>();
            if left.is_empty() || right.is_empty() {
                break;
            }
            let left_mean = left.iter().sum::<f64>() / left.len() as f64;
            let right_mean = right.iter().sum::<f64>() / right.len() as f64;
            split = (left_mean + right_mean) / 2.0;
        }

        let left = lines
            .iter()
            .filter(|line| rect_min_x(line.bbox) < split)
            .cloned()
            .collect::<Vec<_>>();
        let right = lines
            .iter()
            .filter(|line| rect_min_x(line.bbox) >= split)
            .cloned()
            .collect::<Vec<_>>();
        let column_threshold = lines.len() as f64 * 0.25;
        if left.len() as f64 >= column_threshold && right.len() as f64 >= column_threshold {
            let left_max_x = left
                .iter()
                .map(|line| rect_max_x(line.bbox))
                .fold(0.0, f64::max);
            let right_min_x = right
                .iter()
                .map(|line| rect_min_x(line.bbox))
                .fold(1.0, f64::min);
            if left_max_x <= right_min_x + 0.02 {
                let mut ordered = sort_top_down(&left);
                ordered.extend(sort_top_down(&right));
                return ordered;
            }
        }
    }

    sort_top_down(lines)
}

fn sort_top_down(lines: &[OcrLine]) -> Vec<OcrLine> {
    let mut sorted = lines.to_vec();
    sorted.sort_by(|lhs, rhs| {
        if (rect_mid_y(lhs.bbox) - rect_mid_y(rhs.bbox)).abs() < 0.006 {
            rect_min_x(lhs.bbox).total_cmp(&rect_min_x(rhs.bbox))
        } else {
            rect_mid_y(rhs.bbox).total_cmp(&rect_mid_y(lhs.bbox))
        }
    });
    sorted
}

fn mode_binned(values: &[f64], bin_size: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut buckets = std::collections::BTreeMap::<i64, Vec<f64>>::new();
    for value in values {
        let bucket = (*value / bin_size) as i64;
        buckets.entry(bucket).or_default().push(*value);
    }

    buckets
        .values()
        .max_by_key(|bucket_values| bucket_values.len())
        .map(|bucket_values| bucket_values.iter().sum::<f64>() / bucket_values.len() as f64)
        .unwrap_or(0.0)
}

fn strip_running_headers_and_footers(lines: &[OcrLine], stats: &PageStats) -> Vec<OcrLine> {
    lines
        .iter()
        .filter(|line| {
            let at_top = rect_min_y(line.bbox) > 0.94;
            let at_bottom = rect_max_y(line.bbox) < 0.06;
            if !at_top && !at_bottom {
                return true;
            }

            let height_ratio = line.bbox.h / stats.median_height;
            if height_ratio > 1.2 {
                return true;
            }

            let trimmed = line.text.trim_matches(char::is_whitespace);
            if !trimmed.is_empty()
                && trimmed.len() <= 4
                && trimmed.chars().all(|c| c.is_ascii_digit())
            {
                return false;
            }
            trimmed.split_whitespace().count() > 5
        })
        .cloned()
        .collect()
}

fn assemble_markdown(lines: &[OcrLine], stats: &PageStats) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut current_block = String::new();
    let mut current_kind = BlockKind::Body;

    for (i, line) in lines.iter().enumerate() {
        let classified = classify(line, stats);

        if i == 0 {
            current_kind = classified.kind;
            current_block = classified.text;
            continue;
        }

        let prev = &lines[i - 1];
        let boundary = paragraph_boundary(prev, line, stats);

        if classified.kind != current_kind || boundary == Boundary::HardBreak {
            flush_block(&mut out, &mut current_block, &current_kind);
            current_kind = classified.kind;
            current_block = classified.text;
        } else {
            current_block = soft_join(&current_block, &classified.text);
        }
    }
    flush_block(&mut out, &mut current_block, &current_kind);

    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", out.trim_matches(char::is_whitespace))
}

fn flush_block(out: &mut String, current_block: &mut String, current_kind: &BlockKind) {
    let piece = current_block.trim_matches(char::is_whitespace);
    if piece.is_empty() {
        current_block.clear();
        return;
    }

    match current_kind {
        BlockKind::Heading { level } => {
            out.push_str(&"#".repeat(*level));
            out.push(' ');
            out.push_str(piece);
            out.push_str("\n\n");
        }
        BlockKind::ListItem { ordered } => {
            out.push_str(if *ordered { "1. " } else { "- " });
            out.push_str(piece);
            out.push('\n');
        }
        BlockKind::BlockQuote => {
            for para in piece.split("\n\n") {
                out.push_str("> ");
                out.push_str(&para.replace('\n', "\n> "));
                out.push_str("\n\n");
            }
        }
        BlockKind::Body => {
            out.push_str(piece);
            out.push_str("\n\n");
        }
    }
    current_block.clear();
}

fn classify(line: &OcrLine, stats: &PageStats) -> Classified {
    let text = line.text.as_str();
    let height_ratio = line.bbox.h / stats.median_height;
    let body_width = (stats.body_right_edge - stats.body_left_edge).max(0.0001);
    let indent_ratio = (rect_min_x(line.bbox) - stats.body_left_edge) / body_width;
    let width_ratio = line.bbox.w / stats.median_height;
    let centered_deviation = (rect_mid_x(line.bbox) - stats.page_center_x).abs() / body_width;

    if let Some((ordered, remainder)) = strip_list_marker(text) {
        return Classified {
            kind: BlockKind::ListItem { ordered },
            text: remainder,
        };
    }

    let mut heading_signals = 0;
    if height_ratio > 1.25 {
        heading_signals += 1;
    }
    if centered_deviation < 0.08 && width_ratio > 1.5 {
        heading_signals += 1;
    }
    let word_count = text.split_whitespace().count();
    let terminator = text.chars().last().is_some_and(|c| ".!?".contains(c));
    if word_count < 8 && !terminator {
        heading_signals += 1;
    }
    let uppercased = text
        .chars()
        .flat_map(char::to_uppercase)
        .collect::<String>();
    if text == uppercased && word_count > 0 && text.chars().any(char::is_alphabetic) {
        heading_signals += 1;
    }

    let is_drop_cap = width_ratio < 1.5 && text.chars().count() <= 2;
    if heading_signals >= 2 && !is_drop_cap {
        let level = if height_ratio > 1.55 { 1 } else { 2 };
        return Classified {
            kind: BlockKind::Heading { level },
            text: text.to_string(),
        };
    }

    let pulled_left = indent_ratio > 0.06;
    let pulled_right = (stats.body_right_edge - rect_max_x(line.bbox)) / body_width > 0.06;
    if pulled_left && pulled_right && height_ratio < 1.2 {
        return Classified {
            kind: BlockKind::BlockQuote,
            text: text.to_string(),
        };
    }

    Classified {
        kind: BlockKind::Body,
        text: text.to_string(),
    }
}

fn strip_list_marker(text: &str) -> Option<(bool, String)> {
    let trimmed = text.trim_start_matches(char::is_whitespace);
    if let Some(first) = trimmed.chars().next() {
        if matches!(first, '•' | '·' | '●' | '○' | '▪' | '◦' | '–' | '—') {
            let remainder = trimmed[first.len_utf8()..]
                .trim_matches(char::is_whitespace)
                .to_string();
            if remainder.chars().count() > 2 {
                return Some((false, remainder));
            }
        }
    }

    let mut chars = trimmed.char_indices();
    let mut digit_count = 0;
    let mut marker_end = None;
    for (idx, c) in &mut chars {
        if c.is_ascii_digit() && digit_count < 2 {
            digit_count += 1;
            continue;
        }
        if digit_count > 0 && matches!(c, '.' | ')') {
            marker_end = Some(idx + c.len_utf8());
        }
        break;
    }
    if let Some(end) = marker_end {
        let rest = &trimmed[end..];
        if rest.chars().next().is_some_and(char::is_whitespace) {
            let remainder = rest.trim_matches(char::is_whitespace).to_string();
            if !remainder.is_empty() {
                return Some((true, remainder));
            }
        }
    }

    if let Some(remainder) = trimmed.strip_prefix("- ") {
        if !remainder.is_empty() {
            return Some((false, remainder.to_string()));
        }
    }
    None
}

fn paragraph_boundary(prev: &OcrLine, curr: &OcrLine, stats: &PageStats) -> Boundary {
    let gap = rect_min_y(prev.bbox) - rect_max_y(curr.bbox);
    let gap_ratio = gap / stats.median_height.max(0.0001);
    let body_width = (stats.body_right_edge - stats.body_left_edge).max(0.0001);
    let indent_ratio = (rect_min_x(curr.bbox) - stats.body_left_edge) / body_width;
    let prev_short_ratio = (stats.body_right_edge - rect_max_x(prev.bbox)) / body_width;
    let prev_ends_terminal = prev
        .text
        .chars()
        .last()
        .is_some_and(|c| ".!?\"'".contains(c));

    if gap_ratio > 0.6 {
        return Boundary::HardBreak;
    }
    if indent_ratio > 0.04 && gap_ratio > 0.15 {
        return Boundary::HardBreak;
    }
    if prev_short_ratio > 0.12 && prev_ends_terminal && gap_ratio > 0.2 {
        return Boundary::HardBreak;
    }
    Boundary::SoftWrap
}

fn soft_join(left: &str, right: &str) -> String {
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() {
        return left.to_string();
    }

    if left.ends_with('-') || left.ends_with('\u{2010}') || left.ends_with('\u{2011}') {
        let without_hyphen = left
            .char_indices()
            .next_back()
            .map(|(idx, _)| &left[..idx])
            .unwrap_or(left);
        let left_lower = without_hyphen
            .chars()
            .last()
            .is_some_and(char::is_lowercase);
        let right_lower = right.chars().next().is_some_and(char::is_lowercase);
        if left_lower && right_lower {
            return format!("{without_hyphen}{right}");
        }
    }

    format!("{left} {right}")
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::ocr::{OcrLine, OcrOp, OcrRect, OcrResult, OcrWord};
    use crate::capabilities::CapabilityResult;
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::ViewSnapshot;
    use crate::kernel::view::{ViewId, ViewRoute};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn line(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLine {
        OcrLine {
            text: text.to_string(),
            bbox: OcrRect { x, y, w, h },
            confidence: 0.9,
            words: Vec::new(),
        }
    }

    // 5D-T1: dispatch hl.ocr.recognize emits OcrCapabilityRequest
    #[test]
    fn ocr_recognize_emits_ocr_capability_request() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(crate::kernel::action::AppActionEnvelope {
                namespace: "hl.ocr.recognize".to_string(),
                json: r#"{"image_handle": "/tmp/test-image.jpg"}"#.to_string(),
            }),
        );

        let ocr_requests: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(
                        OcrOp::RecognizeText { .. }
                    ))
                )
            })
            .collect();

        assert_eq!(
            ocr_requests.len(),
            1,
            "must emit exactly one EmitCapabilityRequest::Ocr; got: {effects:?}"
        );

        if let Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(OcrOp::RecognizeText {
            image_handle,
        })) = &ocr_requests[0]
        {
            assert_eq!(image_handle, "/tmp/test-image.jpg");
        }

        assert!(
            state.ocr.pending,
            "pending must be true after recognize dispatch"
        );
        assert_eq!(
            state.ocr.image_handle.as_deref(),
            Some("/tmp/test-image.jpg")
        );
    }

    // 5D-T2: injecting Lines result runs reconstruction
    #[test]
    fn ocr_result_runs_reconstruction() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Put it in pending state first.
        state.ocr.pending = true;
        state.ocr.image_handle = Some("/tmp/img.jpg".to_string());

        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![
                line(
                    "This is a paragraph with enough words to stay body",
                    0.1,
                    0.80,
                    0.7,
                    0.03,
                ),
                line(
                    "and it wraps across the next recognized line.",
                    0.1,
                    0.765,
                    0.65,
                    0.03,
                ),
            ]))),
        );

        assert!(!state.ocr.pending, "pending must be false after result");
        assert!(
            !state.ocr.markdown.is_empty(),
            "markdown must be non-empty after reconstruction"
        );
    }

    // 5D-T3: reconstruct_markdown output matches live lane fixtures
    #[test]
    fn reconstruct_markdown_matches_live() {
        // Fixture 1: soft-wrapping body lines (same fixture as live ocr.rs)
        let md = reconstruct_markdown(&[
            line(
                "This is a paragraph with enough words to stay body",
                0.1,
                0.80,
                0.7,
                0.03,
            ),
            line(
                "and it wraps across the next recognized line.",
                0.1,
                0.765,
                0.65,
                0.03,
            ),
        ]);
        assert_eq!(
            md,
            "This is a paragraph with enough words to stay body and it wraps across the next recognized line.\n"
        );

        // Fixture 2: heading + ordered list (same fixture as live ocr.rs)
        let md2 = reconstruct_markdown(&[
            line("CHAPTER ONE", 0.33, 0.86, 0.34, 0.05),
            line("1. First point", 0.1, 0.74, 0.5, 0.03),
            line("2. Second point", 0.1, 0.68, 0.5, 0.03),
        ]);
        assert_eq!(md2, "# CHAPTER ONE\n\n1. First point\n1. Second point\n");
    }

    // 5D-T4: ocr_snapshot_raw — ViewId::Capture projections
    #[test]
    fn ocr_snapshot_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Open the Capture view.
        step(
            &mut state,
            &clock,
            Cmd::OpenView(ViewId::Capture, ViewRoute::Capture),
        );

        // Put pending state and then inject OCR result.
        state.ocr.pending = true;
        state.ocr.image_handle = Some("/tmp/cap.jpg".to_string());

        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![line(
                "Hello world text here to be captured",
                0.1,
                0.8,
                0.7,
                0.03,
            )]))),
        );

        let snap = project_capture_snapshot(&state);
        assert!(snap.is_some(), "snapshot must be Some");

        if let Some(ViewSnapshot::Capture(s)) = snap {
            assert!(!s.pending, "pending must be false");
            assert!(!s.markdown.is_empty(), "markdown must be non-empty");
            assert_eq!(s.image_handle.as_deref(), Some("/tmp/cap.jpg"));
        } else {
            panic!("expected ViewSnapshot::Capture");
        }
    }

    // 5D-T5: empty observations — no-op, no panic
    #[test]
    fn malformed_empty_observations_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.ocr.pending = true;

        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![]))),
        );

        assert!(!state.ocr.pending, "pending must be false");
        assert_eq!(
            state.ocr.markdown, "",
            "markdown must be empty for no lines"
        );

        // No publish effects.
        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![]))),
        );
        let publish_effects: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. }
                        | Effect::DispatchFollowAction { .. }
                        | Effect::DispatchNip29Action { .. }
                        | Effect::DispatchShareToRoom { .. }
                        | Effect::DispatchBookmarkAction { .. }
                        | Effect::DispatchReactAction { .. }
                        | Effect::PublishRoomsRelayList { .. }
                )
            })
            .collect();
        assert!(
            publish_effects.is_empty(),
            "must not emit any publish effects (device-local)"
        );
    }

    // 5D-T6: OCR error is a no-op, no panic
    #[test]
    fn ocr_error_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.ocr.pending = true;

        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Error(
                "vision failed".to_string(),
            ))),
        );

        assert!(!state.ocr.pending, "pending must be false after error");
    }

    // Additional: OcrRecognitionComplete event injected via Cmd::Event is a no-op
    #[test]
    fn ocr_recognition_complete_event_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.ocr.markdown = "prior".to_string();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::OcrRecognitionComplete {
                image_handle: "/tmp/test.jpg".to_string(),
                markdown: "new markdown".to_string(),
                selectable_words: vec![],
                raw_lines: vec![],
            }),
        );

        // State must be unchanged (OcrRecognitionComplete is a no-op reducer arm).
        assert_eq!(
            state.ocr.markdown, "prior",
            "OcrRecognitionComplete via Cmd::Event must not overwrite state"
        );
    }
}
