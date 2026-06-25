#!/usr/bin/env bash
# Actor-blocking lint (design: actor-blocking-fix.md, Phase 0).
#
# The kernel actor in app/core/src/kernel/actor.rs is the single writer and single
# emitter. Any `runtime.block_on(...)` on the actor thread freezes every other
# message and every snapshot emission. Phase 0 routed all legitimate actor-side
# local work through `block_on_local(...)` and moved network work off-actor via
# the OpRunner. This lint bans any *new* naked `.block_on(` from creeping back
# onto the actor.
#
# Allowed `.block_on(` in actor.rs:
#   1. The single call inside the `block_on_local` wrapper definition.
#   2. Off-actor worker threads (each builds its own runtime), explicitly
#      marked with a trailing `// lint-allow: block_on (worker runtime)`.
#
# Anything else is a violation.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FILE="$ROOT/app/core/src/kernel/actor.rs"

if [[ ! -f "$FILE" ]]; then
  echo "lint-actor-blocking: cannot find $FILE" >&2
  exit 2
fi

violations=0
lineno=0
# The lint guards the PRODUCTION actor code. The `#[cfg(test)] mod tests` block
# legitimately demonstrates the pre-fix blocking anti-pattern in its regression
# proof, so scanning stops at the test-module boundary.
in_tests=0
prev_line=""
while IFS= read -r line; do
  lineno=$((lineno + 1))
  if [[ "$in_tests" -eq 0 && "$prev_line" == "#[cfg(test)]" && "$line" == "mod tests {" ]]; then
    in_tests=1
  fi
  prev_line="$line"
  if [[ "$in_tests" -eq 1 ]]; then
    continue
  fi
  # Only consider lines that actually call .block_on(
  if [[ "$line" != *".block_on("* ]]; then
    continue
  fi
  # Ignore comment lines (doc comments / line comments that merely mention it).
  trimmed="${line#"${line%%[![:space:]]*}"}"
  if [[ "$trimmed" == "//"* ]]; then
    continue
  fi
  # Allow the wrapper definition itself.
  if [[ "$line" == *"let out = runtime.block_on(fut);"* ]]; then
    continue
  fi
  # Allow explicitly-marked off-actor worker runtimes.
  if [[ "$line" == *"// lint-allow: block_on (worker runtime)"* ]]; then
    continue
  fi
  echo "actor-blocking violation: kernel/actor.rs:$lineno" >&2
  echo "    $line" >&2
  violations=$((violations + 1))
done < "$FILE"

# Multi-line bypass guard: a formatter could split `runtime\n    .block_on(`
# across lines so no single line contains ".block_on(". Normalize all
# whitespace runs to single spaces and count occurrences; it must equal the
# allowlisted count found by the per-line pass above (wrapper + worker tags
# + test module). A mismatch means a formatted-away call slipped past.
normalized_total=$(tr -s ' \n\t' ' ' < "$FILE" | { grep -o '\. *block_on(' || true; } | wc -l | tr -d ' ')
perline_total=$(grep -c '\.block_on(' "$FILE" || true)
if [[ "$normalized_total" -ne "$perline_total" ]]; then
  echo "actor-blocking violation: a multi-line-formatted .block_on( call" >&2
  echo "exists in kernel/actor.rs (normalized count $normalized_total != per-line" >&2
  echo "count $perline_total). Re-join the call onto one line so the lint" >&2
  echo "can classify it." >&2
  violations=$((violations + 1))
fi

if [[ "$violations" -gt 0 ]]; then
  echo "" >&2
  echo "Found $violations un-allowlisted .block_on( call(s) in kernel/actor.rs." >&2
  echo "Route actor-side local work through block_on_local(...), or move" >&2
  echo "network work off-actor via OpRunner::submit_op. Off-actor worker" >&2
  echo "threads must carry a '// lint-allow: block_on (worker runtime)' note." >&2
  exit 1
fi

echo "lint-actor-blocking: OK (no un-allowlisted .block_on in kernel/actor.rs)"
