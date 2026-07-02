#!/bin/bash
# Fix libsodium.a for Android cross-compilation, then re-link if needed.
# When cross-compiling on macOS, the host 'ar' tool produces an empty archive
# because it cannot handle Android-target .o files. This script re-creates
# the archive using the NDK's llvm-ar and re-links highlighter-core.
set -e

CORE_DIR="$1"
LLVM_AR="$2"
LLVM_RANLIB="$3"
JNI_OUT_DIR="$4"

if [ -z "$CORE_DIR" ] || [ -z "$LLVM_AR" ] || [ -z "$LLVM_RANLIB" ] || [ -z "$JNI_OUT_DIR" ]; then
    echo "Usage: fix-libsodium.sh <core-dir> <llvm-ar> <llvm-ranlib> <jni-out-dir>"
    exit 1
fi

SODIUM_BUILD_DIR="$CORE_DIR/target/aarch64-linux-android/release/build"
LIB_DIR=$(find "$SODIUM_BUILD_DIR" -path '*/libsodium-sys-stable*/out/installed/lib' -type d 2>/dev/null | head -1)

if [ -z "$LIB_DIR" ]; then
    echo "fix-libsodium: No libsodium build dir found, skipping"
    exit 0
fi

ARCHIVE="$LIB_DIR/libsodium.a"
SOURCE_DIR=$(dirname "$LIB_DIR")/source

OBJ_COUNT=$(find "$SOURCE_DIR" -name '*.o' 2>/dev/null | wc -l | tr -d ' ')
if [ "$OBJ_COUNT" -eq 0 ]; then
    echo "fix-libsodium: No .o files found, skipping"
    exit 0
fi

ARCHIVE_SIZE=$(stat -f%z "$ARCHIVE" 2>/dev/null || stat -c%s "$ARCHIVE" 2>/dev/null || echo "999999")
if [ "$ARCHIVE_SIZE" -lt 200 ]; then
    echo "fix-libsodium: libsodium.a is empty (${ARCHIVE_SIZE} bytes), re-creating with llvm-ar"
    rm -f "$ARCHIVE"
    find "$SOURCE_DIR" -name '*.o' -print0 | xargs -0 "$LLVM_AR" rcs "$ARCHIVE"
    "$LLVM_RANLIB" "$ARCHIVE"
    NEW_SIZE=$(stat -f%z "$ARCHIVE" 2>/dev/null || stat -c%s "$ARCHIVE" 2>/dev/null)
    echo "fix-libsodium: Re-created libsodium.a (${NEW_SIZE} bytes), re-linking highlighter-core"

    # Clean crates that depend on libsodium so they re-link against the fixed archive
    cd "$CORE_DIR"
    cargo clean --target aarch64-linux-android --release -p highlighter-core -p nostr-sdk 2>/dev/null || true
    AR="$LLVM_AR" RANLIB="$LLVM_RANLIB" cargo ndk -t arm64-v8a -o "$JNI_OUT_DIR" build --release
else
    echo "fix-libsodium: libsodium.a looks valid (${ARCHIVE_SIZE} bytes), no fixup needed"
fi
