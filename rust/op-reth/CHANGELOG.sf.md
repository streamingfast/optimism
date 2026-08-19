# StreamingFast changelog

This changelog tracks changes that the StreamingFast fork applies on top of upstream
`paradigmxyz/reth` (via the SF `streamingfast/reth` fork) and on top of
`ethereum-optimism/optimism`'s `op-reth` tree.

## v2.4.2-fh3.1

Bumps the SF op-reth fork to upstream `op-reth/v2.4.2` (upstream `v2.4.1` is subsumed; 389
upstream commits). Validated against the battlefield-ethereum `op-reth-devnet` suite.

### Changed

- Reth pin moved from `streamingfast/reth` tag `v2.3.0-fh-8` to **`op-rs-aef8d3e-fh-1`**. Upstream
  repointed its reth dependency twice in this range — `paradigmxyz/reth` tag `v2.3.0` (v2.4.0) ->
  rev `f2eecc65` (v2.4.1) -> `op-rs/reth` rev `aef8d3ef92117f91455e16969f0adf5bf7c6e9e1` (v2.4.2) —
  and the published `reth-*` crates moved 0.4.1 -> 0.5.0. `op-rs-aef8d3e-fh-1` is the Firehose
  rebase of that final rev (branch `firehose/op-reth-2.4.x-fh`), so it already carries the
  genesis-block-on-empty-chain emission.
- `[patch.crates-io]` `alloy-evm` bumped to `streamingfast/evm` tag `v0.37.0-sf` (upstream moved
  `alloy-evm` 0.36 -> 0.37). Upstream's lock resolves `alloy-evm 0.37.1`, at which point cargo
  silently reports `patch ... was not used in the crate graph` and op-reth links stock `alloy-evm`
  — which routes no block-level system call (EIP-4788, EIP-2935, OP withdrawals) through the revm
  inspector, so every block trace loses its `systemCalls`. The lock pins `alloy-evm` to `0.37.0`.
  **Watch for that warning on every future bump.**
- `crates/firehose/src/engine_validator.rs` adapted to the v2.4.2 payload-validator API. Upstream
  replaced the validator's inline multiproof / state-root-task machinery with a pluggable
  `state_root_strategy` framework whose context constructors are `pub(crate)` and therefore
  unusable from this external crate; `ParallelStateRoot` was removed as well (folded into the
  now-private sparse-trie job). The Firehose validator falls back to synchronous state-root
  computation. This is a state-root *algorithm* choice only — no tracer event is emitted, dropped
  or reordered because of it. Also threads the new `state_trie_overlays` parameter through the
  validator builder, and moves the state hook from the executor to the revm `State`
  (`executor.evm_mut().db_mut().set_state_hook(..)`, alloy-evm #366).
- `crates/firehose/src/evm_config.rs`: `ConfigurePostExecEvm` gained an associated `Snapshot` type;
  `OpFirehoseEvmConfig` delegates it to the inner config.
- `Dockerfile.sf`: cargo-chef base moved to `rust-1.95` to match the workspace `rust-version`, apt
  fetches now retry (`Acquire::Retries=8`), and the builder stage receives `GIT_VERSION` /
  `GIT_COMMIT` / `GIT_DATE`, which upstream's new `op-version` crate reads to stamp
  `op-reth --version`. `sf-release.yml` supplies them from the pushed tag and commit.

## v2.4.0-fh3.1-1

### Fixed

- Bumped the SF reth fork pin in `rust/Cargo.toml` from `v2.3.0-fh-2` to `v2.3.0-fh-7`, picking up
  five Firehose fixes. No reth/revm/alloy version moves — the tags differ only in `crates/firehose`
  (plus a small `payload_validator.rs` finality fix), so the `Cargo.lock` diff is purely the git
  tag/rev of the `streamingfast/reth` source.
  - fh-7: include the SELFDESTRUCT refund when resolving an account's post-transaction balance.
    revm credits the beneficiary in place and records the move only inside its `AccountDestroyed`
    journal entry on the truly-destroyed path (EIP-6780), so a coinbase, sender or fee vault that
    received a suicide refund reported a `RewardTransactionFee` / `GasRefund` `old_balance`
    contradicting the `SuicideRefund` event emitted moments earlier. The same resolver backs the
    OP fee-vault credits in `OpPostTxExtras`.
  - fh-6: emit the value-transfer balance changes when a transaction sends value to a precompile
    and then fails; the reverted callee had its `BalanceTransfer` journal entry truncated before
    the journal walk ran.
  - fh-5: fix a call/receipt log-count mismatch panic when a native-precompile log is emitted at a
    journal index freed by a reverted opcode `LOG`.
  - fh-3: gas-bound cap on `step_keccak256`, preventing an OOM panic for operations that would
    out-of-gas anyway.
  - fh-4 is CI/packaging only (Docker build for the reth fork), no runtime effect here.

### Changed

- Merged upstream `op-reth/v2.4.0` into the Firehose branch. No reth/revm/alloy version bump
  (reth pin stays `v2.3.0-fh-2`): upstream's `[workspace.dependencies]` reth/revm/alloy pins are
  byte-for-byte identical between `op-reth/v2.3.3` and `op-reth/v2.4.0` (still `paradigmxyz/reth`
  tag `v2.3.0`), so `streamingfast/reth v2.3.0-fh-2` already covers it and no new reth tag was
  required. The merge was conflict-free — upstream's op-reth changes (`node/src/node.rs`,
  `payload/src/builder.rs` + tests, `txpool/src/lib.rs`, new `node/tests/it/custom_pool/*`) do not
  overlap the Firehose hooks, which sit in disjoint functions (`OpFirehoseEngineValidatorBuilder`/
  `OpFirehoseEvmConfig` wiring in `node.rs`, `firehose_trace_built_block` in the `no_tx_pool`
  branch of `build_payload`). The Firehose crate `crates/firehose/` is untouched upstream.
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
