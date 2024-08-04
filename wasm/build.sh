#!/bin/bash

mkdir -p ./public
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/rustico_wasm.wasm --out-dir ./public --no-modules
