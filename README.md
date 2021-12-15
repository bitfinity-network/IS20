IC20 is an Internet Computer token standard proposed by Infinity Swap.

This repo contains the standard spec (in development) as well as default implementation.

It builds upon the [DIP20 standard](https://github.com/Psychedelic/DIP20/blob/main/spec.md), making it
backwards compatible with it. Additional functionality will be described later.

# Development

## Building

Use build script to build the release version of the token canister, use the build script:

```shell
./scripts/build.sh
```

## Running tests

At the moment only unit tests are written for this crate. So running them is simple as

```shell
cargo test
```

## Enable pre-commit

Before committing to this repo, install and activate the `pre-commit` tool.

```shell
pip install pre-commit
pre-commit install
```
