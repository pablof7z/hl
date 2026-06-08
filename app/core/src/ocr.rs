#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct OcrRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl OcrRect {
    fn min_x(self) -> f64 {
        self.x
    }

    fn min_y(self) -> f64 {
        self.y
    }

    fn max_x(self) -> f64 {
        self.x + self.w
    }

    fn max_y(self) -> f64 {
        self.y + self.h
    }

    fn mid_x(self) -> f64 {
        self.x + self.w / 2.0
    }

    fn mid_y(self) -> f64 {
        self.y + self.h / 2.0
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OcrWord {
    pub text: String,
    pub bbox: OcrRect,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OcrLine {
    pub text: String,
    pub bbox: OcrRect,
    pub confidence: f32,
    pub words: Vec<OcrWord>,
}

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
            .map(|line| line.bbox.min_x())
            .collect::<Vec<_>>();
        let rights = lines
            .iter()
            .map(|line| line.bbox.max_x())
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
        .map(|line| line.bbox.min_x())
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
            .filter(|line| line.bbox.min_x() < split)
            .cloned()
            .collect::<Vec<_>>();
        let right = lines
            .iter()
            .filter(|line| line.bbox.min_x() >= split)
            .cloned()
            .collect::<Vec<_>>();
        let column_threshold = lines.len() as f64 * 0.25;
        if left.len() as f64 >= column_threshold && right.len() as f64 >= column_threshold {
            let left_max_x = left
                .iter()
                .map(|line| line.bbox.max_x())
                .fold(0.0, f64::max);
            let right_min_x = right
                .iter()
                .map(|line| line.bbox.min_x())
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
        if (lhs.bbox.mid_y() - rhs.bbox.mid_y()).abs() < 0.006 {
            lhs.bbox.min_x().total_cmp(&rhs.bbox.min_x())
        } else {
            rhs.bbox.mid_y().total_cmp(&lhs.bbox.mid_y())
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
            let at_top = line.bbox.min_y() > 0.94;
            let at_bottom = line.bbox.max_y() < 0.06;
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
    let indent_ratio = (line.bbox.min_x() - stats.body_left_edge) / body_width;
    let width_ratio = line.bbox.w / stats.median_height;
    let centered_deviation = (line.bbox.mid_x() - stats.page_center_x).abs() / body_width;

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
    let pulled_right = (stats.body_right_edge - line.bbox.max_x()) / body_width > 0.06;
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
    let gap = prev.bbox.min_y() - curr.bbox.max_y();
    let gap_ratio = gap / stats.median_height.max(0.0001);
    let body_width = (stats.body_right_edge - stats.body_left_edge).max(0.0001);
    let indent_ratio = (curr.bbox.min_x() - stats.body_left_edge) / body_width;
    let prev_short_ratio = (stats.body_right_edge - prev.bbox.max_x()) / body_width;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLine {
        OcrLine {
            text: text.to_string(),
            bbox: OcrRect { x, y, w, h },
            confidence: 0.9,
            words: Vec::new(),
        }
    }

    #[test]
    fn reconstruct_markdown_soft_wraps_body_lines() {
        let markdown = reconstruct_markdown(&[
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
            markdown,
            "This is a paragraph with enough words to stay body and it wraps across the next recognized line.\n"
        );
    }

    #[test]
    fn reconstruct_markdown_fuses_soft_hyphenated_words() {
        let markdown = reconstruct_markdown(&[
            line(
                "A quietly written paragraph with enough ordinary words before a hyphen-",
                0.1,
                0.80,
                0.7,
                0.03,
            ),
            line("ated sentence.", 0.1, 0.765, 0.65, 0.03),
        ]);
        assert_eq!(
            markdown,
            "A quietly written paragraph with enough ordinary words before a hyphenated sentence.\n"
        );
    }

    #[test]
    fn reconstruct_markdown_emits_heading_and_list_items() {
        let markdown = reconstruct_markdown(&[
            line("CHAPTER ONE", 0.33, 0.86, 0.34, 0.05),
            line("1. First point", 0.1, 0.74, 0.5, 0.03),
            line("2. Second point", 0.1, 0.68, 0.5, 0.03),
        ]);
        assert_eq!(
            markdown,
            "# CHAPTER ONE\n\n1. First point\n1. Second point\n"
        );
    }

    #[test]
    fn reconstruct_markdown_strips_common_ocr_artifacts() {
        assert_eq!(normalize("  o\u{FB01}ce\u{200B}  "), "ofice");
    }

    #[test]
    fn reconstruct_markdown_reads_two_columns_left_then_right() {
        let markdown = reconstruct_markdown(&[
            line(
                "right column first body line with enough words",
                0.58,
                0.82,
                0.3,
                0.03,
            ),
            line(
                "left column first body line with enough words",
                0.10,
                0.82,
                0.3,
                0.03,
            ),
            line(
                "right column second body line with enough words",
                0.58,
                0.78,
                0.3,
                0.03,
            ),
            line(
                "left column second body line with enough words",
                0.10,
                0.78,
                0.3,
                0.03,
            ),
        ]);
        assert_eq!(
            markdown,
            "left column first body line with enough words left column second body line with enough words right column first body line with enough words right column second body line with enough words\n"
        );
    }
}
