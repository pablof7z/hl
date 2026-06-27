#!/usr/bin/env bash
# Build the nmp-browser-runtime WASM artifact and vendor it into web/static/nmp-wasm/.
#
# Usage:
#   cd web && bun run build:wasm         # release build (default)
#   cd web && bun run build:wasm --dev   # debug build (faster, larger)
#
# Output:
#   web/static/nmp-wasm/nmp-browser-runtime.js
#   web/static/nmp-wasm/nmp-browser-runtime_bg.wasm
#   web/static/nmp-wasm/nmp-browser-runtime.d.ts
#   web/static/nmp-wasm/nmp-browser-runtime_bg.wasm.d.ts
#   web/static/nmp-wasm/snippets/nmp-sqlite-wasm-*/vendor/sqlite-wasm/
#     nmp-sqlite3-shim.mjs  (wasm-pack emits this)
#     sqlite3.mjs           (sqlite.org official WASM/JS build — copied from nmp-sqlite-wasm crate)
#     sqlite3.wasm          (sqlite.org official WASM engine — copied from nmp-sqlite-wasm crate)
#
# The files are committed into the repo so Vercel/CI does not need a Rust
# toolchain (same vendoring posture as the iOS sibling-core dep).
#
# Prerequisites:
#   cargo install wasm-pack
#   rustup target add wasm32-unknown-unknown
#   brew install llvm  (macOS: Homebrew LLVM required for secp256k1-sys wasm32 build)
#
# The Homebrew LLVM clang is required because the system clang (Apple SDK)
# lacks the wasm32 backend needed by secp256k1-sys.  Set CC/AR/CFLAGS so
# cargo picks up the right toolchain when cross-compiling to wasm32.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
NMP_DIR="$(cd "${WEB_DIR}/../../nostr-multi-platform" && pwd)"
NMP_CRATE_DIR="${NMP_DIR}/crates/nmp-browser-runtime"
NMP_SQLITE_WASM_VENDOR="${NMP_DIR}/crates/nmp-sqlite-wasm/vendor/sqlite-wasm"
STATIC_OUT="${WEB_DIR}/static/nmp-wasm"
PKG_OUT="${NMP_CRATE_DIR}/pkg/nmp-browser-runtime"

if [[ ! -d "${NMP_CRATE_DIR}" ]]; then
  echo "ERROR: nmp-browser-runtime crate not found at ${NMP_CRATE_DIR}"
  echo "  Expected sibling repo at ${NMP_DIR}"
  exit 1
fi

# Ensure wasm-pack is on PATH.
if ! command -v wasm-pack &>/dev/null; then
  echo "ERROR: wasm-pack not found. Install with: cargo install wasm-pack"
  exit 1
fi

# Ensure the wasm32 target is installed.
if ! rustup target list --installed 2>/dev/null | grep -q "wasm32-unknown-unknown"; then
  echo "==> Adding wasm32-unknown-unknown target ..."
  rustup target add wasm32-unknown-unknown
fi

# Optional --dev flag for an unoptimised (debug) build.
PROFILE_FLAG="--release"
if [[ "${1:-}" == "--dev" ]]; then
  PROFILE_FLAG="--dev"
fi

# Homebrew LLVM toolchain for secp256k1-sys wasm32 support.
# Apple's system clang lacks the wasm32 backend; Homebrew llvm ships it.
LLVM_PREFIX="/opt/homebrew/opt/llvm"
if [[ -x "${LLVM_PREFIX}/bin/clang" ]]; then
  export CC_wasm32_unknown_unknown="${LLVM_PREFIX}/bin/clang"
  export AR_wasm32_unknown_unknown="${LLVM_PREFIX}/bin/llvm-ar"
  export CFLAGS_wasm32_unknown_unknown="--target=wasm32-unknown-unknown"
  echo "==> Using Homebrew LLVM clang: ${LLVM_PREFIX}/bin/clang"
else
  echo "WARNING: Homebrew LLVM not found at ${LLVM_PREFIX}; falling back to system clang."
  echo "  secp256k1-sys may fail to compile. Install with: brew install llvm"
fi

echo "==> Building nmp-browser-runtime WASM artifact..."
wasm-pack build \
  "${NMP_CRATE_DIR}" \
  --target web \
  "${PROFILE_FLAG}" \
  --out-name nmp-browser-runtime \
  --out-dir "${PKG_OUT}" \
  --features wasm

echo "==> Copying artifacts to ${STATIC_OUT}..."
mkdir -p "${STATIC_OUT}"

# Copy primary wasm-pack output files.
cp "${PKG_OUT}/nmp-browser-runtime.js"         "${STATIC_OUT}/"
cp "${PKG_OUT}/nmp-browser-runtime_bg.wasm"    "${STATIC_OUT}/"
cp "${PKG_OUT}/nmp-browser-runtime.d.ts"       "${STATIC_OUT}/"
cp "${PKG_OUT}/nmp-browser-runtime_bg.wasm.d.ts" "${STATIC_OUT}/"

# Copy the snippets directory (contains the SQLite shim stub emitted by wasm-pack).
# Replace (not merge): a stale snippet-hash dir from a previous NMP rev would
# otherwise linger, and the `ls | head -1` below could then select the wrong
# hash dir — copying sqlite3.mjs beside the wrong shim and silently degrading
# the @wasm tier. A clean replace keeps exactly one (current) snippet dir.
rm -rf "${STATIC_OUT}/snippets"
cp -r "${PKG_OUT}/snippets" "${STATIC_OUT}/"

# Copy the sqlite.org official SQLite WASM engine alongside the shim.
# The shim (nmp-sqlite3-shim.mjs) imports "./sqlite3.mjs" at module load time;
# if sqlite3.mjs is absent the dynamic import of nmp-browser-runtime.js fails
# and the wasm bridge degrades to DegradedRuntime("browser_bridge_unavailable").
# Both sqlite3.mjs (the JS driver) and sqlite3.wasm (the engine binary) must
# live next to the shim so their relative paths resolve correctly.
SNIPPET_HASH_DIR=$(ls "${STATIC_OUT}/snippets/" | head -1)
SNIPPET_VENDOR="${STATIC_OUT}/snippets/${SNIPPET_HASH_DIR}/vendor/sqlite-wasm"
if [[ -n "${SNIPPET_HASH_DIR}" && -d "${SNIPPET_VENDOR}" ]]; then
  cp "${NMP_SQLITE_WASM_VENDOR}/sqlite3.mjs"  "${SNIPPET_VENDOR}/"
  cp "${NMP_SQLITE_WASM_VENDOR}/sqlite3.wasm" "${SNIPPET_VENDOR}/"
  echo "==> Copied sqlite3.mjs + sqlite3.wasm to ${SNIPPET_VENDOR}"
else
  echo "WARNING: Could not find snippet vendor dir; sqlite3.mjs NOT copied."
  echo "  The @wasm E2E tier will degrade. Expected: ${SNIPPET_VENDOR}"
fi

echo ""
echo "==> Done. Vendored files:"
find "${STATIC_OUT}" -type f | sort | sed 's/^/    /'
echo ""
echo "  Commit web/static/nmp-wasm/ to vendor the artifact."
