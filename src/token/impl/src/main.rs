fn main() {
    #[cfg(not(any(target_family = "wasm", test)))]
    print!("{}", is20_token_canister::idl());
}
