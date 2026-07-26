#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
legacy_url="github.com/pablof7z/nostr-multi-platform"
new_url="github.com/pablof7z/nmp.git"
legacy_max=22

legacy_declarations="$(
  find "$repo_root/app" -name Cargo.toml -type f \
    -exec grep -nH -F "$legacy_url" {} + || true
)"
if [[ -n "$legacy_declarations" ]]; then
  legacy_count="$(printf '%s\n' "$legacy_declarations" | wc -l | tr -d ' ')"
else
  legacy_count=0
fi

if (( legacy_count > legacy_max )); then
  echo "NMP boundary violation: legacy declarations increased ($legacy_count > $legacy_max)." >&2
  printf '%s\n' "$legacy_declarations" >&2
  exit 1
fi

new_dependencies="$(
  find "$repo_root/app" -name Cargo.toml -type f \
    -exec grep -nH -F "$new_url" {} + || true
)"
if [[ -z "$new_dependencies" ]]; then
  echo "NMP boundary violation: no pinned new-NMP dependency found." >&2
  exit 1
fi

unpinned="$(
  printf '%s\n' "$new_dependencies" |
    grep -Ev 'rev = "[0-9a-f]{40}"' || true
)"
if [[ -n "$unpinned" ]]; then
  echo "NMP boundary violation: new-NMP dependencies require an exact 40-hex rev." >&2
  printf '%s\n' "$unpinned" >&2
  exit 1
fi

branch_tracking="$(
  printf '%s\n' "$new_dependencies" |
    grep -E 'branch[[:space:]]*=' || true
)"
if [[ -n "$branch_tracking" ]]; then
  echo "NMP boundary violation: new-NMP dependencies must not track a branch." >&2
  printf '%s\n' "$branch_tracking" >&2
  exit 1
fi

forbidden_packages='nmp-(store|router|transport|resolver|engine|executor|signer|ffi)'
mechanism_dependencies="$(
  printf '%s\n' "$new_dependencies" |
    grep -E "(^[^:]+:[0-9]+:${forbidden_packages}[[:space:]]*=|package[[:space:]]*=[[:space:]]*\"${forbidden_packages}\")" || true
)"
if [[ -n "$mechanism_dependencies" ]]; then
  echo "NMP boundary violation: application code must use the facade, not mechanism/FFI crates." >&2
  printf '%s\n' "$mechanism_dependencies" >&2
  exit 1
fi

unstable_usage="$(
  find "$repo_root/app" -type f \( -name Cargo.toml -o -name '*.rs' \) \
    -exec grep -nH -F 'unstable-mechanism' {} + || true
)"
if [[ -n "$unstable_usage" ]]; then
  echo "NMP boundary violation: unstable-mechanism is not an application API." >&2
  printf '%s\n' "$unstable_usage" >&2
  exit 1
fi

new_count="$(printf '%s\n' "$new_dependencies" | wc -l | tr -d ' ')"
echo "NMP boundary OK: legacy declarations=$legacy_count/$legacy_max, pinned new-NMP dependencies=$new_count, mechanism dependencies=0, unstable-mechanism=0"
