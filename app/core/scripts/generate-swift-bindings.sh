#!/usr/bin/env bash
# Builds highlighter-core for iOS (device + simulator), produces a universal
# simulator static library, and generates Swift bindings via uniffi-bindgen.
#
# Adapted from TENEX's scripts/generate-swift-bindings.sh. Outputs land in
# app/ios/Highlighter/Vendor/ for the Xcode project to consume.
#
# Usage:
#   PLATFORM_NAME=iphonesimulator ./scripts/generate-swift-bindings.sh
#   PLATFORM_NAME=iphoneos        ./scripts/generate-swift-bindings.sh
#   PLATFORM_NAME=macosx          ./scripts/generate-swift-bindings.sh
#   (empty PLATFORM_NAME is treated as iphonesimulator)

set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

CORE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ROOT="$(cd "$CORE_DIR/.." && pwd)"
VENDOR_DIR="$APP_ROOT/ios/Highlighter/Vendor"
SWIFT_OUT_DIR="$APP_ROOT/ios/Highlighter/Sources/Highlighter/Core/Generated"

TEMP_OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/highlighter-swift-bindings.XXXXXX")"
trap 'rm -rf "$TEMP_OUT_DIR"' EXIT

ARM64_SIM_LIB="$CORE_DIR/target/aarch64-apple-ios-sim/release/libhighlighter_core.a"
X86_64_SIM_LIB="$CORE_DIR/target/x86_64-apple-ios/release/libhighlighter_core.a"
IOS_DEVICE_LIB="$CORE_DIR/target/aarch64-apple-ios/release/libhighlighter_core.a"
MACOS_LIB="$CORE_DIR/target/release/libhighlighter_core.a"
UNIVERSAL_SIM_DIR="$CORE_DIR/target/universal-ios-sim/release"
UNIVERSAL_SIM_LIB="$UNIVERSAL_SIM_DIR/libhighlighter_core.a"

platform_name="${PLATFORM_NAME:-}"
default_bindgen_lib=""

build_ios_sim_libs() {
  echo "Building iOS simulator libraries..." >&2
  cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios-sim --release
  cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target x86_64-apple-ios     --release

  echo "Creating universal simulator library..." >&2
  mkdir -p "$UNIVERSAL_SIM_DIR"
  lipo -create "$ARM64_SIM_LIB" "$X86_64_SIM_LIB" -output "$UNIVERSAL_SIM_LIB"
}

case "$platform_name" in
  macosx)
    echo "Building macOS Rust library..." >&2
    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release
    default_bindgen_lib="$MACOS_LIB"
    ;;
  iphoneos)
    echo "Building iOS device Rust library..." >&2
    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios --release
    default_bindgen_lib="$IOS_DEVICE_LIB"
    ;;
  iphonesimulator|"")
    build_ios_sim_libs
    default_bindgen_lib="$ARM64_SIM_LIB"
    ;;
  *)
    echo "Unknown PLATFORM_NAME '$platform_name'; defaulting to macOS." >&2
    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release
    default_bindgen_lib="$MACOS_LIB"
    ;;
esac

BINDGEN_LIB="${HIGHLIGHTER_CORE_LIB_PATH:-$default_bindgen_lib}"
if [ ! -f "$BINDGEN_LIB" ]; then
  echo "Expected Rust library at $BINDGEN_LIB" >&2
  exit 1
fi

mkdir -p "$SWIFT_OUT_DIR" "$VENDOR_DIR"

# uniffi-bindgen internally shells out to `cargo metadata`, which must run
# against the highlighter-core Cargo.toml, not whatever CWD Xcode left us in.
(cd "$CORE_DIR" && cargo run --bin uniffi-bindgen -- generate \
  --library "$BINDGEN_LIB" \
  --language swift \
  --out-dir "$TEMP_OUT_DIR")

if [ ! -f "$TEMP_OUT_DIR/highlighter_core.swift" ]; then
  echo "Expected $TEMP_OUT_DIR/highlighter_core.swift to be generated." >&2
  exit 1
fi

cp "$TEMP_OUT_DIR/highlighter_core.swift"       "$SWIFT_OUT_DIR/highlighter_core.swift"
cp "$TEMP_OUT_DIR/highlighter_coreFFI.h"        "$VENDOR_DIR/highlighter_coreFFI.h"
cp "$TEMP_OUT_DIR/highlighter_coreFFI.modulemap" "$VENDOR_DIR/module.modulemap"

echo "Swift bindings generated." >&2
echo "  Swift:    $SWIFT_OUT_DIR/highlighter_core.swift" >&2
echo "  FFI header: $VENDOR_DIR/highlighter_coreFFI.h" >&2
echo "  modulemap:  $VENDOR_DIR/module.modulemap" >&2
