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
#
# The files are committed into the repo so Vercel/CI does not need a Rust
# toolchain (same vendoring posture as the iOS sibling-core dep).
#
# Prerequisites:
#   cargo install wasm-pack
#   rustup target add wasm32-unknown-unknown

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
NMP_DIR="$(cd "${WEB_DIR}/../../nostr-multi-platform" && pwd)"
BUILD_SCRIPT="${NMP_DIR}/crates/nmp-browser-runtime/scripts/build-wasm.sh"
WASM_PKG="${NMP_DIR}/pkg/nmp-browser-runtime"
STATIC_OUT="${WEB_DIR}/static/nmp-wasm"

if [[ ! -f "${BUILD_SCRIPT}" ]]; then
  echo "ERROR: NMP build script not found at ${BUILD_SCRIPT}"
  echo "  Expected sibling repo at ${NMP_DIR}"
  exit 1
fi

echo "==> Building nmp-browser-runtime WASM artifact..."
bash "${BUILD_SCRIPT}" "${@}"

echo "==> Copying artifacts to ${STATIC_OUT}..."
mkdir -p "${STATIC_OUT}"

# wasm-pack names files with underscores by default:
#   nmp_browser_runtime.js  → nmp-browser-runtime.js
#   nmp_browser_runtime_bg.wasm → nmp-browser-runtime_bg.wasm  (keep _bg suffix)
#   nmp_browser_runtime.d.ts → nmp-browser-runtime.d.ts

JS_SRC="${WASM_PKG}/nmp_browser_runtime.js"
WASM_SRC="${WASM_PKG}/nmp_browser_runtime_bg.wasm"
DTS_SRC="${WASM_PKG}/nmp_browser_runtime.d.ts"

JS_DST="${STATIC_OUT}/nmp-browser-runtime.js"
WASM_DST="${STATIC_OUT}/nmp-browser-runtime_bg.wasm"
DTS_DST="${STATIC_OUT}/nmp-browser-runtime.d.ts"

# Copy JS and patch the internal wasm reference (underscore → hyphen).
sed 's/nmp_browser_runtime_bg\.wasm/nmp-browser-runtime_bg.wasm/g' \
  "${JS_SRC}" > "${JS_DST}"

cp "${WASM_SRC}" "${WASM_DST}"

if [[ -f "${DTS_SRC}" ]]; then
  cp "${DTS_SRC}" "${DTS_DST}"
fi

echo ""
echo "==> Done. Vendored files:"
ls "${STATIC_OUT}" | sed 's/^/    /'
echo ""
echo "  Commit web/static/nmp-wasm/ to vendor the artifact."
