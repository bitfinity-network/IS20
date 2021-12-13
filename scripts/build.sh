set -e
cargo build --target wasm32-unknown-unknown --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/ic20.wasm -o target/wasm32-unknown-unknown/release/ic20-opt.wasm
