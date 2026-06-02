#!/usr/bin/env bash
# Build AinmEscrow.xcframework + Swift bindings from crates/ainm-escrow-ffi.
#
# UniFFI 0.31.1 library mode: build static libs for the 3 iOS triples, generate
# Swift bindings from a built .a via the crate's `uniffi-bindgen` bin, lipo the
# simulator arches together, then assemble the xcframework (device + sim slices).
#
# Idempotent: safe to re-run. Regenerates bindings and the xcframework in place.
set -euo pipefail
cd "$(dirname "$0")/.."

LIB=libainm_escrow_ffi.a
OUT=ios/Frameworks
GEN=ios/AINMMessenger/AINMMessenger/Generated
HEADERS=target/uniffi-headers

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

echo "==> Building static libs for 3 iOS triples (release)"
cargo build -p ainm-escrow-ffi --release --target aarch64-apple-ios
cargo build -p ainm-escrow-ffi --release --target aarch64-apple-ios-sim
cargo build -p ainm-escrow-ffi --release --target x86_64-apple-ios

echo "==> Generating Swift bindings"
mkdir -p "$GEN"
# Clean any stale generated artifacts so renames below are deterministic.
rm -f "$GEN"/*.swift "$GEN"/*.h "$GEN"/*.modulemap
cargo run --release -p ainm-escrow-ffi --bin uniffi-bindgen -- generate \
  --library "target/aarch64-apple-ios/release/$LIB" \
  --language swift --out-dir "$GEN"

# UniFFI 0.31 emits: ainm_escrow_ffi.swift (high-level API, stays in $GEN and is
# compiled into the app target), ainm_escrow_ffiFFI.h, and
# ainm_escrow_ffiFFI.modulemap. The .h + a module.modulemap go into the headers
# dir consumed by the xcframework; the .swift's `import` line references the
# module declared in that modulemap.
echo "==> Assembling headers dir"
rm -rf "$HEADERS"; mkdir -p "$HEADERS"
mv "$GEN"/*FFI.h "$HEADERS"/
mv "$GEN"/*.modulemap "$HEADERS"/module.modulemap

echo "==> Lipo simulator arches"
mkdir -p target/ios-sim-universal/release
lipo -create \
  target/aarch64-apple-ios-sim/release/$LIB \
  target/x86_64-apple-ios/release/$LIB \
  -output target/ios-sim-universal/release/$LIB

echo "==> Creating xcframework"
rm -rf "$OUT/AinmEscrow.xcframework"; mkdir -p "$OUT"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/$LIB" -headers "$HEADERS" \
  -library "target/ios-sim-universal/release/$LIB" -headers "$HEADERS" \
  -output "$OUT/AinmEscrow.xcframework"

echo "OK: $OUT/AinmEscrow.xcframework + bindings in $GEN"
