package com.highlighter.app.ui.capture

/**
 * A single recognized line returned by ML Kit Text Recognition, with the
 * vertical position needed for paragraph reconstruction.
 *
 * [top] and [bottom] are the line's y-coordinates in the image (pixels,
 * origin top-left). [left] and [right] are x-coordinates.
 */
internal data class OcrLine(
    val text: String,
    val top: Float,
    val bottom: Float,
    val left: Float,
    val right: Float,
) {
    val midY: Float get() = (top + bottom) / 2f
    val height: Float get() = bottom - top
    val width: Float get() = right - left
    val midX: Float get() = (left + right) / 2f
}

/**
 * Mirrors iOS OCRStructureReconstructor: turns ML Kit's line-by-line output
 * into readable markdown that respects paragraphs, headings, and list items.
 *
 * All heuristics are ratio-based against per-page statistics so they work
 * across different zoom levels and book sizes.
 */
internal object OcrStructureReconstructor {

    fun toMarkdown(lines: List<OcrLine>): String {
        if (lines.isEmpty()) return ""
        val normalized = lines
            .map { it.copy(text = normalizeLine(it.text)) }
            .filter { it.text.isNotEmpty() }
        if (normalized.isEmpty()) return ""

        val ordered = readingOrder(normalized)
        val stats = buildPageStats(ordered)
        val trimmed = stripRunningHeadersAndFooters(ordered, stats)
        return assembleMarkdown(trimmed, stats)
    }

    // ── Normalization ────────────────────────────────────────────────────────

    private fun normalizeLine(raw: String): String {
        var s = raw
        // Common OCR ligature artifacts.
        val ligatures = mapOf(
            "ﬀ" to "ff", "ﬁ" to "fi", "ﬂ" to "fl",
            "ﬃ" to "ffi", "ﬄ" to "ffl",
        )
        for ((from, to) in ligatures) s = s.replace(from, to)
        // Zero-width chars.
        s = s.replace("​", "")
        return s.trim()
    }

    // ── Reading order (two-column aware) ────────────────────────────────────

    private fun readingOrder(lines: List<OcrLine>): List<OcrLine> {
        val lefts = lines.map { it.left }.sorted()
        val lo = lefts.firstOrNull() ?: return lines
        val hi = lefts.lastOrNull() ?: return lines
        val spread = hi - lo

        if (spread > 0.25f * (lines.maxOf { it.right } - lines.minOf { it.left })) {
            // Attempt two-column split using k-means on left edges (3 iterations).
            var split = (lo + hi) / 2f
            repeat(6) {
                val leftGroup = lefts.filter { it < split }
                val rightGroup = lefts.filter { it >= split }
                if (leftGroup.isEmpty() || rightGroup.isEmpty()) return@repeat
                val lm = leftGroup.average().toFloat()
                val rm = rightGroup.average().toFloat()
                split = (lm + rm) / 2f
            }
            val leftLines = lines.filter { it.left < split }
            val rightLines = lines.filter { it.left >= split }
            val threshold = lines.size * 0.25
            if (leftLines.size >= threshold && rightLines.size >= threshold) {
                val leftMaxX = leftLines.maxOf { it.right }
                val rightMinX = rightLines.minOf { it.left }
                if (leftMaxX <= rightMinX + 0.02f * (lines.maxOf { it.right })) {
                    return sortTopDown(leftLines) + sortTopDown(rightLines)
                }
            }
        }
        return sortTopDown(lines)
    }

    /** Sort lines top-to-bottom; within the same row, left-to-right. */
    private fun sortTopDown(lines: List<OcrLine>): List<OcrLine> =
        lines.sortedWith(compareBy({ it.midY }, { it.left }))

    // ── Page statistics ──────────────────────────────────────────────────────

    private data class PageStats(
        val medianHeight: Float,
        val medianGap: Float,
        val bodyLeftEdge: Float,
        val bodyRightEdge: Float,
        val pageCenterX: Float,
        val imageWidth: Float,
        val imageHeight: Float,
    )

    private fun modeBinned(values: List<Float>, binFraction: Float, range: Float): Float {
        if (values.isEmpty()) return 0f
        val binSize = (range * binFraction).coerceAtLeast(1f)
        val buckets = mutableMapOf<Int, MutableList<Float>>()
        for (v in values) {
            val bucket = (v / binSize).toInt()
            buckets.getOrPut(bucket) { mutableListOf() }.add(v)
        }
        val best = buckets.maxByOrNull { it.value.size }?.value ?: return 0f
        return best.average().toFloat()
    }

    private fun buildPageStats(lines: List<OcrLine>): PageStats {
        val heights = lines.map { it.height }.sorted()
        val medianHeight = heights[heights.size / 2]

        val gaps = mutableListOf<Float>()
        for (i in 1 until lines.size) {
            val gap = lines[i].top - lines[i - 1].bottom
            if (gap > 0) gaps.add(gap)
        }
        gaps.sort()
        val medianGap = if (gaps.isEmpty()) medianHeight * 0.3f else gaps[gaps.size / 2]

        val rangeX = (lines.maxOf { it.right } - lines.minOf { it.left }).coerceAtLeast(1f)
        val bodyLeftEdge = modeBinned(lines.map { it.left }, binFraction = 0.05f, range = rangeX)
        val bodyRightEdge = modeBinned(lines.map { it.right }, binFraction = 0.05f, range = rangeX)
        val pageCenterX = (bodyLeftEdge + bodyRightEdge) / 2f

        val imageWidth = rangeX + 1f
        val imageHeight = (lines.maxOf { it.bottom } - lines.minOf { it.top }) + 1f

        return PageStats(
            medianHeight = medianHeight,
            medianGap = medianGap,
            bodyLeftEdge = bodyLeftEdge,
            bodyRightEdge = bodyRightEdge,
            pageCenterX = pageCenterX,
            imageWidth = imageWidth,
            imageHeight = imageHeight,
        )
    }

    // ── Header / footer stripping ────────────────────────────────────────────

    private fun stripRunningHeadersAndFooters(lines: List<OcrLine>, stats: PageStats): List<OcrLine> {
        val imageTop = lines.minOf { it.top }
        val imageBottom = lines.maxOf { it.bottom }
        val imageHeight = (imageBottom - imageTop).coerceAtLeast(1f)

        return lines.filter { line ->
            val relTop = (line.top - imageTop) / imageHeight
            val relBottom = (line.bottom - imageTop) / imageHeight
            val atTop = relTop < 0.06f
            val atBottom = relBottom > 0.94f
            if (!atTop && !atBottom) return@filter true

            val heightRatio = line.height / stats.medianHeight.coerceAtLeast(1f)
            // Large centered line at the very top is a chapter opener — keep.
            if (heightRatio > 1.2f) return@filter true

            val trimmed = line.text.trim()
            // Bare page numbers are always dropped.
            if (trimmed.matches(Regex("^\\d{1,4}$"))) return@filter false
            val wordCount = trimmed.split(Regex("\\s+")).filter { it.isNotEmpty() }.size
            // Very short lines at the edges are almost certainly running headers.
            wordCount > 5
        }
    }

    // ── Assembly ─────────────────────────────────────────────────────────────

    private enum class BlockKind {
        Body, Heading1, Heading2, ListItemOrdered, ListItemUnordered, BlockQuote
    }

    private data class Classified(val kind: BlockKind, val text: String)

    private fun assembleMarkdown(lines: List<OcrLine>, stats: PageStats): String {
        if (lines.isEmpty()) return ""

        var out = ""
        var currentBlock = ""
        var currentKind = BlockKind.Body

        fun flush() {
            val piece = currentBlock.trim()
            if (piece.isEmpty()) { currentBlock = ""; return }
            out += when (currentKind) {
                BlockKind.Heading1 -> "# $piece\n\n"
                BlockKind.Heading2 -> "## $piece\n\n"
                BlockKind.ListItemOrdered -> "1. $piece\n"
                BlockKind.ListItemUnordered -> "- $piece\n"
                BlockKind.BlockQuote -> "> ${piece.replace("\n", "\n> ")}\n\n"
                BlockKind.Body -> "$piece\n\n"
            }
            currentBlock = ""
        }

        for ((i, line) in lines.withIndex()) {
            val classified = classify(line, stats)
            if (i == 0) {
                currentKind = classified.kind
                currentBlock = classified.text
                continue
            }

            val prev = lines[i - 1]
            val isHardBreak = isHardBreak(prev, line, stats)

            if (classified.kind != currentKind || isHardBreak) {
                flush()
                currentKind = classified.kind
                currentBlock = classified.text
            } else {
                currentBlock = softJoin(currentBlock, classified.text)
            }
        }
        flush()

        // Collapse triple+ newlines.
        var result = out
        while (result.contains("\n\n\n")) result = result.replace("\n\n\n", "\n\n")
        return result.trim() + "\n"
    }

    private fun classify(line: OcrLine, stats: PageStats): Classified {
        val text = line.text
        val heightRatio = line.height / stats.medianHeight.coerceAtLeast(1f)
        val bodyWidth = (stats.bodyRightEdge - stats.bodyLeftEdge).coerceAtLeast(1f)
        val indentRatio = (line.left - stats.bodyLeftEdge) / bodyWidth
        val centeredDeviation = Math.abs(line.midX - stats.pageCenterX) / bodyWidth

        // List detection first.
        stripListMarker(text)?.let { (ordered, remainder) ->
            return Classified(if (ordered) BlockKind.ListItemOrdered else BlockKind.ListItemUnordered, remainder)
        }

        // Heading: requires at least two corroborating signals.
        var headingSignals = 0
        if (heightRatio > 1.25f) headingSignals++
        if (centeredDeviation < 0.08f && line.width / stats.medianHeight > 1.5f) headingSignals++
        val wordCount = text.split(Regex("\\s+")).filter { it.isNotEmpty() }.size
        val terminator = text.lastOrNull()?.let { ".!?".contains(it) } ?: false
        if (wordCount < 8 && !terminator) headingSignals++
        if (text == text.uppercase() && wordCount > 0 && text.any { it.isLetter() }) headingSignals++
        // Single/two-glyph drop-cap observations are not headings.
        val isDropCap = (line.width / stats.medianHeight) < 1.5f && text.length <= 2
        if (headingSignals >= 2 && !isDropCap) {
            return Classified(if (heightRatio > 1.55f) BlockKind.Heading1 else BlockKind.Heading2, text)
        }

        // Block quote — sustained inset from both edges.
        val pulledLeft = indentRatio > 0.06f
        val pulledRight = (stats.bodyRightEdge - line.right) / bodyWidth > 0.06f
        if (pulledLeft && pulledRight && heightRatio < 1.2f) {
            return Classified(BlockKind.BlockQuote, text)
        }

        return Classified(BlockKind.Body, text)
    }

    private fun stripListMarker(text: String): Pair<Boolean, String>? {
        val trimmed = text.trim()
        val bullets = setOf('•', '·', '●', '○', '▪', '◦', '–', '—')
        if (trimmed.firstOrNull() in bullets) {
            val remainder = trimmed.drop(1).trim()
            if (remainder.length > 2) return false to remainder
        }
        Regex("^\\d{1,2}[.)]}\\s+").find(trimmed)?.let { m ->
            val remainder = trimmed.substring(m.range.last + 1)
            if (remainder.isNotEmpty()) return true to remainder
        }
        if (trimmed.startsWith("- ") && trimmed.length > 2) {
            return false to trimmed.drop(2)
        }
        return null
    }

    private fun isHardBreak(prev: OcrLine, curr: OcrLine, stats: PageStats): Boolean {
        val gap = curr.top - prev.bottom
        val gapRatio = gap / stats.medianHeight.coerceAtLeast(1f)
        val bodyWidth = (stats.bodyRightEdge - stats.bodyLeftEdge).coerceAtLeast(1f)
        val indentRatio = (curr.left - stats.bodyLeftEdge) / bodyWidth
        val prevShortRatio = (stats.bodyRightEdge - prev.right) / bodyWidth
        val prevEndsTerminal = prev.text.trimEnd().lastOrNull()?.let { ".!?\"'".contains(it) } ?: false

        if (gapRatio > 0.6f) return true
        if (indentRatio > 0.04f && gapRatio > 0.15f) return true
        if (prevShortRatio > 0.12f && prevEndsTerminal && gapRatio > 0.2f) return true
        return false
    }

    /** Join two fragments, handling end-of-line hyphenation. */
    private fun softJoin(left: String, right: String): String {
        if (left.isEmpty()) return right
        if (right.isEmpty()) return left
        if (left.endsWith("-") || left.endsWith("‐") || left.endsWith("‑")) {
            val withoutHyphen = left.dropLast(1)
            val leftLower = withoutHyphen.lastOrNull()?.isLowerCase() ?: false
            val rightLower = right.firstOrNull()?.isLowerCase() ?: false
            if (leftLower && rightLower) return withoutHyphen + right
        }
        return "$left $right"
    }

    /** Flatten markdown to a single-line alt-text string (mirrors iOS flattenForAlt). */
    fun flattenForAlt(markdown: String): String =
        markdown
            .replace("\n\n", " ")
            .replace("\n", " ")
            .trim()
}
