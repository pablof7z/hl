#!/bin/sh
# Runs on Xcode Cloud before every xcodebuild invocation.
# Installs Rust and compiles libhighlighter_core.a for aarch64-apple-ios.
# Swift bindings (highlighter_core.swift, highlighter_coreFFI.h, module.modulemap)
# are committed to the repo and don't need regenerating here.

set -eu

CORE_DIR="$(cd "$(dirname "$0")/../app/core" && pwd)"

echo "==> Installing Rust toolchain"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
. "$HOME/.cargo/env"

echo "==> Adding aarch64-apple-ios target"
rustup target add aarch64-apple-ios

echo "==> Compiling highlighter-core for iOS device"
cargo build \
  --manifest-path "$CORE_DIR/Cargo.toml" \
  --target aarch64-apple-ios \
  --release

echo "==> Rust build complete"
