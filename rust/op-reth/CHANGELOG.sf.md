# StreamingFast changelog

This changelog tracks changes that the StreamingFast fork applies on top of upstream
`paradigmxyz/reth` (via the SF `streamingfast/reth` fork) and on top of
`ethereum-optimism/optimism`'s `op-reth` tree.

## Unreleased

### Added

- Switched workspace `reth-*` git dependencies in `rust/Cargo.toml` from
  `paradigmxyz/reth` (commit `88505c7fcbfdebfd3b56d88c86b62e950043c6c4`) to
  `streamingfast/reth.git` branch `firehose/2.x`. The SF fork's branch is rebased on the
  same upstream commit and ships the `reth-firehose` crate used by this integration.
- New workspace dependencies: `reth-firehose` (from the SF reth fork) and `firehose-tracer`
  (crates.io, pinned to `=5.0.0`).
- New crate `reth-optimism-firehose` at `rust/op-reth/crates/firehose/`. Mirrors
  `base-execution-firehose` from base-reth and contains:
  - `OpPostTxExtras` — emits the three OP fee-vault balance changes (`BaseFeeVault`,
    `L1FeeVault`, `OperatorFeeVault`) with reason `RewardTransactionFee` after each
    transaction (revm's post-execution phase fires no inspector hooks, so without this the
    credits would be invisible to the tracer).
  - `OpPreTxAdjust` — patches the per-tx `TxEvent` nonce for OP deposit transactions
    (which carry no nonce field) and reclassifies the depth-0 sender balance change as
    `IncreaseMint` (reason 18) instead of the default `GasBuy` (reason 7).
  - `OpChainHooks` — `reth_firehose::ChainHooks` impl wiring both hooks into
    `FirehoseWrappedExecutor::with_hooks`.
  - `OpFirehoseEvmConfig<F>` — `ConfigureEvm` wrapper whose `batch_executor` constructs
    `FirehoseBlockExecutor::new_with_chain_hooks(...)` with `OpChainHooks`. The wrapper
    also delegates `ConfigureEngineEvm` and `ConfigurePostExecEvm` through to the inner
    config.
- `OpExecutorBuilder` (`rust/op-reth/crates/node/src/node.rs`) now wraps `OpEvmConfig`
  with `OpFirehoseEvmConfig`, so the pipeline / staged-sync path automatically routes
  through the SF Firehose executor with the OP chain hooks installed.
- CLI `components` lambda (`rust/op-reth/crates/cli/src/app.rs`) is wrapped in
  `OpFirehoseEvmConfig::new(...)` to keep the type of the EVM exposed by node-builder
  consistent with what the `stage` / `re-execute` CLI commands expect.
- Added feature-gated `reth_firehose::mapper::SignatureFields` impl for
  `OpTxEnvelope` in `op-alloy-consensus` (new `firehose` feature, dep on `reth-firehose`,
  enabled transitively by `reth-optimism-firehose`). Lives in this crate because Rust's
  orphan rules forbid the impl elsewhere.

### Notes

- The SF reth fork's `firehose/2.x` branch tracks upstream reth `v2.2.0`
  (`88505c7fcbfdebfd3b56d88c86b62e950043c6c4`). A `v2.x.y-fh-N` tag will replace the
  branch pin once cut.
- The live engine-API path in `streamingfast/reth` `firehose/2.x` does not yet route
  through `ConfigureEvm::batch_executor` for tracing (the engine-tree builds an executor
  via `create_executor` directly); SF reth ships live Firehose support via the
  `firehose` `exex`. When the SF reth team wires live tracing through `ConfigureEvm`, this
  op-reth integration will inherit it for free via `OpFirehoseEvmConfig`.
