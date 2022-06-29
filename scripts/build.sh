set -e
cargo build --target wasm32-unknown-unknown --package is20-token-canister --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/is20-token-canister.wasm -o src/factory/src/token.wasm
cargo build --target wasm32-unknown-unknown --package token-factory --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token-factory.wasm -o target/wasm32-unknown-unknown/release/factory.wasm
cargo run -p token-factory > src/candid/token-factory.did
cargo run -p is20-token-canister > src/candid/token.did
