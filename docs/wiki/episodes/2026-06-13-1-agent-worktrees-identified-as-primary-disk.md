---
type: episode-card
date: 2026-06-13
session: 16ac1219-405e-4d37-bcba-f2ad417a7e1e
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez/16ac1219-405e-4d37-bcba-f2ad417a7e1e.jsonl
salience: root-cause
status: active
subjects:
  - disk-cleanup
  - agent-worktrees
  - rust-target-dirs
supersedes: []
related_claims: []
source_lines:
  - 548-668
captured_at: 2026-06-13T10:48:47Z
---

# Episode: Agent worktrees identified as primary disk consumer, shifting cleanup from reactive to proactive

## Prior State

Initial scan assumed ordinary Rust `target/` dirs and Xcode caches were the main cleanup candidates; the loop's spec only triggered cleanup at the 5 GB floor threshold

## Trigger

Disk space oscillated wildly (88→74→15→33→53→43→34→54→38→23→8→10 GB) without ever crossing the 5 GB trigger, because builds consumed and released 40–60 GB per cycle. Investigation of recently-modified target dirs revealed `.claude/worktrees/agent-*/target/` directories as the culprit — each parallel Claude Code agent gets its own worktree with a full Rust compilation.

## Decision

Shifted from waiting for the 5 GB floor to proactively sweeping completed (unlocked) agent worktrees every cycle, deleting their `target/` dirs while preserving active (locked) worktrees. Cleaned ~82 GB in the first pass, then continued sweeping newly-completed worktrees each cycle.

## Consequences

- Agent worktrees (~80 GB across 14+ dirs) were the dominant disk consumer, not regular project target/ dirs
- Unlocked worktrees can be safely cleaned; locked worktrees (active agents) must be preserved
- Ongoing proactive sweeps needed because new agent worktrees accumulate faster than the 5 GB reactive threshold would catch
- New agent worktrees continued spawning and consuming disk after initial cleanup, requiring per-cycle sweeps

## Open Tail

- No mechanism to auto-clean worktrees when agents complete — requires periodic manual or loop-driven sweeps
- 80 GB free-space target is still not consistently met due to active builds

## Evidence

- transcript lines 548-668

