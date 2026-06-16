#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct OcrRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl OcrRect {
    fn is_usable(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w > 0.0
            && self.h > 0.0
    }

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

    fn contains_point(self, x: f64, y: f64) -> bool {
        x >= self.min_x() && x <= self.max_x() && y >= self.min_y() && y <= self.max_y()
    }

    fn intersection(self, other: OcrRect) -> Option<OcrRect> {
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        let rect = OcrRect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        };
        rect.is_usable().then_some(rect)
    }

    fn standardized(self) -> OcrRect {
        let min_x = self.min_x().min(self.max_x());
        let min_y = self.min_y().min(self.max_y());
        let max_x = self.min_x().max(self.max_x());
        let max_y = self.min_y().max(self.max_y());
        OcrRect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }

    fn union(self, other: OcrRect) -> OcrRect {
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        OcrRect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }

    fn inset_by(self, dx: f64, dy: f64) -> OcrRect {
        OcrRect {
            x: self.x + dx,
            y: self.y + dy,
            w: self.w - 2.0 * dx,
            h: self.h - 2.0 * dy,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum OcrPageSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OcrPageDetection {
    pub page_rect: OcrRect,
    pub chosen_side: OcrPageSide,
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

/// Detect whether the OCR geometry looks like a two-page spread and return the
/// dominant page's normalized crop rectangle.
pub fn detect_active_page(lines: &[OcrLine]) -> Option<OcrPageDetection> {
    let usable = lines
        .iter()
        .filter(|line| line.bbox.is_usable() && line.bbox.w > 0.02 && line.bbox.h > 0.005)
        .collect::<Vec<_>>();
    if usable.len() < 8 {
        return None;
    }

    #[derive(Clone, Copy)]
    struct Split {
        left_max_x: f64,
        right_min_x: f64,
        gutter: f64,
    }

    let mut best_gap = 0.0;
    let mut best_split = None;
    for probe_step in 30..=70 {
        let probe = probe_step as f64 / 100.0;
        let left_lines = usable
            .iter()
            .copied()
            .filter(|line| line.bbox.max_x() < probe)
            .collect::<Vec<_>>();
        let right_lines = usable
            .iter()
            .copied()
            .filter(|line| line.bbox.min_x() > probe)
            .collect::<Vec<_>>();

        if left_lines.len() >= 4 && right_lines.len() >= 4 {
            let left_max_x = left_lines
                .iter()
                .map(|line| line.bbox.max_x())
                .fold(0.0, f64::max);
            let right_min_x = right_lines
                .iter()
                .map(|line| line.bbox.min_x())
                .fold(1.0, f64::min);
            let gap = right_min_x - left_max_x;
            if gap > best_gap {
                best_gap = gap;
                best_split = Some(Split {
                    left_max_x,
                    right_min_x,
                    gutter: (left_max_x + right_min_x) / 2.0,
                });
            }
        }
    }

    let split = best_split.filter(|_| best_gap >= 0.05)?;
    let left_lines = usable
        .iter()
        .copied()
        .filter(|line| line.bbox.mid_x() < split.gutter)
        .collect::<Vec<_>>();
    let right_lines = usable
        .iter()
        .copied()
        .filter(|line| line.bbox.mid_x() > split.gutter)
        .collect::<Vec<_>>();

    let left_area = left_lines
        .iter()
        .map(|line| line.bbox.w * line.bbox.h)
        .sum::<f64>();
    let right_area = right_lines
        .iter()
        .map(|line| line.bbox.w * line.bbox.h)
        .sum::<f64>();
    let chosen_is_right = right_area >= left_area;
    let chosen_lines = if chosen_is_right {
        &right_lines
    } else {
        &left_lines
    };
    if chosen_lines.len() < 4 {
        return None;
    }

    let chosen_min_x = chosen_lines
        .iter()
        .map(|line| line.bbox.min_x())
        .fold(1.0, f64::min);
    let chosen_max_x = chosen_lines
        .iter()
        .map(|line| line.bbox.max_x())
        .fold(0.0, f64::max);
    let chosen_min_y = chosen_lines
        .iter()
        .map(|line| line.bbox.min_y())
        .fold(1.0, f64::min);
    let chosen_max_y = chosen_lines
        .iter()
        .map(|line| line.bbox.max_y())
        .fold(0.0, f64::max);

    let outer_pad_x = 0.04;
    let gutter_pad_x = 0.015;
    let pad_y = 0.04;

    let (crop_min_x, crop_max_x) = if chosen_is_right {
        (
            (chosen_min_x.min(split.right_min_x) - gutter_pad_x).max(0.0),
            (chosen_max_x + outer_pad_x).min(1.0),
        )
    } else {
        (
            (chosen_min_x - outer_pad_x).max(0.0),
            (chosen_max_x.max(split.left_max_x) + gutter_pad_x).min(1.0),
        )
    };
    let crop_min_y = (chosen_min_y - pad_y).max(0.0);
    let crop_max_y = (chosen_max_y + pad_y).min(1.0);

    let page_rect = OcrRect {
        x: crop_min_x,
        y: crop_min_y,
        w: crop_max_x - crop_min_x,
        h: crop_max_y - crop_min_y,
    };
    if !(page_rect.w < 0.92 && page_rect.w > 0.20 && page_rect.h > 0.20) {
        return None;
    }

    Some(OcrPageDetection {
        page_rect,
        chosen_side: if chosen_is_right {
            OcrPageSide::Right
        } else {
            OcrPageSide::Left
        },
    })
}

/// Re-project OCR lines into a cropped page rectangle. Native performs the
/// actual image crop; Rust keeps the OCR coordinate transformation consistent
/// across platforms.
pub fn crop_lines(lines: &[OcrLine], page_rect: OcrRect) -> Vec<OcrLine> {
    let page_w = page_rect.w;
    let page_h = page_rect.h;
    if page_w <= 0.0 || page_h <= 0.0 {
        return lines.to_vec();
    }

    let unit = OcrRect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    lines
        .iter()
        .filter_map(|line| {
            if !page_rect.contains_point(line.bbox.mid_x(), line.bbox.mid_y()) {
                return None;
            }

            let bbox = OcrRect {
                x: (line.bbox.min_x() - page_rect.min_x()) / page_w,
                y: (line.bbox.min_y() - page_rect.min_y()) / page_h,
                w: line.bbox.w / page_w,
                h: line.bbox.h / page_h,
            }
            .intersection(unit)?;

            let words = line
                .words
                .iter()
                .filter_map(|word| {
                    let bbox = OcrRect {
                        x: (word.bbox.min_x() - page_rect.min_x()) / page_w,
                        y: (word.bbox.min_y() - page_rect.min_y()) / page_h,
                        w: word.bbox.w / page_w,
                        h: word.bbox.h / page_h,
                    }
                    .intersection(unit)?;
                    Some(OcrWord {
                        text: word.text.clone(),
                        bbox,
                        confidence: word.confidence,
                    })
                })
                .collect::<Vec<_>>();

            Some(OcrLine {
                text: line.text.clone(),
                bbox,
                confidence: line.confidence,
                words,
            })
        })
        .collect()
}

/// Compute the normalized image crop around a selected OCR passage.
pub fn default_highlight_crop_box(
    highlight_boxes: &[OcrRect],
    image_width: f64,
    image_height: f64,
    margin_fraction: f64,
) -> Option<OcrRect> {
    let selected_bounds = highlight_boxes
        .iter()
        .copied()
        .filter(|rect| rect.is_usable())
        .reduce(OcrRect::union)?;

    let safe_margin_fraction = if margin_fraction.is_finite() {
        margin_fraction.max(0.0)
    } else {
        0.08
    };
    let safe_width = image_width.max(1.0);
    let safe_height = image_height.max(1.0);
    let margin_x = safe_margin_fraction.max(48.0 / safe_width);
    let margin_y = safe_margin_fraction
        .max(selected_bounds.h * 0.55)
        .max(48.0 / safe_height);

    selected_bounds
        .inset_by(-margin_x, -margin_y)
        .intersection(unit_rect())
}

/// Clamp a user-edited highlight crop rectangle back into the normalized image.
pub fn sanitize_highlight_crop_box(crop_box: OcrRect, fallback: Option<OcrRect>) -> OcrRect {
    let unit = unit_rect();
    let Some(mut rect) = crop_box.standardized().intersection(unit) else {
        return fallback.unwrap_or(unit);
    };

    let min_size = 0.08;
    if rect.w < min_size {
        let center = rect.mid_x();
        rect.x = center - min_size / 2.0;
        rect.w = min_size;
    }
    if rect.h < min_size {
        let center = rect.mid_y();
        rect.y = center - min_size / 2.0;
        rect.h = min_size;
    }

    rect.x = rect.min_x().max(0.0).min((1.0 - rect.w).max(0.0));
    rect.y = rect.min_y().max(0.0).min((1.0 - rect.h).max(0.0));
    rect.intersection(unit).unwrap_or(unit)
}

/// Project OCR lines into the ordered word sequence used for drag selection.
pub fn selectable_words(lines: &[OcrLine]) -> Vec<OcrWord> {
    let mut sorted_lines = lines.to_vec();
    sorted_lines.sort_by(|lhs, rhs| {
        if (lhs.bbox.mid_y() - rhs.bbox.mid_y()).abs() < 0.006 {
            lhs.bbox.min_x().total_cmp(&rhs.bbox.min_x())
        } else {
            rhs.bbox.mid_y().total_cmp(&lhs.bbox.mid_y())
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
                words.sort_by(|lhs, rhs| lhs.bbox.min_x().total_cmp(&rhs.bbox.min_x()));
                words
            }
        })
        .collect()
}

/// Join selected OCR words into a quote while folding spaces before punctuation.
pub fn join_quote(words: &[OcrWord]) -> String {
    words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace(" ;", ";")
        .replace(" :", ":")
        .replace(" !", "!")
        .replace(" ?", "?")
}

/// Flatten OCR markdown into the one-line alt text attached to uploaded images.
pub fn alt_text_from_markdown(markdown: &str) -> String {
    markdown
        .replace("\n\n", " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn unit_rect() -> OcrRect {
    OcrRect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    }
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

    fn line_with_word(text: &str, bbox: OcrRect, word_bbox: OcrRect) -> OcrLine {
        OcrLine {
            text: text.to_string(),
            bbox,
            confidence: 0.9,
            words: vec![OcrWord {
                text: text.to_string(),
                bbox: word_bbox,
                confidence: 0.8,
            }],
        }
    }

    fn word(text: &str, bbox: OcrRect) -> OcrWord {
        OcrWord {
            text: text.to_string(),
            bbox,
            confidence: 0.8,
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

    #[test]
    fn detect_active_page_selects_dominant_side_of_spread() {
        let lines = vec![
            line("left one", 0.08, 0.75, 0.25, 0.03),
            line("left two", 0.08, 0.70, 0.25, 0.03),
            line("left three", 0.08, 0.65, 0.25, 0.03),
            line("left four", 0.08, 0.60, 0.25, 0.03),
            line("right one", 0.62, 0.75, 0.28, 0.03),
            line("right two", 0.62, 0.70, 0.28, 0.03),
            line("right three", 0.62, 0.65, 0.28, 0.03),
            line("right four", 0.62, 0.60, 0.28, 0.03),
        ];

        let detection = detect_active_page(&lines).expect("spread should be detected");
        assert_eq!(detection.chosen_side, OcrPageSide::Right);
        assert!((detection.page_rect.x - 0.605).abs() < 0.0001);
        assert!((detection.page_rect.w - 0.335).abs() < 0.0001);
        assert!((detection.page_rect.y - 0.56).abs() < 0.0001);
        assert!((detection.page_rect.h - 0.26).abs() < 0.0001);
    }

    #[test]
    fn crop_lines_reprojects_lines_and_words_into_page_rect() {
        let page_rect = OcrRect {
            x: 0.5,
            y: 0.2,
            w: 0.4,
            h: 0.5,
        };
        let lines = vec![
            line_with_word(
                "kept",
                OcrRect {
                    x: 0.6,
                    y: 0.3,
                    w: 0.2,
                    h: 0.1,
                },
                OcrRect {
                    x: 0.62,
                    y: 0.32,
                    w: 0.08,
                    h: 0.04,
                },
            ),
            line("dropped", 0.1, 0.3, 0.2, 0.1),
        ];

        let cropped = crop_lines(&lines, page_rect);
        assert_eq!(cropped.len(), 1);
        assert_eq!(cropped[0].text, "kept");
        assert!((cropped[0].bbox.x - 0.25).abs() < 0.0001);
        assert!((cropped[0].bbox.y - 0.2).abs() < 0.0001);
        assert!((cropped[0].bbox.w - 0.5).abs() < 0.0001);
        assert!((cropped[0].bbox.h - 0.2).abs() < 0.0001);
        assert_eq!(cropped[0].words.len(), 1);
        assert!((cropped[0].words[0].bbox.x - 0.3).abs() < 0.0001);
        assert!((cropped[0].words[0].bbox.y - 0.24).abs() < 0.0001);
    }

    #[test]
    fn default_highlight_crop_box_expands_selection_with_pixel_floor() {
        let crop = default_highlight_crop_box(
            &[
                OcrRect {
                    x: 0.40,
                    y: 0.40,
                    w: 0.05,
                    h: 0.04,
                },
                OcrRect {
                    x: 0.50,
                    y: 0.42,
                    w: 0.04,
                    h: 0.03,
                },
            ],
            400.0,
            800.0,
            0.08,
        )
        .expect("selected boxes should produce a crop");

        assert!((crop.x - 0.28).abs() < 0.0001);
        assert!((crop.y - 0.32).abs() < 0.0001);
        assert!((crop.w - 0.38).abs() < 0.0001);
        assert!((crop.h - 0.21).abs() < 0.0001);
    }

    #[test]
    fn default_highlight_crop_box_returns_none_for_invalid_boxes() {
        assert_eq!(
            default_highlight_crop_box(
                &[OcrRect {
                    x: 0.1,
                    y: 0.1,
                    w: 0.0,
                    h: 0.2,
                }],
                1000.0,
                1000.0,
                0.08,
            ),
            None
        );
    }

    #[test]
    fn sanitize_highlight_crop_box_clamps_and_enforces_min_size() {
        let sanitized = sanitize_highlight_crop_box(
            OcrRect {
                x: 0.98,
                y: 0.98,
                w: 0.01,
                h: 0.01,
            },
            None,
        );

        assert!((sanitized.x - 0.92).abs() < 0.0001);
        assert!((sanitized.y - 0.92).abs() < 0.0001);
        assert!((sanitized.w - 0.08).abs() < 0.0001);
        assert!((sanitized.h - 0.08).abs() < 0.0001);
    }

    #[test]
    fn sanitize_highlight_crop_box_uses_fallback_for_empty_input() {
        let fallback = OcrRect {
            x: 0.2,
            y: 0.3,
            w: 0.4,
            h: 0.5,
        };
        assert_eq!(
            sanitize_highlight_crop_box(
                OcrRect {
                    x: 0.2,
                    y: 0.2,
                    w: 0.0,
                    h: 0.0,
                },
                Some(fallback),
            ),
            fallback
        );
    }

    #[test]
    fn selectable_words_preserves_ios_selection_order_and_line_fallbacks() {
        let lines = vec![
            OcrLine {
                text: "right fallback".to_string(),
                bbox: OcrRect {
                    x: 0.55,
                    y: 0.60,
                    w: 0.20,
                    h: 0.02,
                },
                confidence: 0.7,
                words: vec![],
            },
            OcrLine {
                text: "lower".to_string(),
                bbox: OcrRect {
                    x: 0.10,
                    y: 0.40,
                    w: 0.30,
                    h: 0.02,
                },
                confidence: 0.9,
                words: vec![
                    word(
                        "second",
                        OcrRect {
                            x: 0.25,
                            y: 0.40,
                            w: 0.08,
                            h: 0.02,
                        },
                    ),
                    word(
                        "first",
                        OcrRect {
                            x: 0.12,
                            y: 0.40,
                            w: 0.08,
                            h: 0.02,
                        },
                    ),
                ],
            },
            OcrLine {
                text: "left upper".to_string(),
                bbox: OcrRect {
                    x: 0.10,
                    y: 0.602,
                    w: 0.20,
                    h: 0.02,
                },
                confidence: 0.9,
                words: vec![word(
                    "left",
                    OcrRect {
                        x: 0.12,
                        y: 0.602,
                        w: 0.08,
                        h: 0.02,
                    },
                )],
            },
        ];

        let texts = selectable_words(&lines)
            .into_iter()
            .map(|word| word.text)
            .collect::<Vec<_>>();
        assert_eq!(texts, ["left", "right fallback", "first", "second"]);
    }

    #[test]
    fn join_quote_folds_spaces_before_punctuation() {
        let words = ["Hello", ",", "world", "!", "Really", "?"]
            .into_iter()
            .map(|text| {
                word(
                    text,
                    OcrRect {
                        x: 0.0,
                        y: 0.0,
                        w: 0.1,
                        h: 0.1,
                    },
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(join_quote(&words), "Hello, world! Really?");
    }

    #[test]
    fn alt_text_from_markdown_flattens_paragraphs_and_trims() {
        assert_eq!(
            alt_text_from_markdown("  First paragraph.\n\nSecond line\nthird line.  "),
            "First paragraph. Second line third line."
        );
    }
}
