fn main() {
    #[cfg(not(target_family = "wasm"))]
    println!("{}", token_factory::idl());
}
