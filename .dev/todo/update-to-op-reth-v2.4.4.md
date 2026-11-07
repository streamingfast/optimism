# op-reth bump to upstream `op-reth/v2.4.4` (op-reth-v2.4.4-fh3.1)

## Branch layout

- `release/op-reth-2.x` follows tagged upstream `op-reth/vX.Y.Z` releases. Tags:
  `op-reth-vX.Y.Z-fh3.N[-M]`.
- Work branch: `bump/op-reth-v2.4.4`. `op-reth/v2.4.3` is an ancestor of `op-reth/v2.4.4`
  (20 upstream commits).

## Reth dependency

Unchanged. Upstream `v2.4.4` still pins `op-rs/reth` rev `aef8d3ef`, same as `v2.4.2` and
`v2.4.3`, so `streamingfast/reth` tag `op-reth-v2.4.2-fh3.2` stays the pin. Upstream's
`Cargo.toml` diff only drops sp1 workspace members and the `cargo_metadata` / `num-format`
dependencies; no revm or alloy version moves.

## Upstream changes that touch op-reth

- `op-reth: add chain head gas and base fee metrics` (#22587): new
  `rust/op-reth/crates/node/src/head_metrics.rs`, spawned from `OpAddOns::launch_add_ons` in
  `node.rs`. It reads headers off `canonical_state_stream()` and records gauges. It does not use
  the EVM config, so it is unaffected by our `OpFirehoseEvmConfig` wrapper. New deps on the
  node crate: `reth-chain-state`, `reth-metrics`, `metrics`, `metrics-util` (dev).

Everything else is kona-sp1, op-devstack, contracts, Go services and docs.

## Conflicts and how they were resolved

None. `git merge op-reth/v2.4.4` completed on its own.

Checked by hand:
- `rust/op-reth/crates/node/src/node.rs`: our `OpFirehoseEngineValidatorBuilder` /
  `OpFirehoseEvmConfig` changes sit next to upstream's new metrics task.
- `rust/Cargo.lock`: still resolves all 106 reth entries to `streamingfast/reth`
  `op-reth-v2.4.2-fh3.2`, and `alloy-evm` stays at `0.37.0` from `streamingfast/evm`
  `v0.37.0-sf` (needed so traces keep `systemCalls`).

## Test results

- `cargo check --locked -p op-reth -p reth-optimism-firehose -p reth-optimism-node -p alloy-op-evm -p op-alloy-consensus` — clean.
- `cargo test --locked -p reth-optimism-firehose -p alloy-op-evm -p op-alloy-consensus` — 166 passed, 0 failed.
- `cargo test --locked -p reth-optimism-node --lib --test it` — 45 passed (31 lib, including the new
  head metrics test; 14 integration), 0 failed.
- Battlefield (`op-reth-devnet`): not run. Four attempts to start the devnet
  (`scripts/optimism/run_optimism_devnet.sh`) failed before `op-reth` was attached:
  - Twice, funding the test address timed out after 15s. flashblocks-rpc (port 8548) has no
    `--rollup.sequencer-http`, so the funding transaction reaches the op-rbuilder sequencer only
    via p2p gossip; when it lands before the two nodes peer, it stays in flashblocks-rpc's pool.
  - Once, bproxy started before Docker DNS could resolve `op-rbuilder`.
  - The one start that funded the address timed out after 60s because `op-reth` was still building.
