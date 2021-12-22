![IS20 banner](https://user-images.githubusercontent.com/6412426/146728389-42384977-0ed3-43a6-83d3-ce16db609c09.png)

IS20 is an Internet Computer token standard proposed by Infinity Swap.

You can find the standard spec at [spec/IS20.md]() and the default implementation in the `src` directory.

# Development

## Building

Use build script to build the release version of the token canister, use the build script:

```shell
./scripts/build.sh
```

## Running tests

In order to run tests:

```shell
cargo test
```

## Enable pre-commit

Before committing to this repo, install and activate the `pre-commit` tool.

```shell
pip install pre-commit
pre-commit install
```

## Local Run

```bash
dfx start --background
dfx deploy
dfx stop
```

## Candid Files

In order to generate candid files, run the following command:

```bash
cargo run -p factory > src/candid/factory.did
cargo run -p token > src/candid/token.did
```
