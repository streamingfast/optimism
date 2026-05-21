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
  (crates.io, tracked at `"5"` — matches what the SF reth fork resolves to).
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
- `OpFirehoseEngineValidator` + `OpFirehoseEngineValidatorBuilder` — clone of
  `reth_engine_tree::tree::BasicEngineValidator` from SF reth `firehose/2.x` with an added
  `execute_and_trace_block` Firehose-enabled twin of `execute_block`. The live engine-API
  path in `OpNode::AddOns` is now wired through this builder (replacing the upstream
  `BasicEngineValidatorBuilder`) so blocks coming in via `engine_newPayload` are executed
  through `FirehoseWrappedExecutor::with_hooks` carrying `OpPreTxAdjust` + `OpPostTxExtras`
  when the global Firehose tracer is initialized. Mirrors the approach base-reth took with
  its `base-engine-tree` crate's `BaseEngineValidator`.
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
- The `reth-optimism-firehose` crate version (`2.2.4`) tracks the public op-reth release
  line (op-reth currently tracks `v2.2.4`) rather than the internal `1.11.3` versions
  carried by sibling `reth-optimism-*` crates.
- Live engine-API tracing is delivered by the cloned `OpFirehoseEngineValidator` in this
  crate (NOT via the SF reth ExEx runner). The clone is necessary because the upstream
  `BasicEngineValidator::execute_block` is private and constructs the executor through
  `evm_config.create_executor(...)` rather than `batch_executor(...)`, so the
  `OpFirehoseEvmConfig` `batch_executor` override does not reach this path. The
  validator's `execute_and_trace_block` mirrors base-reth's
  `BaseEngineValidator::execute_and_trace_block` and wraps the OP block executor with
  `FirehoseWrappedExecutor::with_hooks(.., OpPreTxAdjust, OpPostTxExtras)`. When updating
  the SF reth fork, copy `crates/engine/tree/src/tree/payload_validator.rs` into
  `rust/op-reth/crates/firehose/src/engine_validator.rs` and review the diff.
