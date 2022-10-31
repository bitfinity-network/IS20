set -e
cargo build --target wasm32-unknown-unknown --package is20-token-canister --features export-api --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/is20-token-canister.wasm -o target/wasm32-unknown-unknown/release/token.wasm
cargo build --target wasm32-unknown-unknown --package token-factory --features export-api --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token-factory.wasm -o target/wasm32-unknown-unknown/release/factory.wasm
cargo run -p token-factory --features export-api > src/candid/token-factory.did
cargo run -p is20-token-canister --features export-api > src/candid/token.did
