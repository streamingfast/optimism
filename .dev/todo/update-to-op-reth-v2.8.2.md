# op-reth bump to upstream `op-reth/v2.8.2` (op-reth-v2.8.2-fh3.1)

## Branch layout

- `release/op-reth-2.x` at `op-reth-v2.8.1-fh3.1` (`05c87505`). Work branch: `bump/op-reth-2.8.2`.
- `op-reth/v2.8.1` is an ancestor of `op-reth/v2.8.2`: 18 upstream commits, merged with no
  conflicts.
- Tag `op-reth-v2.8.2-fh3.1`: the newest tag on the branch with its upstream version replaced.
  The Firehose block model is unchanged, so `fh3.1` stays.

## Dependencies

Upstream changes no reth, revm, alloy-evm or op-alloy version in this range (`rust/Cargo.lock`
only gains test dependencies), so neither `streamingfast/reth` nor `streamingfast/evm` needs a
new release.

The reth and `alloy-evm` pins move from tags to the commits they point to, with the tag in a
comment:
- `streamingfast/reth` `op-reth-v2.8.1-fh3.1` -> `93fb772a` (the merge commit of reth#90). The
  lock used to resolve the PR head `1d4c8b49`; both commits have the same tree.
- `streamingfast/evm` `v0.39.0-sf` -> `52afebec` (unchanged commit).
- `firehose-tracer` stays on 5.5.0.

Nothing else in StreamingFast pins this fork except the sf-operator deployment.

## Upstream changes that touch op-reth

- `fdf142b5` interop filter metrics: `endpoint.up` stays up on `-321100` answers. Transaction
  pool only.
- `969931cd` and the gas limit commits: `OpGasLimitConfig` moves from `reth-optimism-payload` to
  `reth-optimism-txpool` (re-exported from the payload crate) and is shared with the pool, which
  now rejects transactions above the operator gas limit and starts with the chain's block gas
  limit. Pool and payload builder only; the Firehose code does not use `OpGasLimitConfig`.
- `L1_BLOCK_JOVIAN_SELECTOR` becomes `pub`. No behavior change.
- kona changes (`settlement_chain_id` in the rollup config, gossip test) are outside the op-reth
  binary.

## Firehose bug patterns

1. Serial order: none found. No change to block execution, prewarming or the engine validator.
2. Emission after acceptance: none found. No new header field or post-execution check.
3. Activations: no `OpHardfork` added, `superchain-registry` stays at `13907b9b`, and
   `alloy-op-hardforks` is unchanged, so no timestamp moved. Lagoon is still unscheduled in the
   code but Optimism now targets mid-December 2026 for OP Mainnet and Unichain; the `0x7D`
   post-exec transaction is still unsupported by `firehose-tracer` 5.5.0 and the reth mapper
   (`ACTIVATIONS.md`).
4. Fields and enums: `OpTxEnvelope`, `OpTxType`, `OpReceipt` and the header are unchanged.
   `op-revm` and the fee vault formulas are unchanged.
5. Transactions without a hash: not affected.

## Status

- Merge, CHANGELOG, ACTIVATIONS refresh: done.
- Build, unit tests, Battlefield: see the PR description.
