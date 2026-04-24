import SwiftUI
@preconcurrency import WebKit

/// Full-page reader for web-URL highlights. Loads the target URL in a
/// `WKWebView`; once the DOM is ready we inject a small script that finds
/// the highlighted quote in the page and wraps it in a `<mark>` with our
/// accent color, then scrolls it into view.
///
/// Extraction / reader-mode rendering (Mozilla-Readability-style) is a
/// deliberate follow-up. For now the raw page is the source of truth so
/// users can see what they're looking at and the highlight is anchored.
struct WebReaderView: View {
    let target: WebReaderTarget

    @State private var loadProgress: Double = 0
    @State private var isLoading: Bool = true

    var body: some View {
        ZStack(alignment: .top) {
            WebView(
                url: target.url,
                highlightQuote: target.highlightQuote,
                isLoading: $isLoading,
                loadProgress: $loadProgress
            )

            if isLoading {
                ProgressView(value: loadProgress, total: 1.0)
                    .progressViewStyle(.linear)
                    .tint(Color.highlighterAccent)
                    .transition(.opacity)
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle(target.url.host ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
    }
}

// MARK: - WKWebView wrapper

private struct WebView: UIViewRepresentable {
    let url: URL
    let highlightQuote: String
    @Binding var isLoading: Bool
    @Binding var loadProgress: Double

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        context.coordinator.webView = webView

        // Track load progress so the linear bar has something to draw.
        let progressObservation = webView.observe(\.estimatedProgress, options: [.new]) { web, change in
            Task { @MainActor in
                loadProgress = change.newValue ?? 0
            }
        }
        context.coordinator.progressObservation = progressObservation

        webView.load(URLRequest(url: url))
        return webView
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {
        context.coordinator.highlightQuote = highlightQuote
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(
            highlightQuote: highlightQuote,
            isLoadingBinding: $isLoading
        )
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        var highlightQuote: String
        let isLoadingBinding: Binding<Bool>
        weak var webView: WKWebView?
        var progressObservation: NSKeyValueObservation?
        private var hasInjected: Bool = false

        init(highlightQuote: String, isLoadingBinding: Binding<Bool>) {
            self.highlightQuote = highlightQuote
            self.isLoadingBinding = isLoadingBinding
        }

        nonisolated func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
            Task { @MainActor in
                self.hasInjected = false
                self.isLoadingBinding.wrappedValue = true
            }
        }

        nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            Task { @MainActor in
                // Let JS-heavy pages settle a beat before we walk the DOM.
                try? await Task.sleep(nanoseconds: 400_000_000)
                self.injectHighlight()
                self.isLoadingBinding.wrappedValue = false
            }
        }

        nonisolated func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.isLoadingBinding.wrappedValue = false
            }
        }

        nonisolated func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.isLoadingBinding.wrappedValue = false
            }
        }

        private func injectHighlight() {
            guard !hasInjected, !highlightQuote.isEmpty, let webView else { return }
            hasInjected = true
            let js = Self.buildHighlightScript(quote: highlightQuote)
            webView.evaluateJavaScript(js, completionHandler: nil)
        }

        /// Builds a self-contained IIFE that:
        ///  1. Normalizes whitespace in the needle + page text.
        ///  2. Walks text nodes, concatenates with a single-space joiner
        ///     while recording per-node (startOffset, endOffset) spans over
        ///     the normalized string.
        ///  3. Finds the needle in the normalized string, maps start/end
        ///     back to (node, offset) pairs, creates a DOM Range, wraps it
        ///     with a styled `<mark>`, and scrolls the mark into view.
        ///
        /// Handles quotes that span multiple text nodes and elements.
        private static func buildHighlightScript(quote: String) -> String {
            // Safely serialize the quote as a JSON string literal.
            let encoded = try? JSONEncoder().encode(quote)
            let needleLiteral = encoded.flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
            return """
            (function() {
              try {
                var needleRaw = \(needleLiteral);
                if (!needleRaw) return;
                var normWs = function(s) { return s.replace(/\\s+/g, ' ').trim(); };
                var needle = normWs(needleRaw);
                if (needle.length < 4) return;

                var root = document.body;
                if (!root) return;

                var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
                  acceptNode: function(n) {
                    var p = n.parentNode;
                    if (!p) return NodeFilter.FILTER_REJECT;
                    var tag = (p.tagName || '').toLowerCase();
                    if (tag === 'script' || tag === 'style' || tag === 'noscript' || tag === 'iframe') {
                      return NodeFilter.FILTER_REJECT;
                    }
                    return NodeFilter.FILTER_ACCEPT;
                  }
                });

                var parts = [];
                var norm = '';
                var node;
                while ((node = walker.nextNode())) {
                  var raw = node.nodeValue;
                  if (!raw) continue;
                  // Replace each whitespace run with a single space, recording
                  // original offsets so we can map back later.
                  var nodeSpan = [];
                  var i = 0, j = 0;
                  while (i < raw.length) {
                    var c = raw.charCodeAt(i);
                    if (c === 32 || c === 9 || c === 10 || c === 13 || c === 12 || c === 160) {
                      // whitespace run
                      var runStart = i;
                      while (i < raw.length) {
                        var cc = raw.charCodeAt(i);
                        if (cc === 32 || cc === 9 || cc === 10 || cc === 13 || cc === 12 || cc === 160) { i++; }
                        else { break; }
                      }
                      // emit a single space if we're not at the start of the
                      // normalized buffer and the previous char isn't already
                      // a space.
                      if (norm.length > 0 && norm.charAt(norm.length - 1) !== ' ') {
                        nodeSpan.push({normStart: norm.length, origStart: runStart, origEnd: i});
                        norm += ' ';
                      }
                    } else {
                      nodeSpan.push({normStart: norm.length, origStart: i, origEnd: i + 1});
                      norm += raw.charAt(i);
                      i++;
                    }
                  }
                  parts.push({node: node, spans: nodeSpan});
                }

                var idx = norm.indexOf(needle);
                if (idx < 0) {
                  // Fallback: try a shorter prefix (first 80 chars) in case
                  // the tail diverges (e.g. truncation, stray markup).
                  var probe = needle.slice(0, Math.min(80, needle.length));
                  if (probe.length < 12) return;
                  idx = norm.indexOf(probe);
                  if (idx < 0) return;
                  needle = probe;
                }
                var endIdx = idx + needle.length;

                // Map normalized start/end back to (node, offset).
                function locate(target) {
                  for (var p = 0; p < parts.length; p++) {
                    var spans = parts[p].spans;
                    for (var s = 0; s < spans.length; s++) {
                      var span = spans[s];
                      if (span.normStart >= target) {
                        return {node: parts[p].node, offset: span.origStart};
                      }
                    }
                    // walked entire node without hitting target → continue.
                  }
                  // Target past end — return last position.
                  if (parts.length === 0) return null;
                  var last = parts[parts.length - 1];
                  var lastSpan = last.spans[last.spans.length - 1];
                  return {node: last.node, offset: lastSpan ? lastSpan.origEnd : 0};
                }

                var startLoc = locate(idx);
                var endLoc = locate(endIdx);
                if (!startLoc || !endLoc) return;

                var range = document.createRange();
                range.setStart(startLoc.node, Math.min(startLoc.offset, startLoc.node.nodeValue.length));
                range.setEnd(endLoc.node, Math.min(endLoc.offset, endLoc.node.nodeValue.length));

                var mark = document.createElement('mark');
                mark.setAttribute('data-highlighter', '1');
                mark.style.backgroundColor = 'rgba(197, 125, 95, 0.32)';
                mark.style.color = 'inherit';
                mark.style.padding = '0.05em 0.15em';
                mark.style.borderRadius = '2px';
                mark.style.boxShadow = 'inset 0 -1px 0 rgba(197, 125, 95, 0.6)';

                try {
                  range.surroundContents(mark);
                } catch(e) {
                  // surroundContents rejects ranges that span partial element
                  // boundaries. Fallback: use extractContents + insertNode.
                  try {
                    var frag = range.extractContents();
                    mark.appendChild(frag);
                    range.insertNode(mark);
                  } catch(e2) {
                    return;
                  }
                }

                setTimeout(function() {
                  try { mark.scrollIntoView({behavior: 'smooth', block: 'center'}); } catch(_) {}
                }, 80);
              } catch (err) {
                // Silently no-op — never break the page.
              }
            })();
            """
        }
    }
}
