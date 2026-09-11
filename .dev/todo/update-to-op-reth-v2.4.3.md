# op-reth bump to upstream `op-reth/v2.4.3` (op-reth-v2.4.3-fh3.1)

## Branch layout

- `release/op-reth-2.x` follows tagged upstream `op-reth/vX.Y.Z` releases. Tags:
  `op-reth-vX.Y.Z-fh3.N[-M]`.
- `release/world-chain-2.x` serves the exact upstream rev world-chain pins. Tags:
  `world-chain-vX.Y.Z-fh3.N`.

The `96ffbb2a` bump (`world-chain-v2.4.3-fh3.1`/`-fh3.2`) landed on `release/op-reth-2.x`.
`96ffbb2a` is an ancestor of `op-reth/v2.4.3`, so merging `v2.4.3` on top is clean.

## Reth dependency

Upstream `v2.4.3` pins `op-rs/reth` rev `aef8d3ef` (reth `v2.4.0` + 9 upstream commits + 1
op-rs commit), same as `v2.4.2`. `streamingfast/reth` `release/optimism-2.x` already contains
`aef8d3ef`, and its tag `op-reth-v2.4.2-fh3.2` stays the pin.

`reth-v2.5.0-fh3.1-1` (same commit as `reth-v2.5.0-fh3.2`) is on `release/reth-2.x`, based on
reth `v2.5.0` (revm 42, alloy-evm 0.38). It can't be used until op-reth itself moves to reth
`v2.5.0`; upstream draft ethereum-optimism/optimism#22495 does that port. Its Firehose commits
match `release/optimism-2.x` except a 6-line `state_hook` API adaptation in
`crates/firehose/src/inspector.rs`.

## Conflicts and how they were resolved

1. `rust/Cargo.lock` — the only conflict. Took upstream's lock, let cargo re-resolve the 106
   reth entries to `streamingfast/reth` `op-reth-v2.4.2-fh3.2`, then
   `cargo update -p alloy-evm@0.37.1 --precise 0.37.0`. Upstream's lock moved `alloy-evm` to
   `0.37.1`, at which cargo drops the `streamingfast/evm` `v0.37.0-sf` patch and traces lose
   `systemCalls`.

Auto-merged, checked by hand: `rust/op-alloy/crates/consensus/src/lib.rs` keeps our
`firehose` module next to upstream's new `decode_2718_canonical` export (#22778).

## Test results

- `cargo check -p op-reth -p reth-optimism-firehose -p alloy-op-evm -p op-alloy-consensus` — clean.
- `cargo test -p reth-optimism-firehose -p alloy-op-evm -p op-alloy-consensus` — 166 passed, 0 failed.
- `cargo test -p reth-optimism-node --test it` — 14 passed, 0 failed.
- Battlefield: not run.
