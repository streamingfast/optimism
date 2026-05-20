# reth-optimism-firehose

OP Stack chain hooks for Firehose tracing, plus a `ConfigureEvm` wrapper that wires
those hooks into the staged-sync (pipeline) path.

## Why this crate exists

`reth_firehose` ships two no-op hook types (`NoPostTxExtras`, `NoPreTxAdjust`) and a
`FirehoseEvmConfig<F>` wrapper that installs them unconditionally. Chains with their
own fee distribution (OP Stack credits the L1 / base-fee / operator fee vaults via
`Journal::balance_incr`, which fires no inspector hooks) must ship their own
`ConfigureEvm` wrapper. See the author's note in
`reth/crates/firehose/src/executor.rs` for the type-system reason upstream
can't parameterize `FirehoseEvmConfig` over `Extras`/`Adjust`.

This crate is that wrapper for op-reth (mirrors `base-execution-firehose` from
base-reth).

## What's in it

- [`OpPostTxExtras`] — post-tx emitter for the three OP fee-vault balance changes
  (`BaseFeeVault` / `L1FeeVault` / `OperatorFeeVault`) with reason
  `RewardTransactionFee`.
- [`OpPreTxAdjust`] — pre-tx patcher that overrides the `TxEvent` nonce for OP
  deposit envelopes (they carry no nonce field; the effective nonce is the sender's
  pre-exec account nonce) and reclassifies the depth-0 sender balance change as
  `IncreaseMint`.
- [`OpChainHooks`] — `reth_firehose::ChainHooks` impl that installs
  [`OpPreTxAdjust`] / [`OpPostTxExtras`] in `execute_one_traced` for the pipeline /
  staged-sync path.
- [`OpFirehoseEvmConfig`] — `ConfigureEvm` wrapper whose `batch_executor` constructs
  `FirehoseBlockExecutor::new_with_chain_hooks(..., OpChainHooks)` so the pipeline
  fires the same OP hooks the live engine-API path already uses.

## Paths into the hooks

| Path                          | Where hooks are installed                                 |
| ----------------------------- | --------------------------------------------------------- |
| Staged sync (pipeline)        | `OpFirehoseEvmConfig::batch_executor` in this crate       |
| Engine API (live)             | Through `OpChainHooks::execute_one_traced` indirectly, via the SF reth fork's engine-tree which dispatches on the `ConfigureEvm` exposed by `OpExecutorBuilder` |
