# StreamingFast changelog

This changelog tracks changes that the StreamingFast fork applies on top of upstream
`paradigmxyz/reth` (via the SF `streamingfast/reth` fork) and on top of
`ethereum-optimism/optimism`'s `op-reth` tree.

## Unreleased

### Fixed

- LIB (finalized block) was never advanced on the live engine-API path: the cloned
  `OpFirehoseEngineValidator` started the block tracer with `finalized = None`. It now reads the
  finalized head via `self.provider.finalized_block_num_hash()` (requiring a `BlockIdReader` bound
  on the provider) and passes it as the block's `FinalizedBlockRef`, mirroring the reth fork's
  `runner.rs`.

### Changed

- Merged upstream `op-reth/v2.3.3` into the Firehose branch. No reth/revm/alloy version bump
  (reth pin stays `v2.3.0-fh`), so the Firehose tracing code is unchanged. Adaptations:
  - Enabled the `reth-codec` feature on the firehose crate's `reth-optimism-primitives`
    dependency. Upstream made `reth-codec` opt-in
    (`feat(op-reth): make chainspec reth-codec opt-in`, #21483), which dropped the `Compact`
    impls for `OpReceipt`/`OpTxEnvelope` from the default feature set; the firehose crate now
    opts in explicitly, matching the payload/rpc/storage/post-exec-replay crates.
  - New build prerequisite from upstream: the chainspec `build.rs` loads chain configs from the
    `superchain-registry` git submodule (`load OP Mainnet/Sepolia from superchain-registry`,
    #21397; root submodule coupling, #21474). Run
    `just update-superchain-registry-submodule` before building.
- Merged upstream `op-reth/v2.3.2-rc.2` into the Firehose branch. The substantive change is the
  reth pin bump from upstream rev `7680d6d` to the released tag `v2.3.0`. Correspondingly:
  - Bumped the SF reth fork pin in `rust/Cargo.toml` from tag `v2.3.0-alpha.7680d6d-fh` to
    `v2.3.0-fh`, rebased on upstream reth `v2.3.0` (the exact tag `op-reth/v2.3.2-rc.2` pins).
  - Adapted `OpFirehoseEngineValidator` in `engine_validator.rs` to the reth `v2.3.0` API:
    - State-hook install moved from the removed `BlockExecutor::with_state_hook(...)` to
      `executor.evm_mut().db_mut().set_state_hook(...)` (both the plain and Firehose-traced
      execution paths).
    - `PayloadProcessor::spawn` gained a 6th `parallel_bal_execution: bool` argument; Firehose
      passes `false` (sequential tracing is incompatible with the parallel BAL path).
    - `CachedStateProvider::new(..).with_cache_stats(..)` replaced by
      `CachedStateProvider::new_with_mode(.., CacheFillMode::LookupOnly, Some(metrics), cache_stats)`
      (the `with_cache_stats` builder was removed and metrics is now `Option`).
    - `DeferredTrieData::pending(..)` now returns a `(DeferredTrieData, DeferredTrieDataProducer)`
      tuple; the background task uses `producer.compute_and_publish()` (was `handle.wait_cloned()`).
  - `OpFirehoseEvmConfig::post_exec_builder_for_next_block` return bound widened with
    `Result: PreRefundGasUsed` to match the upstream `ConfigurePostExecEvm` trait.
  - `alloy-op-evm` `lib.rs` import reconciled: keep Firehose's `InspectSystemCallEvm` alongside
    upstream's new `DBErrorMarker`.
- Merged upstream `op-reth/v2.3.1` into the Firehose branch. The only substantive upstream
  change is the reth pin bump (`81c0261` → `7680d6d`, a chain-state lock fix one commit
  ahead) plus the removal of `OpEvmConfig::with_sdm_enabled` (SDM is now wired via
  `sdm_post_exec_opt_in`). Correspondingly:
  - Bumped the SF reth fork pin in `rust/Cargo.toml` from tag `v2.3.0-alpha.81c0261-fh-1`
    to `v2.3.0-alpha.7680d6d-fh`, rebased on upstream reth
    `7680d6d8a931c0af4f4eed26e971596970238b54` — the exact rev `op-reth/v2.3.1` pins.
  - `OpExecutorBuilder::build_evm` no longer calls `.with_sdm_enabled(...)` (field removed
    upstream); it now wraps the plain `OpEvmConfig::new(...)` in `OpFirehoseEvmConfig`.
  - `OpFirehoseEvmConfig::post_exec_builder_for_next_block` widened its return bound to
    match the trait's new `Executor: BlockExecutor<Evm: Evm<DB: DerefMut<Target = State<DB>>>>`
    requirement; the `it/builder.rs` test drops the removed `sdm_enabled` field.
- Merged upstream `op-reth/v2.3.0` into the Firehose branch (previously based on
  `op-reth/v2.2.4`). This is a genuine merge — `op-reth/v2.3.0` is now a parent in history,
  so future `op-reth/v2.3.x` updates merge cleanly instead of re-conflicting the whole delta.
- Bumped the SF reth fork pin in `rust/Cargo.toml` from branch `firehose/2.x` to tag
  `v2.3.0-alpha.81c0261-fh-1`, built on upstream reth commit
  `81c026181e96ef33a823f3ef4d2a28940e9fa4fe` — the exact rev `op-reth/v2.3.0` pins. The tag
  also exposes `TreeState::state_trie_overlays()` publicly so `OpFirehoseEngineValidator`
  (which lives outside `reth-engine-tree`) follows the same `OverlayBuilder` construction
  path as upstream `BasicEngineValidator`.
- Bumped the `reth-optimism-firehose` crate version from `2.2.4` to `2.3.0` to track the
  public op-reth release line.
- Adapted `OpFirehoseEngineValidator` in `engine_validator.rs` to the op-reth v2.3.0 API:
  - `on_inserted_executed_block` now takes `BuiltPayloadExecutedBlock<N>` (was `ExecutedBlock<N>`)
  - `DeferredTrieData::pending` drops `anchor_hash` and `ancestors` args
  - `validate_payload` / `validate_block` now wrap return in `ValidationOutput::new(...)`
  - `validate_block_post_execution` gains a 4th `Option` arg
  - `LazyOverlay` / `get_parent_lazy_overlay` removed; replaced by
    `overlay_builder_for_parent` using `state.tree_state().state_trie_overlays()`
- Fixed `BlockHashNotFound` regression: parent blocks not yet persisted to the database
  now resolve correctly via the `StateTrieOverlayManager` carried in `EngineApiTreeState`,
  exactly mirroring upstream `BasicEngineValidator` behavior.

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

- The SF reth fork is pinned via tag `v2.3.0-alpha.81c0261-fh-1`, rebased on upstream reth
  commit `81c026181e96ef33a823f3ef4d2a28940e9fa4fe` (the same commit `op-reth/v2.3.0` uses).
- The `reth-optimism-firehose` crate version (`2.3.0`) tracks the public op-reth release
  line (op-reth currently tracks `v2.3.0`) rather than the internal `1.11.3` versions
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
