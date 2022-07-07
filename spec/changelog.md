# IS20 changelog

There have been substantial changes to the IS20 token standard.
We have adapted the ICRC-1 token methods while retaining the IS20 standard with minimal changes.

## Changes

The following API changes have been made to the IS20 token standard :-

### Transfer

- The `transfer` function has been divided into two functions: `icrc_transfer` and `is20_transfer`.
- The `transferIncludeFee` has been divided into two functions: `icrc_transferIncludeFee` and `is20_transferIncludeFee`.

### Burn

- The `burn` function has been divided into two functions: `burn` and `is20_burn`.

### Mint

- The `mint` function has been divided into two functions: `mint` and `is20_mint`.

### Deprecated

- `transferFrom`
- `approve`
