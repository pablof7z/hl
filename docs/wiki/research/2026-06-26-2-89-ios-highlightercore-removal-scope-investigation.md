---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 2122-2175
agent_attribution: plan89
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "#89 iOS HighlighterCore removal scope investigation with grep-verified findings on deletability, relay-routing derivability, and dead code, plus ordered 3-slice TDD plan"
captured_at: 2026-06-26T20:26:58Z
---

#89 iOS HighlighterCore removal scope investigation with grep-verified findings on deletability, relay-routing derivability, and dead code, plus ordered 3-slice TDD plan

---


User: [Tool result]: chpwd:3: command not found: _iterm2_set_profile
FeedbackThreadsSnapshot USED: app/core/src/kernel/view.rs:343:    /// Feedback thread list projection — `FeedbackThreadsSnapshot` (top-level
FeedbackMessageRowProjection USED: app/ios/Highlighter/Sources/Highlighter/Features/Feedback/FeedbackThreadDetailView.swift:145:        for row: FeedbackMessageRowProjection
FeedbackThreadSnapshot USED: app/core/src/kernel/view.rs:189:    /// chain to project. Snapshot: `ViewSnapshot::FeedbackThread(FeedbackThreadSnapshot)`.
FeedbackRootPublishSnapshot USED: app/ios/Highlighter/Sources/Highlighter/Core/Generated/highlighter_core.swift:9076:public struct FeedbackRootPublishSnapshot {
FeedbackReplyPublishSnapshot USED: app/ios/Highlighter/Sources/Highlighter/Core/Generated/highlighter_core.swift:9006:public struct FeedbackReplyPublishSnapshot {
FeedbackComposerProjectionInput USED: app/ios/Highlighter/Sources/Highlighter/Core/Generated/highlighter_core.swift:8286:public struct FeedbackComposerProject

Assistant: Decisive finding: feedback.rs is **NOT** wholesale dead — its uniffi types + projection functions are **live**, consumed directly by the iOS app (`FeedbackThreadDetailView.swift`) and kernel `view.rs` (267 refs in generated bindings). Only the 7 `&Ndb`/`&NostrRuntime` query/publish fns are dead. This concretely validates the entanglement risk: deleting wrong breaks the iOS feedback view. The safe path is cargo-guided surgical deletion of just the dead fns + their exclusively-used private helpers. Let me read the dead-fn region precisely.

User: [Tool result]: 200	        detail_summary,
201	        status_label,
202	    }
203	}
204	
205	pub fn query_threads_snapshot(
206	    ndb: &Ndb,
207	    coordinate: &str,
208	    current_user_pubkey: Option<&str>,
209	) -> FeedbackThreadsSnapshot {
210	    let current_user_pubkey = current_user_pubkey.unwrap_or_default().trim();
211	    if current_user_pubkey.is_empty() {
212	        return FeedbackThreadsSnapshot {
213	            threads: Vec::new(),
214	            error: String::new(),
215	        };
216	    }
217	
218	    match query_threads(ndb, coordinate, current_user_pubkey) {
219	        Ok(threads) => FeedbackThreadsSnapshot {
220	            threads,
221	            error: String::new(),
222	        },
223	        Err(error) => FeedbackThreadsSnapshot {
224	            threads: Vec::new(),
225	            error: error.to_string(),
226	        },
227	    }
228	}
229	
230	pub fn query_thread_snapshot(ndb: &Ndb, root_event_id: &str) -> FeedbackThreadSnapshot {
231	    match query_thread_event

Assistant: The 7 dead fns are clear. But `threads_snapshot_with_root`/`thread_snapshot_with_event` (250–259) are interspersed pure transformers, and several `pub` helpers won't trigger dead_code warnings if orphaned. Let me check their external callers before deleting, so I delete the full dead set in one consistent pass.

User: [Tool result]: chpwd:3: command not found: _iterm2_set_profile
threads_snapshot_with_root -> ext: NONE
thread_snapshot_with_event -> ext: NONE
optimistically_insert_root_thread -> ext: NONE
upsert_thread_event -> ext: NONE
snapshot_from_events -> ext: NONE
rows_for_events -> ext: NONE
record_from_root -> ext: NONE
