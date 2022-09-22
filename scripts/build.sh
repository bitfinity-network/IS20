set -e
cargo build --target wasm32-unknown-unknown --package is20-token-canister --features export_api --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/is20-token-canister.wasm -o src/factory/src/token.wasm
cargo build --target wasm32-unknown-unknown --package token-factory --features export_api --release
ic-cdk-optimizer target/wasm32-unknown-unknown/release/token-factory.wasm -o target/wasm32-unknown-unknown/release/factory.wasm
cargo run -p token-factory --features export_api > src/candid/token-factory.did
cargo run -p is20-token-canister --features export_api > src/candid/token.did
