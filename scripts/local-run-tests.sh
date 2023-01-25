#!/usr/bin/env sh
# To run from the local env 
# FMT and clippy runs as a separate jobs on CI

cargo fmt -- --check
cargo clippy

cargo test -p token-factory 
cargo test -p is20-token --features auction
cargo test -p is20-token-canister
