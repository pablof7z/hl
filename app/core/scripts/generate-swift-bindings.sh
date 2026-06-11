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

rust_inputs_newer_than() {
  local output="$1"
  if [ ! -f "$output" ]; then
    return 0
  fi
  find "$CORE_DIR/src" "$CORE_DIR/Cargo.toml" "$CORE_DIR/Cargo.lock" "$CORE_DIR/uniffi.toml" \
    -newer "$output" -print -quit | grep -q .
}

rust_outputs_are_fresh() {
  local output
  for output in "$@"; do
    if rust_inputs_newer_than "$output"; then
      return 1
    fi
  done
  return 0
}

swift_bindings_are_fresh() {
  local bindgen_lib="$1"
  rust_outputs_are_fresh \
    "$bindgen_lib" \
    "$SWIFT_OUT_DIR/highlighter_core.swift" \
    "$VENDOR_DIR/highlighter_coreFFI.h" \
    "$VENDOR_DIR/module.modulemap"
}

build_ios_sim_libs() {
  echo "Building iOS simulator libraries..." >&2
  mkdir -p "$UNIVERSAL_SIM_DIR"

  case "${ONLY_ACTIVE_ARCH:-}" in
    YES|yes|1|true)
      case " ${ARCHS:-${CURRENT_ARCH:-arm64}} " in
        *" x86_64 "*)
          if rust_outputs_are_fresh "$X86_64_SIM_LIB" "$UNIVERSAL_SIM_LIB"; then
            echo "Using cached iOS simulator libraries." >&2
            return
          fi
          cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target x86_64-apple-ios --release
          cp "$X86_64_SIM_LIB" "$UNIVERSAL_SIM_LIB"
          ;;
        *)
          if rust_outputs_are_fresh "$ARM64_SIM_LIB" "$UNIVERSAL_SIM_LIB"; then
            echo "Using cached iOS simulator libraries." >&2
            return
          fi
          cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios-sim --release
          cp "$ARM64_SIM_LIB" "$UNIVERSAL_SIM_LIB"
          ;;
      esac
      ;;
    *)
      if rust_outputs_are_fresh "$ARM64_SIM_LIB" "$X86_64_SIM_LIB" "$UNIVERSAL_SIM_LIB"; then
        echo "Using cached iOS simulator libraries." >&2
        return
      fi
      cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios-sim --release
      cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target x86_64-apple-ios --release

      echo "Creating universal simulator library..." >&2
      lipo -create "$ARM64_SIM_LIB" "$X86_64_SIM_LIB" -output "$UNIVERSAL_SIM_LIB"
      ;;
  esac
}

case "$platform_name" in
  macosx)
    echo "Building macOS Rust library..." >&2
    if rust_outputs_are_fresh "$MACOS_LIB"; then
      echo "Using cached macOS Rust library." >&2
    else
      cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release
    fi
    default_bindgen_lib="$MACOS_LIB"
    ;;
  iphoneos)
    echo "Building iOS device Rust library..." >&2
    if rust_outputs_are_fresh "$IOS_DEVICE_LIB"; then
      echo "Using cached iOS device Rust library." >&2
    else
      cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios --release
    fi
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

if swift_bindings_are_fresh "$BINDGEN_LIB"; then
  echo "Swift bindings are up to date." >&2
  echo "  Swift:    $SWIFT_OUT_DIR/highlighter_core.swift" >&2
  echo "  FFI header: $VENDOR_DIR/highlighter_coreFFI.h" >&2
  echo "  modulemap:  $VENDOR_DIR/module.modulemap" >&2
  exit 0
fi

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
