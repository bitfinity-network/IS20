set -e
cargo build -j`nproc || echo 1` --target wasm32-unknown-unknown --package token --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token.wasm -o src/factory/src/token.wasm
cargo build -j`nproc || echo 1` --target wasm32-unknown-unknown --package factory --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/factory.wasm -o target/wasm32-unknown-unknown/release/factory-opt.wasm
cargo run -p factory > src/candid/factory.did
cargo run -p token > src/candid/token.did
