#!/usr/bin/env bash
# Generuje bindingi Swift i Kotlin z crate'u tracker-core.
#
# Uzycie: ./scripts/build-bindings.sh
# Wynik:  bindings/swift/, bindings/kotlin/
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> budowanie biblioteki"
cargo build --release

LIB="target/release/libtracker_core.dylib"
if [ ! -f "$LIB" ]; then
    LIB="target/release/libtracker_core.so"
fi

echo "==> generowanie bindingow Swift"
rm -rf bindings/swift
cargo run --features cli --bin uniffi-bindgen -- \
    generate --library "$LIB" --language swift --out-dir bindings/swift

echo "==> generowanie bindingow Kotlin"
rm -rf bindings/kotlin
cargo run --features cli --bin uniffi-bindgen -- \
    generate --library "$LIB" --language kotlin --out-dir bindings/kotlin

echo "==> gotowe"
ls -la bindings/swift bindings/kotlin
