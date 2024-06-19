#!/bin/sh

# release/playground.sh builds the web playground.

set -eux

CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
cargo +nightly build \
    -p dbgen-playground \
    --release \
    --no-default-features \
    -Z avoid-dev-deps \
    --target wasm32-unknown-unknown

wasm-opt -Oz \
    -o target/wasm32-unknown-unknown/release/playground.wasm \
    target/wasm32-unknown-unknown/release/dbgen_playground.wasm

wasm-bindgen target/wasm32-unknown-unknown/release/playground.wasm \
    --out-dir dbgen-playground \
    --target no-modules \
    --no-typescript
