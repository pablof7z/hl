import SwiftUI
import UIKit

/// UITextView-backed body for the capture review screen. Renders the
/// reconstructed OCR markdown as attributed text with native selection, and
/// exposes a single "Highlight" action in the edit menu that stashes the
/// selected span as a pending highlight (not yet published).
///
/// The stashed quote is drawn with a yellow background so the user sees what
/// they've picked. The view is non-scrolling — the parent SwiftUI ScrollView
/// owns the gesture.
struct CapturePageBodyView: UIViewRepresentable {
    let attributedText: NSAttributedString
    let stashedQuote: String?
    let accent: UIColor
    let highlightTint: UIColor
    let paperColor: UIColor

    /// User selected text and tapped **Highlight** — stash the quote + its
    /// surrounding paragraph as context. Does NOT publish; the tray's
    /// Publish button is the terminal action.
    var onStash: (_ quote: String, _ context: String) -> Void

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView(usingTextLayoutManager: true)
        tv.isEditable = false
        tv.isSelectable = true
        tv.isScrollEnabled = false
        tv.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        tv.setContentHuggingPriority(.defaultHigh, for: .vertical)
        tv.backgroundColor = paperColor
        tv.textContainer.lineFragmentPadding = 0
        tv.textContainerInset = UIEdgeInsets(top: 20, left: 20, bottom: 40, right: 20)
        tv.attributedText = overlaid(attributedText, quote: stashedQuote)
        tv.adjustsFontForContentSizeCategory = true
        tv.dataDetectorTypes = []
        tv.linkTextAttributes = [:]
        tv.delegate = context.coordinator
        tv.tintColor = accent
        return tv
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        let overlay = overlaid(attributedText, quote: stashedQuote)
        if uiView.attributedText != overlay {
            // Preserve selection where possible — reassigning attributedText
            // resets it, which kills selection handles mid-drag.
            let savedRange = uiView.selectedRange
            uiView.attributedText = overlay
            if savedRange.location + savedRange.length <= overlay.length {
                uiView.selectedRange = savedRange
            }
        }
        uiView.backgroundColor = paperColor
        context.coordinator.parent = self
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    private func overlaid(_ base: NSAttributedString, quote: String?) -> NSAttributedString {
        guard let quote, !quote.isEmpty else { return base }
        let mutable = base.mutableCopy() as! NSMutableAttributedString
        let plain = mutable.string
        if let range = plain.range(of: quote) {
            let nsRange = NSRange(range, in: plain)
            mutable.addAttribute(
                .backgroundColor,
                value: highlightTint.withAlphaComponent(0.55),
                range: nsRange
            )
        }
        return mutable
    }

    @MainActor
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: CapturePageBodyView

        init(parent: CapturePageBodyView) {
            self.parent = parent
        }

        func textView(
            _ textView: UITextView,
            editMenuForTextIn range: NSRange,
            suggestedActions: [UIMenuElement]
        ) -> UIMenu? {
            guard range.length > 0 else {
                return UIMenu(children: suggestedActions)
            }

            let stashAction = UIAction(
                title: "Highlight",
                image: UIImage(systemName: "highlighter")
            ) { [weak self, weak textView] _ in
                guard let self, let tv = textView else { return }
                let (quote, context) = self.selectionText(tv)
                guard !quote.isEmpty else { return }
                self.parent.onStash(quote, context)
                tv.selectedRange = NSRange(location: 0, length: 0)
            }

            let customMenu = UIMenu(options: .displayInline, children: [stashAction])
            return UIMenu(children: [customMenu] + suggestedActions)
        }

        private func selectionText(_ tv: UITextView) -> (quote: String, context: String) {
            let range = tv.selectedRange
            guard range.length > 0 else { return ("", "") }
            guard let textRange = Range(range, in: tv.text) else { return ("", "") }
            let quote = String(tv.text[textRange]).trimmingCharacters(in: .whitespacesAndNewlines)

            let full = tv.text as NSString
            var start = range.location
            var end = range.location + range.length
            while start > 0 {
                let prior = full.substring(with: NSRange(location: start - 1, length: 1))
                if prior == "\n" {
                    if start >= 2, full.substring(with: NSRange(location: start - 2, length: 1)) == "\n" {
                        break
                    }
                }
                start -= 1
            }
            while end < full.length {
                if end + 1 < full.length,
                   full.substring(with: NSRange(location: end, length: 1)) == "\n",
                   full.substring(with: NSRange(location: end + 1, length: 1)) == "\n" {
                    break
                }
                end += 1
            }
            let paragraphRange = NSRange(location: start, length: max(0, end - start))
            let paragraph = full.substring(with: paragraphRange)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            let context = paragraph == quote ? "" : paragraph
            return (quote, context)
        }
    }
}
