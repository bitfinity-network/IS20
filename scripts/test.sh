cargo fmt -- --check
cargo clippy -j`nproc || printf 1`
cargo test
