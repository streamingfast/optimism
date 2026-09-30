# op-reth bump to upstream `op-reth/v2.5.0` (op-reth-v2.5.0-fh3.1)

## Branch layout

- `release/op-reth-2.x` follows tagged upstream `op-reth/vX.Y.Z` releases. Tags:
  `op-reth-vX.Y.Z-fh3.N[-M]`.
- Work branch: `bump/op-reth-v2.5.0`. `op-reth/v2.4.4` is an ancestor of `op-reth/v2.5.0`
  (67 upstream commits).

## Reth dependency

Unchanged. Upstream `v2.5.0` still pins `op-rs/reth` rev `aef8d3ef`, same as `v2.4.2` through
`v2.4.4`, so `streamingfast/reth` tag `op-reth-v2.4.2-fh3.1-3` (branch `release/optimism-2.x`)
stays the pin and the reth fork needs no merge or new tag. Upstream's `Cargo.toml` diff only adds
kona-sp1 workspace members and bumps the sp1 crates 6.4.0 -> 6.8.0; no revm or alloy version moves.

## Upstream changes that touch op-reth

- `op-reth!: remove legacy import commands` (#22942): drops `import-op` and `import-receipts-op`,
  their codecs, and the cli crate's `reth-node-events` / `reth-optimism-evm` deps. Our Firehose
  change in `cli/src/app.rs` (wrapping `OpExecutorProvider` in `OpFirehoseEvmConfig`) is untouched.
- `op-reth: zero tx-scoped L1 fee fields on PostExec receipts` (#22941): RPC receipt fields only.
  Firehose computes the L1FeeVault credit itself in `firehose/src/extras.rs` and is unaffected.
- `alloy-op-evm,op-revm,kona: cover SDM refund settlement and gas accounting` (#22724):
  `operator_fee_charge` now returns zero before Isthmus (our `extras.rs` already gates on
  Isthmus), and `canonicalize_result_gas` also lowers `floor_gas` by the post-exec refund. The
  latter can change the gas a Firehose trace reports only for a tx where both the EIP-7623 floor
  binds and an SDM refund applies.
- `rust: tag upstream mirrors ...` (#22477): doc comments only (`UPSTREAM-MIRROR(copy)` markers).
  The `OpEvm` marker names `alloy-evm@0.37.1`; our lock stays on `0.37.0` on purpose (see below).

Everything else is kona-sp1, op-devstack, contracts, Go services and docs.

## Conflicts and how they were resolved

- `rust/op-reth/crates/cli/Cargo.toml`: upstream removed `reth-node-events` and
  `reth-optimism-evm`, we had added `reth-optimism-firehose` next to them. Kept only
  `reth-optimism-firehose`.
- `rust/Cargo.lock`: same conflict in the `reth-optimism-cli` dependency list, resolved the same
  way.

Checked by hand:
- `rust/Cargo.lock`: all reth entries still resolve to `streamingfast/reth`
  `op-reth-v2.4.2-fh3.1-3`, and `alloy-evm` stays at `0.37.0` from `streamingfast/evm`
  `v0.37.0-sf` (needed so traces keep `systemCalls`).

## Test results

- `cargo check --locked -p op-reth -p reth-optimism-firehose -p reth-optimism-node -p reth-optimism-cli -p alloy-op-evm -p op-alloy-consensus` — clean.
- `cargo test --locked -p reth-optimism-firehose -p alloy-op-evm -p op-alloy-consensus` — 167 passed, 0 failed.
- `cargo test --locked -p reth-optimism-node --lib --test it` — 45 passed (31 lib, 14 integration), 0 failed.
- `cargo test --locked -p reth-optimism-cli` — 9 passed, 0 failed.
- Battlefield (`op-reth-devnet`): not run.
