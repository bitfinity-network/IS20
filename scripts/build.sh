set -e
cargo build --target wasm32-unknown-unknown --package token-impl --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token-impl.wasm -o src/factory/src/token.wasm
cargo build --target wasm32-unknown-unknown --package token-factory --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token-factory.wasm -o target/wasm32-unknown-unknown/release/factory.wasm
cargo run -p token-factory > src/candid/token-factory.did
cargo run -p token-impl > src/candid/token.did
