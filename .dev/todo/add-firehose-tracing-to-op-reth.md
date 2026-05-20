# Add Firehose Tracing to op-reth

mode: feature
state: review
root_git: /Users/maoueh/work/sf/op-reth
worktree: .worktrees/feature/add-firehose-tracing-to-op-reth
branch: feature/add-firehose-tracing-to-op-reth
target_branch: firehose/2.x

> **Resume protocol:** read **Dev Feedback** and the **State Tracker** below first, then jump to the
> step marked `Current`. Ensure that you are in the correct worktree and branch according to preamble here. Update current with Developer feedback and update the tracker after every meaningful change.
> Do not mutate completed steps; append a new entry instead.

---

## Initial Description

We have a base-reth code which is at https://github.com/streamingfast/base/tree/firehose/0.x and is probably very similar to how op-reth would be modified to support Firehose Tracing.

Investigate the difference between https://github.com/streamingfast/base/tree/firehose/0.x and v0.8.0 on our https://github.com/streamingfast/base fork (fork off https://github.com/base/base). Determine the set of changes we needed to do to add Firehose Tracing on base-reth and let's do have the same thing for op-reth.

## Dev Feedback

1. _Important_ While the base fork use reth implementation tag `v1.11.4-fh-1`, this is not valid for op-reth which is tracking instead v2 of reth. So the work must update to `branch firehose/2.x` which contains op-reth v2.2.0 which is commit 88505c7fcbfdebfd3b56d88c86b62e950043c6c4

### Implementor Notes

1. **Orphan-rule workaround for `SignatureFields`:** The `reth_firehose::mapper::SignatureFields` impl
   for `OpTxEnvelope` had to live in `op-alloy-consensus` (the crate that defines `OpTxEnvelope`)
   because neither `SignatureFields` (foreign — `reth-firehose`) nor `OpTxEnvelope` (foreign —
   `op-alloy-consensus`) is local to our new `reth-optimism-firehose` crate. Added behind an
   optional `firehose` feature on `op-alloy-consensus` so the published crate does not
   unconditionally pull in `reth-firehose`. `reth-optimism-firehose` enables the feature.

2. **CLI `components` lambda updated:** `rust/op-reth/crates/cli/src/app.rs`'s `components` lambda
   (used by `Stage` and `ReExecute` CLI subcommands) had to be wrapped in
   `OpFirehoseEvmConfig::new(...)` because the node-builder now produces
   `OpFirehoseEvmConfig<OpEvmConfig>` and `CliComponentsBuilder` requires matching types.
   Added `reth-optimism-firehose` to the cli crate's deps.

3. **`OpExecutorProvider` is a `pub type` alias** for `OpEvmConfig`, so the CLI components wrap
   `OpExecutorProvider::optimism(spec.clone())` (same as before) with the firehose wrapper.

4. **Test helper updated:** `rust/op-reth/crates/node/tests/it/builder.rs` destructures
   `OpExecutorBuilder::default().build_evm(ctx).await?.inner` (added `.inner`) to unwrap the
   new `OpFirehoseEvmConfig` layer.

5. **`ConfigurePostExecEvm` is op-reth-specific** (not in the SF reth firehose crate) — added a
   delegating impl on `OpFirehoseEvmConfig<F>` so the OP payload-builder pipeline that uses
   `post_exec_executor_for_block` / `post_exec_builder_for_next_block` still works. The
   firehose wrapper only intercepts `batch_executor`; all post-exec paths pass through unchanged.

6. **Live engine-API path is not yet wired through `ConfigureEvm` upstream:** Inspected
   `streamingfast/reth` (`firehose/2.x`) engine-tree (`crates/engine/tree/src/tree/payload_validator.rs`).
   Its `execute_block` builds the executor via `evm_config.create_executor(evm, ctx)` directly,
   not via `batch_executor` — so the live path does NOT inherit the firehose hooks from our
   wrapper today. SF reth ships live Firehose support via the separate `firehose` `exex`
   (`reth_firehose::run_exex`) but that runner is currently mostly stubbed. When SF reth wires
   live tracing through `ConfigureEvm`, our `OpFirehoseEvmConfig` will pick it up automatically.
   No op-reth-side change is needed today; surfaced here for visibility.

7. **`firehose-tracer` version:** Pinned to `=5.0.0` per the prompt; matches the version the SF
   reth fork's workspace resolves to.

8. **Workspace `version.workspace = true` not available:** the workspace `[workspace.package]`
   does not declare a `version` field; existing op-reth crates use literal `version = "1.11.3"`.
   The new `reth-optimism-firehose` crate follows the same convention.

## Spec & Implementation

### Summary

Add Firehose Tracing support to `op-reth` — the OP Stack execution client built on reth — using the same approach applied for `base-reth`. This enables Firehose 3.0 block production from a Reth-based OP Stack node, producing block version 5 of the Firehose Ethereum Block protobuf model.

The integration hooks into two execution paths exposed by the StreamingFast `reth-firehose` crate:

1. **Live / engine-API path** — driven by `FirehoseEvmConfig::batch_executor` / engine-tree validation in `streamingfast/reth`.
2. **Staged-sync / pipeline path** — driven by `FirehoseBlockExecutor` in `streamingfast/reth`.

Both paths require OP Stack-specific chain hooks:

- **`OpPostTxExtras`** (implements `reth_firehose::PostTxExtras`) — emits the three OP fee vault balance changes (BaseFeeVault, L1FeeVault, OperatorFeeVault) that happen inside revm's post-execution phase where no inspector hooks fire.
- **`OpPreTxAdjust`** (implements `reth_firehose::PreTxAdjust`) — patches the `TxEvent` nonce for OP deposit transactions (which carry no nonce field) by reading the sender's pre-execution nonce from the DB and overrides the root balance-change reason to `IncreaseMint`.

Both hooks are bundled in an `OpChainHooks` (`reth_firehose::ChainHooks`) so the same wiring serves both the live path (via an `OpFirehoseEvmConfig<F>` wrapper around `OpEvmConfig`) and the pipeline path (via `FirehoseBlockExecutor::new_with_chain_hooks(...)`).

### Background: What base-reth did (the reference implementation)

The `firehose/0.x` branch of `streamingfast/base` differs from the upstream `base/base` (tag `v0.8.0`) in these ways:

1. Switched all reth dependencies from `paradigmxyz/reth` to `streamingfast/reth` (the reth v1.x line, tag `v1.11.4-fh-1`). The fork adds the `reth-firehose` crate with the core Firehose instrumentation scaffolding.
2. Added the `firehose-tracer` crate as a workspace dependency (crates.io), pinned to the version the SF fork's workspace pulls in.
3. Created `crates/execution/firehose` (`base-execution-firehose`) with:
   - `OpPostTxExtras` — post-tx hook emitting the three OP fee vault balance changes.
   - `OpPreTxAdjust` — pre-tx hook patching deposit-tx nonces and reclassifying sender balance-change reason to `IncreaseMint`.
   - `OpChainHooks` — `ChainHooks<F>` implementation wiring both hooks into the staged-sync path.
   - `OpFirehoseEvmConfig<F>` — `ConfigureEvm` wrapper that overrides `batch_executor` to use `FirehoseBlockExecutor::new_with_chain_hooks(inner, db, OpChainHooks)`.
4. Wired the live execution path so that whatever code instantiates `FirehoseWrappedExecutor` for the engine-API path uses `FirehoseWrappedExecutor::with_hooks(inner, withdrawals, OpPreTxAdjust, OpPostTxExtras)` instead of the no-hooks default.
5. Modified the node builder so the EVM config exposed by the executor builder is wrapped with `OpFirehoseEvmConfig`, ensuring the batch executor automatically gets Firehose instrumentation.

The same five-step pattern is applied here, adapted to reth v2.x and the op-reth crate layout.

### Scope

**In scope:**

- Switch all reth git dependencies in `rust/Cargo.toml` from `paradigmxyz/reth` (commit `88505c7fcbfdebfd3b56d88c86b62e950043c6c4`) to `streamingfast/reth.git` branch `firehose/2.x`. The SF fork's `firehose/2.x` branch is rebased on the same commit so this is a drop-in fork switch, not a version bump.
- Add `reth-firehose = { git = "https://github.com/streamingfast/reth.git", branch = "firehose/2.x" }` to the workspace deps.
- Create new crate `rust/op-reth/crates/firehose/` (`reth-optimism-firehose`) with:
  - `OpPostTxExtras` (post-tx fee vault emission, with L1 cost-cache invalidation).
  - `OpPreTxAdjust` (deposit-tx nonce patch + `IncreaseMint` balance-reason override).
  - `OpChainHooks` (`ChainHooks<F>` impl).
  - `OpFirehoseEvmConfig<F>` (`ConfigureEvm` wrapper that calls `FirehoseBlockExecutor::new_with_chain_hooks(..., OpChainHooks)`).
- Modify `rust/op-reth/crates/node/src/node.rs` — `OpExecutorBuilder::build_evm` returns `OpFirehoseEvmConfig<OpEvmConfig<...>>` instead of bare `OpEvmConfig`.
- Wire the live engine-API execution path so it installs `OpPreTxAdjust` / `OpPostTxExtras` via `FirehoseWrappedExecutor::with_hooks(inner, withdrawals, OpPreTxAdjust, OpPostTxExtras)`. The exact integration point depends on what `streamingfast/reth` (`firehose/2.x`) exposes for OP integrators; the implementor must locate it during step 2 of the implementation plan.
- Register the new crate in `rust/Cargo.toml` workspace members and add the workspace path alias.
- Add `rust/op-reth/CHANGELOG.sf.md` (StreamingFast changelog) documenting the integration.

**Out of scope:**

- Changes to `reth-firehose` itself (lives in `streamingfast/reth`).
- Changes to `firehose-tracer` (lives on crates.io).
- Battlefield / integration test suite execution (separate task after implementation).
- Migrating the reth fork from `firehose/2.x` branch to a tagged version (the SF team will cut a `v2.2.0-fh-N` tag at a later stage; until then the branch is the canonical reference).

### Design

#### `streamingfast/reth` API surface (verified on `firehose/2.x`)

The SF fork's `reth-firehose` crate (verified by inspecting `crates/firehose/src/{lib,executor,prelude}.rs` on branch `firehose/2.x`, HEAD `68241c23da6daabfe44c28ad4d4fe73f92ed1551`) exports:

- `trait PostTxExtras<E> where E: reth_evm::Evm, E::Inspector: FirehoseInspectorApi` with `fn emit_post_tx_extras(&self, evm: &mut E, gas_used: u64, base_fee: u64)`.
- `trait PreTxAdjust<E>` with `fn adjust_tx_event(&self, evm: &mut E, tx_event: &mut firehose_tracer::types::TxEvent, sender: Address)`.
- `struct FirehoseWrappedExecutor<Inner, Extras = NoPostTxExtras, Adjust = NoPreTxAdjust>` with `const fn with_hooks(inner, withdrawals: Option<Withdrawals>, adjust: Adjust, extras: Extras) -> Self` — **note parameter order is `(adjust, extras)`**, not `(extras, adjust)`.
- `trait ChainHooks<F: ConfigureEvm>: Send + Sync + Clone + Unpin + 'static` with one method `execute_one_traced<DB>(&self, evm_config: &F, db: &mut State<DB>, block, tracer) -> Result<BlockExecutionResult<...>, BlockExecutionError>`.
- `struct NoChainHooks` — default Ethereum-mainnet impl. Its body delegates to the free function `run_wrapped_block::<F, DB, NoPostTxExtras, NoPreTxAdjust, _>(evm_config, db, block, tracer, NoPreTxAdjust, NoPostTxExtras)`.
- `struct FirehoseBlockExecutor<F, DB, H = NoChainHooks>` with `new(strategy_factory, db)` (mainnet) and `new_with_chain_hooks(strategy_factory, db, hooks)` (OP).
- `struct FirehoseEvmConfig<F>` — a generic `ConfigureEvm` wrapper whose `batch_executor<DB>` returns `FirehoseBlockExecutor::new(self.inner.clone(), db)` — i.e. **no chain hooks**. OP needs its own wrapper that calls `new_with_chain_hooks` with `OpChainHooks`.
- `pub fn run_wrapped_block<F, DB, Extras, Adjust, G>(...)` — convenience for `ChainHooks` impls; takes adjust + extras and wraps `FirehoseWrappedExecutor::with_hooks` for one block.

#### Dependency change: `paradigmxyz/reth` → `streamingfast/reth`

The current op-reth workspace (`rust/Cargo.toml`, lines 322–~460) pins every `reth-*` crate to:

```toml
git = "https://github.com/paradigmxyz/reth", rev = "88505c7fcbfdebfd3b56d88c86b62e950043c6c4"
```

`88505c7fcbfdebfd3b56d88c86b62e950043c6c4` is the upstream reth `v2.2.0` commit and is the **base** of the SF fork's `firehose/2.x` branch — confirmed by the dev feedback. The fork switch is therefore a drop-in change of the git URL and reference:

```toml
git = "https://github.com/streamingfast/reth.git", branch = "firehose/2.x"
```

All ~140 `reth-*` workspace declarations get this same change (URL + `rev = "..."` → `branch = "firehose/2.x"`).

Two new workspace deps are added in the same edit:

```toml
reth-firehose = { git = "https://github.com/streamingfast/reth.git", branch = "firehose/2.x" }
```

#### New crate: `rust/op-reth/crates/firehose/` (`reth-optimism-firehose`)

This crate is the direct op-reth analogue of `base-execution-firehose`. The API is the same since Base and OP Stack share the OP Stack EVM execution model (both use `op-revm`, `OpHandler`, the same three fee vaults, deposit transactions, etc.).

**Files:**

- `Cargo.toml` — `name = "reth-optimism-firehose"`, `version.workspace = true`, `edition.workspace = true`, etc. Dependencies:
  - From the SF reth fork (via the new workspace aliases): `reth-evm`, `reth-revm`, `reth-errors`, `reth-firehose`, `reth-primitives-traits`, `reth-optimism-evm` (path), `reth-optimism-primitives` (path).
  - From crates.io: `firehose-tracer`, `alloy-evm`, `alloy-consensus`, `alloy-primitives`, `op-revm`, `alloy-op-evm`.
- `src/lib.rs` — module declarations and re-exports of `OpPostTxExtras`, `OpPreTxAdjust`, `OpChainHooks`, `OpFirehoseEvmConfig`.
- `src/extras.rs` — `OpPostTxExtras` (`impl<E> PostTxExtras<E> for OpPostTxExtras where E: reth_evm::Evm<...>`) and `OpPreTxAdjust` (`impl<E> PreTxAdjust<E> for OpPreTxAdjust ...`). Logic mirrors base-reth's `extras.rs`:
  - Post-tx: read EIP-1559 base fee + L1 fee + (post-Isthmus) operator fee, emit `Reason::RewardTransactionFee` balance-change entries to `BaseFeeVault → L1FeeVault → OperatorFeeVault` in ascending-address order, skip zero amounts, and **call `l1.clear_tx_l1_cost()` after each tx** to prevent L1-cost cache poisoning across tx boundaries.
  - Pre-tx: for `TxDeposit`, read the sender's pre-execution nonce from `evm.db_mut().basic(sender)` and write it into `tx_event.nonce`; also call `evm.inspector_mut().tracer_mut().set_root_balance_reason(Reason::IncreaseMint)` so the sender's balance increase is correctly classified.
  - The hook bodies are essentially identical to base-reth's, with `BaseEvm` replaced by op-reth's `OpEvm` (from `op-revm` / `alloy-op-evm`) and `Predeploys` constants pulled from `op-alloy-consensus` or `reth-optimism-primitives`.
- `src/evm_config.rs` — `OpChainHooks` (`impl<F: ConfigureEvm<Primitives = OpPrimitives>> ChainHooks<F> for OpChainHooks`) delegates to `run_wrapped_block::<F, DB, OpPostTxExtras, OpPreTxAdjust, _>(evm_config, db, block, tracer, OpPreTxAdjust, OpPostTxExtras)`. `OpFirehoseEvmConfig<F>` is a near-clone of `reth_firehose::FirehoseEvmConfig<F>` whose only meaningful difference is the `batch_executor<DB>` impl: it returns `FirehoseBlockExecutor::new_with_chain_hooks(self.inner.clone(), db, OpChainHooks)` instead of `FirehoseBlockExecutor::new(...)`.
- `README.md` — short description of crate purpose.

**Important type-bound notes:**

- The `ChainHooks` trait requires `Send + Sync + Clone + Unpin + 'static` on the implementor — make `OpChainHooks` a zero-sized `#[derive(Default, Debug, Clone, Copy)]` unit struct.
- The `PostTxExtras` / `PreTxAdjust` trait blanket-bound `E::Inspector: FirehoseInspectorApi` is satisfied automatically by `FirehoseWrappedExecutor`'s inspector wiring; the OP impls do not need to spell that out beyond the generic `E: reth_evm::Evm` bound.

#### Modification: `OpExecutorBuilder` in `rust/op-reth/crates/node/src/node.rs`

Current (lines 1028–1041):

```rust
impl<Node> ExecutorBuilder<Node> for OpExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec: OpHardforks, Primitives = OpPrimitives>>,
{
    type EVM =
        OpEvmConfig<<Node::Types as NodeTypes>::ChainSpec, <Node::Types as NodeTypes>::Primitives>;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = OpEvmConfig::new(ctx.chain_spec(), OpRethReceiptBuilder::default())
            .with_sdm_enabled(self.sdm_enabled);

        Ok(evm_config)
    }
}
```

Becomes:

```rust
impl<Node> ExecutorBuilder<Node> for OpExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec: OpHardforks, Primitives = OpPrimitives>>,
{
    type EVM = OpFirehoseEvmConfig<
        OpEvmConfig<<Node::Types as NodeTypes>::ChainSpec, <Node::Types as NodeTypes>::Primitives>,
    >;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = OpEvmConfig::new(ctx.chain_spec(), OpRethReceiptBuilder::default())
            .with_sdm_enabled(self.sdm_enabled);

        Ok(OpFirehoseEvmConfig::new(evm_config))
    }
}
```

(`OpFirehoseEvmConfig` imported from the new `reth_optimism_firehose` crate; `reth-optimism-firehose.workspace = true` added to `rust/op-reth/crates/node/Cargo.toml`.)

#### Modification: live engine-API execution path

In op-reth, the engine-API live execution path runs through `reth-engine-tree`'s payload validator. With the SF fork's `firehose/2.x` branch, the live execution flow is driven by the `ConfigureEvm` exposed by the executor builder — once `OpExecutorBuilder` returns `OpFirehoseEvmConfig<...>`, the engine-tree path automatically picks it up via `batch_executor` and therefore inherits `OpChainHooks`. **In other words, the executor-builder change above covers both paths.**

There is however a second integration point: if the SF fork's engine-tree (in `firehose/2.x`) builds `FirehoseWrappedExecutor` directly for the per-payload execute-and-trace step (rather than going through `batch_executor`), it currently uses `FirehoseWrappedExecutor::new` (no hooks). The implementor must:

1. After switching dependencies (step 1) and inspecting the SF fork's `crates/engine/tree` source, locate any direct call to `FirehoseWrappedExecutor::new(...)`.
2. If such a call exists on the OP-relevant code path, propose a follow-up change either upstream in `streamingfast/reth` (preferred) or here in op-reth (only if the entry point is downstream-overridable).

If the engine-tree path goes solely through `ConfigureEvm`, no further op-reth change is required — the hooks flow through automatically. The implementor must verify this during step 6.

### Key Technical Notes from base-reth (must carry over to op-reth)

1. **L1 cost cache invalidation in `OpPostTxExtras`:** After computing `calculate_tx_l1_cost`, the hook must call `l1.clear_tx_l1_cost()` to prevent the cached value leaking into the next transaction's L1 cost calculation (state-root mismatch otherwise).

2. **OperatorFeeVault is only active post-Isthmus:** the operator fee computation is gated on `spec.is_enabled_in(OpSpecId::ISTHMUS)` and emits `U256::ZERO` (which is skipped — see #3) otherwise.

3. **Zero-amount vault credits are skipped:** if `amount.is_zero()`, the vault entry is not emitted, avoiding phantom balance changes.

4. **Vault emission order:** `BaseFeeVault → L1FeeVault → OperatorFeeVault` (address-ascending order, matching geth instrumentation), NOT the handler's internal `L1 → base → operator` journal order.

5. **Deposit tx nonce:** `TxDeposit::nonce()` returns literal `0`. `OpPreTxAdjust` reads the sender's actual account nonce from the DB and writes it into the `TxEvent`.

6. **Deposit tx balance reason:** the sender balance change for a deposit is `IncreaseMint` (reason 18), not `GasBuy` (reason 7). `OpPreTxAdjust` calls `inspector.tracer_mut().set_root_balance_reason(Reason::IncreaseMint)` to override the default.

### Implementation Plan

1. **Switch reth fork + add firehose-tracer / reth-firehose deps** (`rust/Cargo.toml`)
   - Replace all `git = "https://github.com/paradigmxyz/reth", rev = "88505c7fcbfdebfd3b56d88c86b62e950043c6c4"` occurrences (~140 lines, starting at line 322) with `git = "https://github.com/streamingfast/reth.git", branch = "firehose/2.x"`. Keep `default-features = false` and any other per-crate options intact.
   - Add the workspace dep `reth-firehose = { git = "https://github.com/streamingfast/reth.git", branch = "firehose/2.x" }`.
   - Run `cargo check --workspace -p reth-optimism-node` (from `rust/`) to confirm the fork switch compiles. If a reth-\* sub-crate is referenced by op-reth but missing from the SF fork on `firehose/2.x`, surface that immediately and stop for guidance.

2. **Confirm SF reth fork API surface** (no code change yet — read-only inspection)
   - Verify the exported names match what this spec assumes: `reth_firehose::{ChainHooks, FirehoseBlockExecutor, FirehoseEvmConfig, FirehoseWrappedExecutor, NoChainHooks, NoPostTxExtras, NoPreTxAdjust, PostTxExtras, PreTxAdjust, run_wrapped_block, FirehoseBlockTracer, init_tracer, tracer, is_tracer_initialized}`.
   - Confirm `FirehoseWrappedExecutor::with_hooks` signature: `(inner, withdrawals, adjust, extras)`.
   - Confirm `FirehoseBlockExecutor::new_with_chain_hooks(strategy_factory, db, hooks)`.
   - Identify op-reth EVM types needed for hook generics: `OpEvm` (from `op-revm` / `alloy-op-evm`), `OpEvmFactory`, `OpHaltReason`, `OpSpecId`. Confirm the three predeploy addresses from `op-alloy-consensus` (or `reth-optimism-primitives`).

3. **Create `rust/op-reth/crates/firehose/` crate**
   - `Cargo.toml`: name `reth-optimism-firehose`, workspace inherits.
   - `src/lib.rs`: module declarations + re-exports.
   - `src/extras.rs`: `OpPostTxExtras` + `OpPreTxAdjust` — adapt directly from base-reth's `extras.rs`, substituting OP types.
   - `src/evm_config.rs`: `OpChainHooks` + `OpFirehoseEvmConfig<F>` — adapt from base-reth's `evm_config.rs`. The `ConfigureEvm` impl is a near-clone of `reth_firehose::FirehoseEvmConfig<F>`'s impl, differing only in `batch_executor` (calls `new_with_chain_hooks(... , OpChainHooks)`).
   - `README.md`: brief description.
   - Register in `rust/Cargo.toml`:
     - Add `"op-reth/crates/firehose"` to `members`.
     - Add `reth-optimism-firehose = { path = "op-reth/crates/firehose/" }` to workspace deps (alongside the other `reth-optimism-*` path aliases at lines 284–298).

4. **Wire `reth-optimism-firehose` into op-reth node**
   - Add `reth-optimism-firehose.workspace = true` to `rust/op-reth/crates/node/Cargo.toml`.
   - In `rust/op-reth/crates/node/src/node.rs`:
     - Add `use reth_optimism_firehose::OpFirehoseEvmConfig;`.
     - Update `OpExecutorBuilder::EVM` associated type and `build_evm` as shown in the Design section.

5. **Verify build** — from `rust/`:
   - `cargo check --workspace` — confirm the new crate compiles in workspace context.
   - `cargo build -p reth-optimism-node` — confirm node builds end-to-end.
   - `cargo build -p op-reth` (the binary) — confirm the executable links.

6. **Verify live (engine-API) execution path**
   - Inspect the SF reth fork's `crates/engine/tree` (via the cargo source mirror or git ls-tree on `firehose/2.x`) to confirm the engine-tree flow uses `ConfigureEvm::batch_executor` for payload execution. If it does, no additional op-reth change is needed — the hooks are applied transparently.
   - If the engine-tree path bypasses `batch_executor` and constructs `FirehoseWrappedExecutor::new` directly for OP, file an upstream fix in `streamingfast/reth` so the live path also accepts chain hooks (e.g., a `WithChainHooks` extension on `ConfigureEvm`).
   - Either outcome must be recorded in `CHANGELOG.sf.md` and the State Tracker.

7. **Add `CHANGELOG.sf.md`** at `rust/op-reth/CHANGELOG.sf.md`
   - One `## Unreleased` section, `### Added` bullet listing:
     - Switched workspace reth deps to `streamingfast/reth` branch `firehose/2.x`.
     - New `reth-optimism-firehose` crate with OP-specific Firehose chain hooks.
     - `OpExecutorBuilder` now wraps `OpEvmConfig` with `OpFirehoseEvmConfig`.

### Decisions & Assumptions

| Decision/Assumption                                        | Rationale                                                                                                                                                                                                                                         |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Use `streamingfast/reth` branch `firehose/2.x` (not a tag) | The SF fork has not yet cut a tag aligned with reth v2; the branch tracks v2.2.0 commit `88505c7fcbfdebfd3b56d88c86b62e950043c6c4`, which matches op-reth's current pin exactly. Branch-based dep is acceptable until a `v2.x.y-fh-N` tag exists. |

| Name new crate `reth-optimism-firehose` at path `rust/op-reth/crates/firehose/` | Follows op-reth crate naming convention (`reth-optimism-*`); mirrors base-reth's `base-execution-firehose`. |
| OP Stack hooks for base-reth and op-reth are functionally identical | Both clients use `op-revm`, the same `OpHandler`, same fee vaults, same deposit-tx semantics — they share the OP Stack EVM layer. |
| Emit block version 5 (inherited from `reth-firehose`) | The model version is owned by `reth-firehose` (and `firehose-tracer`). Version 5 removes GasChanges, fixes CodeChange no-ops, fixes self-destruct ordering — consistent with the base-reth release. |
| `OpFirehoseEvmConfig` is a near-clone of `FirehoseEvmConfig`, not a wrapper | The only meaningful difference is `batch_executor`; the generic `FirehoseEvmConfig` cannot be parameterized over `ChainHooks` because of HRTB constraints documented in the SF fork's `executor.rs` (the `for<'a> Extras: PostTxExtras<...>` bound is not expressible at the generic-config layer). |
| Live engine-API path is covered transparently if engine-tree goes through `ConfigureEvm::batch_executor` | This is the default in upstream reth v2; verification is part of implementation step 6. |
| `target_branch: firehose/2.x` (op-reth fork's working branch) | The op-reth repo's current HEAD branch is `firehose/2.x` and that's where SF integration work is tracked; merging back to `develop` (the public default branch) is out of scope for this task. |
| `root_git: /Users/maoueh/work/sf/op-reth` | Correct repo root path (the previous value `/Users/maoueh/work/sf/optimism/rust` was a leftover from the base-reth task template). |

---

## State Tracker

**Last Updated:** 2026-05-20
**Current Step:** Implementation complete — ready for developer review
**Status:** All build verifications pass (`cargo build -p reth-optimism-node`, `cargo build -p op-reth`, `cargo check -p reth-optimism-node --all-targets`). Awaiting review.

| Step                                                                        | Status  | Notes                                                                                                                                                                                                                                                              |
| --------------------------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Phase 1 — Contextual Understanding                                          | Done    | Explored op-reth crates structure, node.rs, evm, engine; fetched base-reth firehose branch.                                                                                                                                                                        |
| Phase 2 — Gap Analysis                                                      | Done    | Key gap: reth fork switch needed; OP type-name mappings TBD at implementation time.                                                                                                                                                                                |
| Phase 3 — Challenging Dialogue                                              | Skipped | No ambiguous decisions requiring user input; base-reth is a direct reference.                                                                                                                                                                                      |
| Phase 4 — Specification Writing                                             | Done    | Initial spec with 8-step implementation plan written.                                                                                                                                                                                                              |
| Phase 5 — Spec Review (initial)                                             | Done    | Developer flagged the spec referenced reth v1.x (tag `v1.11.4-fh-1`) but op-reth tracks v2; SF fork branch is `firehose/2.x` (commit `68241c23...`, rebased on op-reth's pinned `88505c7f...`).                                                                    |
| Phase 4 — Specification Writing (replan, v2)                                | Done    | Updated dep references to `streamingfast/reth.git` branch `firehose/2.x`; verified API surface (`ChainHooks`, `FirehoseEvmConfig`, `with_hooks` arg order); corrected `root_git` and `target_branch` preamble; confirmed `with_hooks` order is `(adjust, extras)`. |
| Phase 5 — Spec Review (replan)                                              | Done    | Developer approved spec; ready state set.                                                                                                                                                                                                                          |
| Impl Step 1 — Switch reth fork + add firehose deps (`rust/Cargo.toml`)      | Done    | All ~70 `paradigmxyz/reth, rev=88505c7f...` entries replaced with `streamingfast/reth.git, branch="firehose/2.x"`. Added `reth-firehose` and `firehose-tracer = "=5.0.0"` workspace deps. Added `reth-optimism-firehose` path workspace dep and member.             |
| Impl Step 2 — Confirm SF reth fork API surface                              | Done    | Verified: `FirehoseWrappedExecutor::with_hooks(inner, withdrawals, adjust, extras)`, `FirehoseBlockExecutor::new_with_chain_hooks`, `ChainHooks` trait. OP types confirmed: `OpEvm`, `OpEvmFactory<OpTx>` from `alloy-op-evm`, `OpSpecId` from `op-revm`.            |
| Impl Step 3 — Create `reth-optimism-firehose` crate                         | Done    | `Cargo.toml`, `src/lib.rs`, `src/extras.rs` (`OpPostTxExtras`, `OpPreTxAdjust`), `src/evm_config.rs` (`OpChainHooks`, `OpFirehoseEvmConfig`), `README.md`. Implements `ConfigureEvm`, `ConfigureEngineEvm`, and `ConfigurePostExecEvm` for the wrapper.              |
| Impl Step 3.5 — `SignatureFields for OpTxEnvelope` (orphan workaround)      | Done    | Added feature-gated impl in `op-alloy-consensus` (new `firehose` feature). See Implementor Note #1.                                                                                                                                                                |
| Impl Step 4 — Wire into op-reth node (`OpExecutorBuilder`)                  | Done    | `OpExecutorBuilder::EVM` now `OpFirehoseEvmConfig<OpEvmConfig<...>>`; `build_evm` returns the wrapped config. Imported `reth_optimism_firehose::OpFirehoseEvmConfig`. Added `reth-optimism-firehose` to node Cargo.toml.                                              |
| Impl Step 4b — Wire CLI components (`reth-optimism-cli`)                    | Done    | `app.rs`'s `components` lambda wrapped with `OpFirehoseEvmConfig::new(...)`. Test helper `node/tests/it/builder.rs` adjusted to destructure `.inner` of the wrapper.                                                                                                |
| Impl Step 5 — Verify build                                                  | Done    | `cargo check -p reth-optimism-firehose`, `cargo build -p reth-optimism-node`, `cargo build -p op-reth`, `cargo check -p reth-optimism-node --all-targets` all pass. Workspace-wide `cargo check --workspace` fails only on `kona-hardforks` build script (unrelated — looks for a file outside our worktree).  |
| Impl Step 6 — Verify live engine-API execution path                         | Done    | Inspected SF reth's engine-tree (`payload_validator.rs::execute_block`); it constructs the executor via `create_executor` directly, not `batch_executor`, so live firehose tracing is NOT inherited automatically. See Implementor Note #6 — no op-reth-side change needed; SF reth handles live via the `exex` runner.   |
| Impl Step 7 — Add `CHANGELOG.sf.md`                                         | Done    | `rust/op-reth/CHANGELOG.sf.md` documents the integration under `## Unreleased`.                                                                                                                                                                                    |
| Implementation complete                                                     | Current | All steps complete; build verifications pass; task ready for developer review.                                                                                                                                                                                     |
