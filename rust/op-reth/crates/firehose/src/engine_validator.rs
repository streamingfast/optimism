//! OP Stack-specific engine validator that mirrors
//! [`reth_engine_tree::tree::OpFirehoseEngineValidator`] and adds the Firehose live-tracing path on
//! the engine-API flow.
//!
//! # Payload validation flow
//!
//! [`BasicEngineValidator::validate_block_with_state`] is the engine-side entry point for an
//! inserted block or an `engine_newPayload` payload. It overlaps payload conversion, transaction
//! preparation, cache prewarming, receipt-root computation, state-root computation, and deferred
//! trie input construction wherever those tasks do not depend on each other.
//!
//! ## Validation phases
//!
//! 1. Fetch the parent header and spawn `payload-convert`, which converts payloads into sealed
//!    blocks and runs header, parent-header, and pre-execution consensus validation.
//! 2. Build the parent state provider, EVM environment, transaction iterator, lazy ancestor
//!    overlay, and optional decoded EIP-7928 block access list (BAL).
//! 3. Prepare the per-block state-root job. The default strategy picks a skipped, synchronous, or
//!    sparse-trie job from [`TreeConfig`].
//! 4. Spawn the payload processor. This always prepares transaction conversion and prewarming; a
//!    streaming state-root job can provide a sink for prewarm and execution updates.
//! 5. Execute the block. BAL payloads use the parallel BAL execute path only when state caching and
//!    BAL parallel execution are enabled. Otherwise the regular executor still builds and validates
//!    the BAL before post-execution consensus uses the decoded BAL hash.
//! 6. Stop prewarming, terminate execution caching, spawn `hash-post-state`, await
//!    `payload-convert` and `receipt-root`, then run post-execution consensus validation.
//! 7. Resolve the state root by finishing the prepared job. The sparse-trie job falls back to
//!    serial computation when the state-root task fails to produce a usable root.
//! 8. Verify the header state root, spawn deferred trie input computation, and return the executed
//!    block without waiting for that deferred trie task on the hot path.
//!
//! ## Spawned background work
//!
//! | Work | Spawned when | Role | Completion point |
//! | --- | --- | --- | --- |
//! | `payload-convert` | parent is known | convert payloads, validate header and body roots | after execution, unless the gas sanity check awaits it earlier |
//! | `tx-iterator` | payload processor setup | convert transactions, using rayon for larger blocks | consumed by regular and BAL execution |
//! | `prewarm` | payload processor setup | warm execution caches; in BAL mode, stream BAL-derived trie targets | stopped after execution, then caching is terminated |
//! | proof workers | sparse-trie task setup | fetch trie proofs for sparse trie updates | consumed by the sparse trie task |
//! | `sparse-trie` | sparse-trie task setup | apply execution or BAL updates and compute the state root | awaited by the state-root job |
//! | `receipt-root` | execution start | compute receipt root and logs bloom incrementally | awaited before post-execution consensus |
//! | `hash-post-state` | after execution | hash changed accounts and storage from `BundleState` | awaited by post-execution validation and root computation |
//! | `serial-root` | sparse trie timeout fallback | race serial state-root computation against the sparse trie task | polled by the sparse-trie job |
//! | deferred trie task | after root verification | sort trie data | not awaited by the validation hot path |
//!
//! ```mermaid
//! sequenceDiagram
//!     autonumber
//!     participant Main as validate_block_with_state
//!     participant Convert as payload-convert
//!     participant Tx as tx-iterator
//!     participant Prewarm as prewarm
//!     participant Exec as EVM execution
//!     participant Receipt as receipt-root
//!     participant Trie as sparse trie and proofs
//!     participant Hash as hash-post-state
//!     participant Deferred as deferred trie task
//!
//!     Main->>Convert: spawn convert and pre-execution validation
//!     Main->>Main: parent provider, EVM env, optional BAL decode
//!     Main->>Tx: spawn transaction conversion
//!     alt sparse-trie job
//!         Main->>Trie: spawn proof workers and sparse trie
//!     end
//!     Main->>Prewarm: spawn transaction, BAL, or skipped prewarm
//!     Main->>Receipt: spawn receipt root task
//!     alt BAL path eligible
//!         Main->>Exec: execute_block_bal
//!         Prewarm->>Trie: BAL-derived sparse trie updates
//!     else regular execution
//!         Tx-->>Exec: recovered transactions in block order
//!         Main->>Exec: execute_block
//!         Exec->>Receipt: stream receipts
//!         Exec->>Trie: stream state hook updates
//!         Exec->>Exec: rebuild and validate BAL when present
//!     end
//!     Main->>Prewarm: stop prewarming and terminate cache
//!     Main->>Hash: spawn changed-state hashing
//!     Convert-->>Main: sealed block
//!     Receipt-->>Main: receipt root and logs bloom
//!     Main->>Main: post-execution consensus and BAL hash check
//!     Hash-->>Main: hashed post state
//!     alt sparse-trie job
//!         Trie-->>Main: state root and trie updates
//!     else synchronous or fallback
//!         Main->>Main: compute serial StateRoot
//!     end
//!     Main->>Main: verify header state root
//!     Main->>Deferred: spawn trie input sorting
//!     Main-->>Main: return ValidationOutput
//! ```
//!
//! ## Payload attributes validation
//!
//! During `engine_forkchoiceUpdated`,
//! [`PayloadValidator::validate_payload_attributes_against_header`] checks payload attributes
//! before a payload build job starts. On failure, the engine returns
//! `INVALID_PAYLOAD_ATTRIBUTES` without rolling back the forkchoice update.

//! Cloned from `reth_engine_tree::tree::payload_validator` (SF reth fork branch `firehose/2.x`).
//! When updating, copy that file and review the diff. The two surgical differences:
//!   1. `validate_block_with_state` branches on [`reth_firehose::is_tracer_initialized`] and
//!      eagerly resolves the [`SealedBlock`] so a [`reth_firehose::FirehoseBlockTracer`] guard can
//!      be started **before** execution.
//!   2. A new [`OpFirehoseEngineValidator::execute_and_trace_block`] is the Firehose-enabled twin
//!      of `execute_block`. It builds the EVM with `evm_with_env_and_inspector` (the
//!      `FirehoseInspector` borrowed from the tracer) and wraps the OP block executor in a
//!      [`reth_firehose::FirehoseWrappedExecutor::with_hooks`] carrying [`crate::OpPreTxAdjust`] +
//!      [`crate::OpPostTxExtras`]. The non-traced path is unchanged.
//!
//! MAINTENANCE CONTRACT: keep [`OpFirehoseEngineValidator::execute_block`] and
//! [`OpFirehoseEngineValidator::execute_and_trace_block`] in lock-step. Any change to the
//! non-traced path (new metrics, error handling, ordering of pre/post steps) must be mirrored in
//! the traced twin.

use alloy_consensus::transaction::{Either, TxHashRef};
use alloy_evm::Evm;
use alloy_primitives::{
    B256,
    map::{AddressMap, B256Set},
};

use alloy_consensus::constants::KECCAK_EMPTY;
use alloy_primitives::Address;
use reth_chain_state::{ExecutedBlock, ExecutionTimingStats, StateTrieOverlayManager};
use reth_consensus::{ConsensusError, FullConsensus, ReceiptRootBloom};
use reth_engine_primitives::{
    ConfigureEngineEvm, ExecutableTxIterator, InvalidBlockHook, PayloadValidator,
};
// NOTE: the new `state_root_strategy` module (`StateRootJobContext`, `PayloadStateRootJobContext`,
// `StateRootStrategy`, `DefaultStateRootStrategy`) is intentionally NOT used here. Its context
// constructors (`StateRootJobContext::new`, `PayloadStateRootJobContext::new`) and
// `PayloadProcessor::execution_cache()` are `pub(crate)` in `reth_engine_tree` — inaccessible from
// this external crate. Only `BasicEngineValidator` itself (same-crate) can drive that machinery.
// We fall back to computing the state root synchronously/in-parallel via the still-public
// `StateRootProvider`/`ParallelStateRoot` primitives below (see `compute_state_root_parallel` /
// `compute_state_root_serial` / `plan_state_root_computation`), same as this clone did before the
// v2.4.2 rebase. This only affects the state-root algorithm choice, not Firehose tracing: no
// tracer event is emitted or altered by which state-root strategy runs.
use reth_engine_tree::tree::{
    CacheWaitDurations, CachedStateProvider, EngineApiMetrics, EngineApiTreeState, ExecutionEnv,
    PayloadHandle, PayloadProcessor, StateProviderBuilder, TreeConfig, ValidationOutput,
    WaitForCaches,
    error::{InsertBlockError, InsertBlockErrorKind, InsertPayloadError},
    instrumented_state::{InstrumentedStateProvider, StateProviderStats},
    payload_processor::receipt_root_task::{IndexedReceipt, ReceiptRootTaskHandle},
    payload_validator::{BlockOrPayload, EngineValidator, TreeCtx, ValidationOutcome},
    precompile_cache::{CachedPrecompile, CachedPrecompileMetrics, PrecompileCacheMap},
    state_root_strategy::{PayloadStateRootHandle, StateRootHintStream, StateRootUpdateStream},
};
use reth_errors::{BlockExecutionError, ProviderResult};
use reth_evm::{
    ConfigureEvm, EvmEnvFor, ExecutionCtxFor, OnStateHook, SpecFor, block::BlockExecutor,
    execute::ExecutableTxFor,
};
use reth_execution_cache::{CacheFillMode, CacheStats, SavedCache};
use reth_payload_primitives::{
    BuiltPayload, BuiltPayloadExecutedBlock, InvalidPayloadAttributesError, NewPayloadError,
    PayloadTypes,
};
use reth_primitives_traits::{
    AlloyBlockHeader, BlockBody, FastInstant as Instant, GotExpected, NodePrimitives,
    RecoveredBlock, SealedBlock, SealedHeader, SignerRecoverable,
};
use reth_provider::{
    BlockExecutionOutput, BlockNumReader, BlockReader, ChangeSetReader, DatabaseProviderFactory,
    DatabaseProviderROFactory, HashedPostStateProvider, ProviderError, PruneCheckpointReader,
    StageCheckpointReader, StateProvider, StateProviderBox, StateProviderFactory, StateReader,
    StorageChangeSetReader, StorageSettingsCache, providers::OverlayStateProviderFactory,
};
use reth_revm::{
    database::StateProviderDatabase,
    db::{BundleAccount, State, states::bundle_state::BundleRetention},
};
use reth_trie::{
    HashedPostState, LazyTrieData, hashed_cursor::HashedCursorFactory,
    prefix_set::TriePrefixSetsMut, trie_cursor::TrieCursorFactory, updates::TrieUpdates,
};
use reth_trie_db::ChangesetCache;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tracing::{Span, debug, debug_span, error, instrument, trace, warn};

use alloy_op_evm::OpEvmFactory;
use reth_firehose::{
    FirehoseBlockTracer, FirehoseWrappedExecutor, is_tracer_initialized, mapper::SignatureFields,
};
use reth_optimism_evm::OpTx;
use reth_primitives_traits::TxTy;

use crate::{OpPostTxExtras, OpPreTxAdjust};

/// Handle to a [`HashedPostState`] computed on a background thread.
type LazyHashedPostState = reth_tasks::LazyHandle<HashedPostState>;

/// Result type for block validation with optional timing stats.
type InsertPayloadResult<N> = Result<
    (ExecutedBlock<N>, Option<Box<ExecutionTimingStats>>),
    InsertPayloadError<<N as NodePrimitives>::Block>,
>;

/// Worker name for deferred trie data preparation.
const DEFERRED_TRIE_WORKER_NAME: &str = "deferred-trie";

/// Pauses JIT helper execution while validating imported payloads.
///
/// Validation still queues JIT work and can use resident compiled code, but helper execution is
/// paused during validation to minimize latency. Queued work resumes when validation exits, so JIT
/// compilation is biased toward idle periods instead of competing with payload validation.
struct JitPauseGuard<Evm: ConfigureEvm>(Evm);

impl<Evm: ConfigureEvm> JitPauseGuard<Evm> {
    fn new(evm_config: &Evm) -> Self {
        if let Some(jit_backend) = evm_config.jit_backend() {
            jit_backend.pause();
        }
        Self(evm_config.clone())
    }
}

impl<Evm: ConfigureEvm> Drop for JitPauseGuard<Evm> {
    fn drop(&mut self) {
        if let Some(jit_backend) = self.0.jit_backend() {
            jit_backend.resume();
        }
    }
}
/// A helper type that provides reusable payload validation logic for OP Stack networks
/// with Firehose live-tracing support.
///
/// This type satisfies [`EngineValidator`] and is responsible for executing blocks/payloads.
///
/// This type contains common validation, execution, and state root computation logic that can be
/// used by network-specific payload validators (e.g., Ethereum, Optimism). It is not meant to be
/// used as a standalone component, but rather as a building block for concrete implementations.
#[derive(derive_more::Debug)]
pub struct OpFirehoseEngineValidator<P, Evm, V>
where
    Evm: ConfigureEvm,
{
    /// Provider for database access.
    provider: P,
    /// Consensus implementation for validation.
    consensus: Arc<dyn FullConsensus<Evm::Primitives>>,
    /// EVM configuration.
    evm_config: Evm,
    /// Configuration for the tree.
    config: TreeConfig,
    /// Payload processor for transaction conversion, prewarming, and execution caching.
    payload_processor: PayloadProcessor<Evm>,
    /// Precompile cache map.
    precompile_cache_map: PrecompileCacheMap<SpecFor<Evm>>,
    /// Precompile cache metrics.
    precompile_cache_metrics: AddressMap<CachedPrecompileMetrics>,
    /// Hook to call when invalid blocks are encountered.
    #[debug(skip)]
    invalid_block_hook: Box<dyn InvalidBlockHook<Evm::Primitives>>,
    /// Metrics for the engine api.
    metrics: EngineApiMetrics,
    /// Validator for the payload.
    validator: V,
    /// Changeset cache for in-memory trie changesets
    changeset_cache: ChangesetCache,
    /// Task runtime for spawning parallel work.
    runtime: reth_tasks::Runtime,
    /// Shared state trie in-memory overlay data.
    state_trie_overlays: StateTrieOverlayManager<Evm::Primitives>,
}

impl<N, P, Evm, V> OpFirehoseEngineValidator<P, Evm, V>
where
    N: NodePrimitives,
    P: DatabaseProviderFactory<
            Provider: BlockReader
                          + StageCheckpointReader
                          + PruneCheckpointReader
                          + ChangeSetReader
                          + StorageChangeSetReader
                          + BlockNumReader
                          + StorageSettingsCache,
        > + BlockReader<Header = N::BlockHeader>
        + ChangeSetReader
        + BlockNumReader
        + StateProviderFactory
        + StateReader
        + HashedPostStateProvider
        + Clone
        + 'static,
    OverlayStateProviderFactory<P, N>: DatabaseProviderROFactory<Provider: TrieCursorFactory + HashedCursorFactory>
        + Clone
        + Send
        + Sync
        + 'static,
    Evm: ConfigureEvm<Primitives = N> + 'static,
{
    /// Creates a new `TreePayloadValidator`.
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        provider: P,
        consensus: Arc<dyn FullConsensus<N>>,
        evm_config: Evm,
        validator: V,
        config: TreeConfig,
        invalid_block_hook: Box<dyn InvalidBlockHook<N>>,
        changeset_cache: ChangesetCache,
        state_trie_overlays: StateTrieOverlayManager<N>,
        runtime: reth_tasks::Runtime,
    ) -> Self {
        let precompile_cache_map = PrecompileCacheMap::default();
        let payload_processor = PayloadProcessor::new(
            runtime.clone(),
            evm_config.clone(),
            &config,
            precompile_cache_map.clone(),
        );
        Self {
            provider,
            consensus,
            evm_config,
            payload_processor,
            precompile_cache_map,
            precompile_cache_metrics: AddressMap::default(),
            config,
            invalid_block_hook,
            metrics: EngineApiMetrics::default(),
            validator,
            changeset_cache,
            runtime,
            state_trie_overlays,
        }
    }

    /// Converts a [`BlockOrPayload`] to a recovered block.
    #[instrument(level = "debug", target = "engine::tree::payload_validator", skip_all)]
    pub fn convert_to_block<T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &self,
        input: BlockOrPayload<T>,
    ) -> Result<SealedBlock<N::Block>, NewPayloadError>
    where
        V: PayloadValidator<T, Block = N::Block>,
    {
        match input {
            BlockOrPayload::Payload(payload) => self.validator.convert_payload_to_block(payload),
            BlockOrPayload::Block(block) => Ok(block),
        }
    }

    /// Returns EVM environment for the given payload or block.
    pub fn evm_env_for<T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &self,
        input: &BlockOrPayload<T>,
    ) -> Result<EvmEnvFor<Evm>, Evm::Error>
    where
        V: PayloadValidator<T, Block = N::Block>,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
    {
        match input {
            BlockOrPayload::Payload(payload) => Ok(self.evm_config.evm_env_for_payload(payload)?),
            BlockOrPayload::Block(block) => Ok(self.evm_config.evm_env(block.header())?),
        }
    }

    /// Returns [`ExecutableTxIterator`] for the given payload or block.
    pub fn tx_iterator_for<'a, T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &'a self,
        input: &'a BlockOrPayload<T>,
    ) -> Result<impl ExecutableTxIterator<Evm>, NewPayloadError>
    where
        V: PayloadValidator<T, Block = N::Block>,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
    {
        Ok(match input {
            BlockOrPayload::Payload(payload) => {
                let iter = self
                    .evm_config
                    .tx_iterator_for_payload(payload)
                    .map_err(NewPayloadError::other)?;
                Either::Left(iter)
            }
            BlockOrPayload::Block(block) => {
                let txs = block.body().clone_transactions();
                let convert = |tx: N::SignedTx| tx.try_into_recovered();
                Either::Right((txs, convert))
            }
        })
    }

    /// Returns a [`ExecutionCtxFor`] for the given payload or block.
    pub fn execution_ctx_for<'a, T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &self,
        input: &'a BlockOrPayload<T>,
    ) -> Result<ExecutionCtxFor<'a, Evm>, Evm::Error>
    where
        V: PayloadValidator<T, Block = N::Block>,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
    {
        match input {
            BlockOrPayload::Payload(payload) => Ok(self.evm_config.context_for_payload(payload)?),
            BlockOrPayload::Block(block) => Ok(self.evm_config.context_for_block(block)?),
        }
    }

    /// Handles execution errors by checking if header validation errors should take precedence.
    ///
    /// When an execution error occurs, this function checks if there are any header validation
    /// errors that should be reported instead, as header validation errors have higher priority.
    fn handle_execution_error<T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &self,
        input: BlockOrPayload<T>,
        execution_err: InsertBlockErrorKind,
        parent_block: &SealedHeader<N::BlockHeader>,
    ) -> InsertPayloadResult<N>
    where
        V: PayloadValidator<T, Block = N::Block>,
    {
        debug!(
            target: "engine::tree::payload_validator",
            ?execution_err,
            block = ?input.num_hash(),
            "Block execution failed, checking for header validation errors"
        );

        // If execution failed, we should first check if there are any header validation
        // errors that take precedence over the execution error
        let block = self.convert_to_block(input)?;

        // Validate block consensus rules which includes header validation
        if let Err(consensus_err) = self.validate_block_inner(&block, None) {
            // Header validation error takes precedence over execution error
            return Err(InsertBlockError::new(block, consensus_err.into()).into());
        }

        // Also validate against the parent
        if let Err(consensus_err) =
            self.consensus.validate_header_against_parent(block.sealed_header(), parent_block)
        {
            // Parent validation error takes precedence over execution error
            return Err(InsertBlockError::new(block, consensus_err.into()).into());
        }

        // No header validation errors, return the original execution error
        Err(InsertBlockError::new(block, execution_err).into())
    }

    /// Validates a block that has already been converted from a payload.
    ///
    /// This method performs:
    /// - Consensus validation
    /// - Block execution
    /// - State root computation
    /// - Fork detection
    #[instrument(
        level = "debug",
        target = "engine::tree::payload_validator",
        skip_all,
        fields(
            parent = ?input.parent_hash(),
            type_name = ?input.type_name(),
        )
    )]
    pub fn validate_block_with_state<T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &mut self,
        input: BlockOrPayload<T>,
        mut ctx: TreeCtx<'_, N>,
    ) -> InsertPayloadResult<N>
    where
        V: PayloadValidator<T, Block = N::Block> + Clone,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
        Evm::BlockExecutorFactory:
            alloy_evm::block::BlockExecutorFactory<EvmFactory = OpEvmFactory<OpTx>>,
        TxTy<N>: alloy_consensus::Transaction
            + alloy_consensus::transaction::TxHashRef
            + SignatureFields,
    {
        // Spawn payload conversion on a background thread so it runs concurrently with the
        // rest of the function (setup + execution). For payloads this overlaps the cost of
        // RLP decoding + header hashing.
        let is_payload = matches!(&input, BlockOrPayload::Payload(_));
        let convert_to_block = match &input {
            BlockOrPayload::Payload(_) => {
                let payload_clone = input.clone();
                let validator = self.validator.clone();
                let handle = self.runtime.spawn_blocking_named("payload-convert", move || {
                    let BlockOrPayload::Payload(payload) = payload_clone else { unreachable!() };
                    validator.convert_payload_to_block(payload)
                });
                Either::Left(handle)
            }
            BlockOrPayload::Block(_) => Either::Right(()),
        };

        // Returns the sealed block, either by awaiting the background conversion task (for
        // payloads) or by extracting the already-converted block directly.
        let convert_to_block =
            move |input: BlockOrPayload<T>| -> Result<SealedBlock<N::Block>, NewPayloadError> {
                match convert_to_block {
                    Either::Left(handle) => handle.try_into_inner().expect("sole handle"),
                    Either::Right(()) => {
                        let BlockOrPayload::Block(block) = input else { unreachable!() };
                        Ok(block)
                    }
                }
            };

        /// A helper macro that returns the block in case there was an error
        /// This macro is used for early returns before block conversion
        macro_rules! ensure_ok {
            ($expr:expr) => {
                match $expr {
                    Ok(val) => val,
                    Err(e) => {
                        let block = convert_to_block(input)?;
                        return Err(InsertBlockError::new(block, e.into()).into());
                    }
                }
            };
        }

        /// A helper macro for handling errors after the input has been converted to a block
        macro_rules! ensure_ok_post_block {
            ($expr:expr, $block:expr) => {
                match $expr {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(
                            InsertBlockError::new($block.into_sealed_block(), e.into()).into()
                        )
                    }
                }
            };
        }

        let parent_hash = input.parent_hash();
        let _jit_pause = JitPauseGuard::new(&self.evm_config);

        trace!(target: "engine::tree::payload_validator", "Fetching block state provider");
        let _enter =
            debug_span!(target: "engine::tree::payload_validator", "state_provider").entered();
        let Some(provider_builder) =
            ensure_ok!(self.state_provider_builder(parent_hash, ctx.state()))
        else {
            // this is pre-validated in the tree
            return Err(InsertBlockError::new(
                convert_to_block(input)?,
                ProviderError::HeaderNotFound(parent_hash.into()).into(),
            )
            .into());
        };
        let mut state_provider = ensure_ok!(provider_builder.build());
        drop(_enter);

        // Fetch parent block. This goes to memory most of the time unless the parent block is
        // beyond the in-memory buffer.
        let Some(parent_block) = ensure_ok!(self.sealed_header_by_hash(parent_hash, ctx.state()))
        else {
            return Err(InsertBlockError::new(
                convert_to_block(input)?,
                ProviderError::HeaderNotFound(parent_hash.into()).into(),
            )
            .into());
        };

        let evm_env = debug_span!(target: "engine::tree::payload_validator", "evm_env")
            .in_scope(|| self.evm_env_for(&input))
            .map_err(NewPayloadError::other)?;

        // Extract the decoded BAL, if valid and available.
        let decoded_bal = ensure_ok!(
            input
                .try_decoded_access_list()
                .map_err(|err| { Box::<dyn std::error::Error + Send + Sync>::from(err) })
        )
        .map(Arc::new);

        if let Some(decoded_bal) = decoded_bal.as_deref() {
            // Reject oversized BAL sidecars before executing the block.
            ensure_ok!(
                decoded_bal
                    .as_bal()
                    .validate_gas_limit(input.gas_limit())
                    .map_err(ConsensusError::from)
            );
        }

        let env = ExecutionEnv {
            evm_env,
            hash: input.hash(),
            parent_hash: input.parent_hash(),
            parent_state_root: parent_block.state_root(),
            transaction_count: input.transaction_count(),
            gas_used: input.gas_used(),
            withdrawals: input.withdrawals().map(|w| w.to_vec()),
            decoded_bal: decoded_bal.as_ref().map(Arc::clone),
        };

        // Get an iterator over the transactions in the payload
        let txs = self.tx_iterator_for(&input)?;

        // OP payloads never carry a decoded EIP-7928 BAL (`env.decoded_bal` is always `None` here
        // — OP Stack has not activated EIP-7928), so the BAL-parallel-execution path is always
        // disabled. See `execute_and_trace_block` / `execute_block`'s call sites: neither branches
        // on this beyond passing it through to `spawn_with_state_root_streams`, which accepts it
        // unconditionally.
        let parallel_bal_execution = false;

        // No execution-state-hook streaming source: the new `state_root_strategy` framework's
        // context constructors (`StateRootJobContext::new`) are `pub(crate)` in `reth_engine_tree`
        // and inaccessible here (see the module-doc note above `PayloadStateRootHandle`'s import).
        // `hint_stream` / `hashed_update_stream` / `execution_state_hook` are therefore always
        // `None`: the state root is computed after execution via `compute_state_root_serial`
        // below instead of the newer streaming sparse-trie job. `ParallelStateRoot` (the previous
        // non-streaming fast path) was also removed upstream in this rebase — its functionality
        // was folded into the now-`pub(crate)` sparse-trie job — so serial computation is the only
        // reachable strategy for this clone.
        let hint_stream = None;
        let hashed_update_stream = None;
        let execution_state_hook = None;

        // Spawn transaction conversion and prewarming.
        let mut handle = ensure_ok!(self.spawn_payload_processor(
            env.clone(),
            txs,
            provider_builder.clone(),
            hint_stream,
            hashed_update_stream,
            parallel_bal_execution,
        ));

        // Create optional cache stats for detailed block logging
        let slow_block_enabled = self.config.slow_block_threshold().is_some();
        let cache_stats = slow_block_enabled.then(|| Arc::new(CacheStats::default()));

        // Use cached state provider before executing, used in execution after prewarming threads
        // complete
        if let Some(caches) = handle.caches() {
            // `CacheFillMode::LookupOnly` matches upstream's serial execution path: revm's own
            // `State` caching makes fill-on-miss redundant here, and the execution post-state is
            // dumped into the execution cache wholesale after the block anyway.
            state_provider = Box::new(CachedStateProvider::new_with_mode(
                state_provider,
                caches,
                CacheFillMode::LookupOnly,
                handle.cache_metrics(),
                cache_stats.clone(),
            ));
        };

        let state_provider_stats = if slow_block_enabled || self.config.state_provider_metrics() {
            let instrumented_state_provider =
                InstrumentedStateProvider::new(state_provider, "engine");
            let stats = slow_block_enabled.then(|| instrumented_state_provider.stats());
            state_provider = Box::new(instrumented_state_provider);
            stats
        } else {
            None
        };

        // Firehose: when the tracer is active, eagerly resolve the sealed block so we can start a
        // block-level tracer guard before execution. The guard emits `on_block_start` now and
        // defers `on_block_end(None)` until `mark_verified()` runs after post-execution
        // validation.
        //
        // If any early return is taken between here and `mark_verified()`, the guard's Drop emits
        // `on_block_end(Some(err))` so invalid blocks are never flushed downstream.
        //
        // `fh_tracer_rewrote_input` tracks whether the branch below actually replaced `input`
        // with an already-sealed `BlockOrPayload::Block` (only happens when the tracer is
        // active). When the tracer is NOT active, `input` is left completely untouched — it may
        // still be `BlockOrPayload::Payload(_)` — so the `convert_to_block` closure used further
        // down must keep handling both variants (via the background-conversion-aware closure
        // defined above, not a Block-only shortcut). A previous version of this file
        // unconditionally re-shadowed `convert_to_block` with a Block-only closure here, which
        // panicked (`unreachable!`) whenever the tracer was inactive and the payload path was
        // taken — fixed by only rebuilding the closure inside the tracer-active branch.
        let (mut fh_tracer, input, convert_to_block): (
            Option<FirehoseBlockTracer>,
            _,
            Box<dyn FnOnce(BlockOrPayload<T>) -> Result<SealedBlock<N::Block>, NewPayloadError>>,
        ) = if is_tracer_initialized() {
            let sealed = match convert_to_block(input) {
                Ok(sealed) => sealed,
                Err(e) => return Err(e.into()),
            };
            let tracer = FirehoseBlockTracer::start::<N>(&sealed, None);
            firehose_tracer::firehose_debug!(
                "validator: firehose tracer initialized (block={})",
                sealed.header().number(),
            );
            let fh_tracer = Some(tracer);
            // `input` is now a fully-resolved `Block`. Rebuild the lazy `convert_to_block`
            // closure as a trivial no-op — safe here, and only here, because we just rewrote
            // `input` ourselves.
            let input = BlockOrPayload::Block(sealed);
            let convert_to_block: Box<
                dyn FnOnce(BlockOrPayload<T>) -> Result<SealedBlock<N::Block>, NewPayloadError>,
            > = Box::new(|input: BlockOrPayload<T>| match input {
                BlockOrPayload::Block(block) => Ok(block),
                BlockOrPayload::Payload(_) => {
                    unreachable!("convert_to_block already resolved input to Block")
                }
            });
            (fh_tracer, input, convert_to_block)
        } else {
            firehose_tracer::firehose_debug!(
                "validator: firehose tracer NOT initialized — non-traced execution path",
            );
            (None, input, Box::new(convert_to_block))
        };

        // Execute the block and handle any execution errors.
        // The receipt root task is spawned before execution and receives receipts incrementally
        // as transactions complete, allowing parallel computation during execution.
        //
        // Two entry points exist for the same work: `execute_block` is the non-traced path and
        // `execute_and_trace_block` is its Firehose-enabled twin. We pick based on whether a
        // live tracer guard is available for this block (i.e. whether the tracer is
        // initialized).
        let execute_block_start = Instant::now();
        let (output, senders, receipt_root_rx) = match fh_tracer.as_mut() {
            Some(tracer) => {
                match self.execute_and_trace_block(state_provider, env, &input, tracer, &mut handle)
                {
                    Ok(output) => output,
                    Err(err) => return self.handle_execution_error(input, err, &parent_block),
                }
            }
            None => match self.execute_block(
                state_provider,
                env,
                &input,
                &mut handle,
                execution_state_hook,
            ) {
                Ok(output) => output,
                Err(err) => return self.handle_execution_error(input, err, &parent_block),
            },
        };
        let execution_duration = execute_block_start.elapsed();

        // After executing the block we can stop prewarming transactions
        handle.stop_prewarming_execution();

        // Create ExecutionOutcome early so we can terminate caching before validation and state
        // root computation. Using Arc allows sharing with both the caching task and the deferred
        // trie task without cloning the expensive BundleState.
        let output = Arc::new(output);

        // Terminate caching task early since execution is complete and caching is no longer
        // needed. This frees up resources while state root computation continues.
        let valid_block_tx = handle.terminate_caching(Some(output.clone()));

        // Spawn hashed post state computation in background so it runs concurrently with
        // block conversion and receipt root computation. This is a pure CPU-bound task
        // (keccak256 hashing of all changed addresses and storage slots).
        let hashed_state_output = output.clone();
        let hashed_state_provider = self.provider.clone();
        let hashed_state: LazyHashedPostState =
            self.runtime.spawn_blocking_named("hash-post-state", move || {
                let _span = debug_span!(
                    target: "engine::tree::payload_validator",
                    "hashed_post_state",
                )
                .entered();
                hashed_state_provider.hashed_post_state(&hashed_state_output.state)
            });

        let block = convert_to_block(input)?;
        let transaction_root = is_payload.then(|| {
            let body = block.body().clone();
            let parent_span = Span::current();
            let num_hash = block.num_hash();
            self.runtime.spawn_blocking_named("payload-tx-root", move || {
                let _span =
                    debug_span!(target: "engine::tree::payload_validator", parent: parent_span, "payload_tx_root", block = ?num_hash)
                        .entered();
                body.calculate_tx_root()
            })
        });
        let block = block.with_senders(senders);

        // Wait for the receipt root computation to complete.
        let receipt_root_bloom = {
            let _enter = debug_span!(
                target: "engine::tree::payload_validator",
                "wait_receipt_root",
            )
            .entered();

            receipt_root_rx
                .blocking_recv()
                .inspect_err(|_| {
                    tracing::error!(
                        target: "engine::tree::payload_validator",
                        "Receipt root task dropped sender without result, receipt root calculation likely aborted"
                    );
                })
                .ok()
        };
        let transaction_root = transaction_root.map(|handle| {
            let _span =
                debug_span!(target: "engine::tree::payload_validator", "wait_payload_tx_root")
                    .entered();
            handle.try_into_inner().expect("sole handle")
        });

        ensure_ok_post_block!(
            self.validate_post_execution(
                &block,
                &parent_block,
                &output,
                &mut ctx,
                transaction_root,
                receipt_root_bloom,
            ),
            block
        );

        let hashed_state_validate_result = debug_span!(
            target: "engine::tree::payload_validator",
            "validate_block_post_execution_with_hashed_state"
        )
        .in_scope(|| {
            self.validator.validate_block_post_execution_with_hashed_state(
                || hashed_state.get(),
                &block,
                || provider_builder.build(),
            )
        });

        if let Err(err) = hashed_state_validate_result {
            if err.is_validation_error() {
                self.on_invalid_block(&parent_block, &block, &output, None, ctx.state_mut());
            }
            return Err(InsertBlockError::new(block.into_sealed_block(), err).into());
        }

        // Compute the state root serially. Neither the sparse-trie background job
        // (`state_root_task`) nor the old `ParallelStateRoot` fast path is available here — see
        // the module-doc note above.
        let root_start = Instant::now();
        debug!(target: "engine::tree::payload_validator", "Using synchronous state root computation");
        let (state_root, trie_output) = {
            let (root, updates) = ensure_ok_post_block!(
                provider_builder
                    .build()
                    .and_then(|provider| Self::compute_state_root_serial(provider, &hashed_state)),
                block
            );
            (root, Arc::new(updates))
        };
        let root_elapsed = root_start.elapsed();
        // This clone does not compute the trie's changed base paths (only the sparse-trie job
        // does); pass `None` — `LazyTrieData::pending` treats it as "unknown, compute lazily".
        let changed_paths = None;

        self.metrics.block_validation.record_state_root(&trie_output, root_elapsed.as_secs_f64());
        self.metrics
            .record_state_root_gas_bucket(block.header().gas_used(), root_elapsed.as_secs_f64());
        debug!(target: "engine::tree::payload_validator", ?root_elapsed, "Calculated state root");

        // ensure state root matches
        if state_root != block.header().state_root() {
            // call post-block hook
            self.on_invalid_block(
                &parent_block,
                &block,
                &output,
                Some((&trie_output, state_root)),
                ctx.state_mut(),
            );
            let block_state_root = block.header().state_root();
            return Err(InsertBlockError::new(
                block.into_sealed_block(),
                ConsensusError::BodyStateRootDiff(
                    GotExpected { got: state_root, expected: block_state_root }.into(),
                )
                .into(),
            )
            .into());
        }

        let timing_stats = state_provider_stats.map(|stats| {
            self.calculate_timing_stats(
                &block,
                stats,
                cache_stats,
                &output,
                execution_duration,
                root_elapsed,
            )
        });

        if let Some(valid_block_tx) = valid_block_tx {
            let _ = valid_block_tx.send(());
        }

        // All post-execution validations passed — flush the Firehose block. If the guard was
        // never started (genesis or tracer not initialized), this branch is a no-op.
        if let Some(guard) = fh_tracer.take() {
            guard.mark_verified();
        }

        let executed_block =
            self.spawn_deferred_trie_task(block, output, hashed_state, trie_output, changed_paths);
        Ok((executed_block, timing_stats))
    }

    /// Return sealed block header from database or in-memory state by hash.
    fn sealed_header_by_hash(
        &self,
        hash: B256,
        state: &EngineApiTreeState<N>,
    ) -> ProviderResult<Option<SealedHeader<N::BlockHeader>>> {
        // check memory first
        let header = state.tree_state().sealed_header_by_hash(&hash);

        if header.is_some() { Ok(header) } else { self.provider.sealed_header_by_hash(hash) }
    }

    /// Validate if block is correct and satisfies all the consensus rules that concern the header
    /// and block body itself.
    #[instrument(level = "debug", target = "engine::tree::payload_validator", skip_all)]
    fn validate_block_inner(
        &self,
        block: &SealedBlock<N::Block>,
        transaction_root: Option<B256>,
    ) -> Result<(), ConsensusError> {
        if let Err(e) = self.consensus.validate_header(block.sealed_header()) {
            error!(target: "engine::tree::payload_validator", ?block, "Failed to validate header {}: {e}", block.hash());
            return Err(e);
        }

        if let Err(e) =
            self.consensus.validate_block_pre_execution_with_tx_root(block, transaction_root)
        {
            error!(target: "engine::tree::payload_validator", ?block, "Failed to validate block {}: {e}", block.hash());
            return Err(e);
        }

        Ok(())
    }

    /// Executes a block with the given state provider.
    ///
    /// This method orchestrates block execution:
    /// 1. Sets up the EVM with state database and precompile caching
    /// 2. Spawns a background task for incremental receipt root computation
    /// 3. Executes transactions with metrics collection via state hooks
    /// 4. Merges state transitions and records execution metrics
    #[instrument(level = "debug", target = "engine::tree::payload_validator", skip_all)]
    #[expect(clippy::type_complexity)]
    fn execute_block<S, Err, T>(
        &mut self,
        state_provider: S,
        env: ExecutionEnv<Evm>,
        input: &BlockOrPayload<T>,
        handle: &mut PayloadHandle<impl ExecutableTxFor<Evm>, Err, N::Receipt>,
        state_hook: Option<Box<dyn OnStateHook + 'static>>,
    ) -> Result<
        (
            BlockExecutionOutput<N::Receipt>,
            Vec<Address>,
            tokio::sync::oneshot::Receiver<(B256, alloy_primitives::Bloom)>,
        ),
        InsertBlockErrorKind,
    >
    where
        S: StateProvider + Send,
        Err: core::error::Error + Send + Sync + 'static,
        V: PayloadValidator<T, Block = N::Block>,
        T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
    {
        debug!(target: "engine::tree::payload_validator", "Executing block");

        let mut db = debug_span!(target: "engine::tree", "build_state_db").in_scope(|| {
            State::builder()
                .with_database(StateProviderDatabase::new(state_provider))
                .with_bundle_update()
                .build()
        });

        let (spec_id, mut executor) = {
            let _span = debug_span!(target: "engine::tree", "create_evm").entered();
            let spec_id = *env.evm_env.spec_id();
            let evm_config = self.evm_config.clone().with_jit_support();
            let evm = evm_config.evm_with_env(&mut db, env.evm_env);
            let ctx = self
                .execution_ctx_for(input)
                .map_err(|e| InsertBlockErrorKind::Other(Box::new(e)))?;
            let executor = self.evm_config.create_executor(evm, ctx);
            (spec_id, executor)
        };

        if !self.config.precompile_cache_disabled() {
            let _span = debug_span!(target: "engine::tree", "setup_precompile_cache").entered();
            executor.evm_mut().precompiles_mut().map_cacheable_precompiles(
                |address, precompile| {
                    let metrics = self
                        .precompile_cache_metrics
                        .entry(*address)
                        .or_insert_with(|| CachedPrecompileMetrics::new_with_address(*address))
                        .clone();
                    CachedPrecompile::wrap(
                        precompile,
                        self.precompile_cache_map.cache_for_address(*address),
                        spec_id,
                        Some(metrics),
                    )
                },
            );
        }

        // Spawn background task to compute receipt root and logs bloom incrementally.
        // Unbounded channel is used since tx count bounds capacity anyway (max ~30k txs per block).
        let receipts_len = input.transaction_count();
        let (receipt_tx, receipt_rx) = crossbeam_channel::unbounded();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let task_handle = ReceiptRootTaskHandle::new(receipt_rx, result_tx);
        self.runtime.spawn_blocking_named("receipt-root", move || task_handle.run(receipts_len));

        let transaction_count = input.transaction_count();
        let executed_tx_index = Arc::clone(handle.executed_tx_index());
        executor.evm_mut().db_mut().set_state_hook(state_hook);

        let execution_start = Instant::now();

        // Execute all transactions and finalize
        let (executor, senders) = self.execute_transactions(
            executor,
            transaction_count,
            handle.iter_transactions(),
            &receipt_tx,
            &executed_tx_index,
        )?;
        drop(receipt_tx);

        // Finish execution and get the result
        let post_exec_start = Instant::now();
        let (_evm, result) = debug_span!(target: "engine::tree", "BlockExecutor::finish")
            .in_scope(|| executor.finish())
            .map(|(evm, result)| (evm.into_db(), result))?;
        self.metrics.record_post_execution(post_exec_start.elapsed());

        // Merge transitions into bundle state
        debug_span!(target: "engine::tree", "merge_transitions")
            .in_scope(|| db.merge_transitions(BundleRetention::Reverts));

        let output = BlockExecutionOutput { result, state: db.take_bundle() };

        let execution_duration = execution_start.elapsed();
        self.metrics.record_block_execution(&output, execution_duration);
        self.metrics.record_block_execution_gas_bucket(output.result.gas_used, execution_duration);
        debug!(target: "engine::tree::payload_validator", elapsed = ?execution_duration, "Executed block");

        Ok((output, senders, result_rx))
    }

    /// Firehose-enabled twin of [`Self::execute_block`].
    ///
    /// This mirrors `execute_block` with two surgical differences:
    ///   - the EVM is built via `evm_with_env_and_inspector` so the Firehose inspector borrowed
    ///     from `tracer` is attached;
    ///   - the block executor is wrapped in a [`FirehoseWrappedExecutor`] carrying the OP
    ///     [`OpPreTxAdjust`] + [`OpPostTxExtras`] chain hooks so the tracer sees deposit-tx nonce
    ///     adjustments and the BaseFeeVault / L1FeeVault / OperatorFeeVault credits.
    ///
    /// MAINTENANCE CONTRACT: keep this function in sync with [`Self::execute_block`]. Any change
    /// to the non-traced logic (new metrics, error handling, ordering of pre/post steps, etc.)
    /// must be mirrored here.
    #[instrument(level = "debug", target = "engine::tree::payload_validator", skip_all)]
    #[expect(clippy::type_complexity)]
    fn execute_and_trace_block<S, Err, T>(
        &mut self,
        state_provider: S,
        env: ExecutionEnv<Evm>,
        input: &BlockOrPayload<T>,
        tracer: &mut FirehoseBlockTracer,
        handle: &mut PayloadHandle<impl ExecutableTxFor<Evm>, Err, N::Receipt>,
    ) -> Result<
        (
            BlockExecutionOutput<N::Receipt>,
            Vec<Address>,
            tokio::sync::oneshot::Receiver<(B256, alloy_primitives::Bloom)>,
        ),
        InsertBlockErrorKind,
    >
    where
        S: StateProvider + Send,
        Err: core::error::Error + Send + Sync + 'static,
        V: PayloadValidator<T, Block = N::Block>,
        T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>,
        Evm: ConfigureEngineEvm<T::ExecutionData, Primitives = N>,
        Evm::BlockExecutorFactory:
            alloy_evm::block::BlockExecutorFactory<EvmFactory = OpEvmFactory<OpTx>>,
        TxTy<N>: alloy_consensus::Transaction
            + alloy_consensus::transaction::TxHashRef
            + SignatureFields,
    {
        debug!(target: "engine::tree::payload_validator", "Executing block (with Firehose tracing)");

        let mut db = debug_span!(target: "engine::tree", "build_state_db").in_scope(|| {
            State::builder()
                .with_database(StateProviderDatabase::new(state_provider))
                .with_bundle_update()
                .build()
        });

        let ctx =
            self.execution_ctx_for(input).map_err(|e| InsertBlockErrorKind::Other(Box::new(e)))?;

        // Install the Firehose inspector on the EVM — the inspector borrows the tracer until the
        // executor is consumed in `finish()`.
        let spec_id = *env.evm_env.spec_id();
        let inspector = tracer.inspector();
        let evm = self.evm_config.evm_with_env_and_inspector(&mut db, env.evm_env, inspector);

        let inner_executor = self.evm_config.create_executor(evm, ctx);

        // Pre-materialize withdrawals so the Firehose wrapper can emit them as part of the
        // end-of-block event.
        let withdrawals =
            input.withdrawals().map(|w| alloy_eips::eip4895::Withdrawals::new(w.to_vec()));
        let mut executor = FirehoseWrappedExecutor::with_hooks(
            inner_executor,
            withdrawals,
            OpPreTxAdjust,
            OpPostTxExtras,
        );

        if !self.config.precompile_cache_disabled() {
            let _span = debug_span!(target: "engine::tree", "setup_precompile_cache").entered();
            executor.evm_mut().precompiles_mut().map_cacheable_precompiles(
                |address, precompile| {
                    let metrics = self
                        .precompile_cache_metrics
                        .entry(*address)
                        .or_insert_with(|| CachedPrecompileMetrics::new_with_address(*address))
                        .clone();
                    CachedPrecompile::wrap(
                        precompile,
                        self.precompile_cache_map.cache_for_address(*address),
                        spec_id,
                        Some(metrics),
                    )
                },
            );
        }

        // Spawn background task to compute receipt root and logs bloom incrementally.
        let receipts_len = input.transaction_count();
        let (receipt_tx, receipt_rx) = crossbeam_channel::unbounded();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let task_handle = ReceiptRootTaskHandle::new(receipt_rx, result_tx);
        self.runtime.spawn_blocking_named("receipt-root", move || task_handle.run(receipts_len));

        let transaction_count = input.transaction_count();
        let executed_tx_index = Arc::clone(handle.executed_tx_index());
        // No state hook: `PayloadHandle::state_hook()` was removed along with the old
        // multiproof/`StateRootTask` machinery it fed (see the module-doc note near this file's
        // `state_root_strategy` import). This clone always computes the state root after
        // execution instead of streaming updates into a background sparse-trie job.
        executor.evm_mut().db_mut().set_state_hook(None);

        let execution_start = Instant::now();

        let (executor, senders) = self.execute_transactions(
            executor,
            transaction_count,
            handle.iter_transactions(),
            &receipt_tx,
            &executed_tx_index,
        )?;
        drop(receipt_tx);

        let post_exec_start = Instant::now();
        let (_evm, result) = debug_span!(target: "engine::tree", "BlockExecutor::finish")
            .in_scope(|| executor.finish())
            .map(|(evm, result)| (evm.into_db(), result))?;
        self.metrics.record_post_execution(post_exec_start.elapsed());

        debug_span!(target: "engine::tree", "merge_transitions")
            .in_scope(|| db.merge_transitions(BundleRetention::Reverts));

        let output = BlockExecutionOutput { result, state: db.take_bundle() };

        let execution_duration = execution_start.elapsed();
        self.metrics.record_block_execution(&output, execution_duration);
        self.metrics.record_block_execution_gas_bucket(output.result.gas_used, execution_duration);
        debug!(
            target: "engine::tree::payload_validator",
            elapsed = ?execution_duration,
            "Executed block (with Firehose tracing)"
        );

        Ok((output, senders, result_rx))
    }

    /// Executes transactions and collects senders, streaming receipts to a background task.
    ///
    /// This method handles:
    /// - Applying pre-execution changes (e.g., beacon root updates)
    /// - Executing each transaction with timing metrics
    /// - Streaming receipts to the receipt root computation task
    /// - Collecting transaction senders for later use
    ///
    /// Returns the executor (for finalization) and the collected senders.
    fn execute_transactions<E, Tx, InnerTx, Err>(
        &self,
        mut executor: E,
        transaction_count: usize,
        transactions: impl Iterator<Item = Result<Tx, Err>>,
        receipt_tx: &crossbeam_channel::Sender<IndexedReceipt<N::Receipt>>,
        executed_tx_index: &AtomicUsize,
    ) -> Result<(E, Vec<Address>), BlockExecutionError>
    where
        E: BlockExecutor<Receipt = N::Receipt>,
        Tx: alloy_evm::block::ExecutableTx<E> + alloy_evm::RecoveredTx<InnerTx>,
        InnerTx: TxHashRef,
        Err: core::error::Error + Send + Sync + 'static,
    {
        let mut senders = Vec::with_capacity(transaction_count);

        // Apply pre-execution changes (e.g., beacon root update)
        let pre_exec_start = Instant::now();
        debug_span!(target: "engine::tree", "pre_execution")
            .in_scope(|| executor.apply_pre_execution_changes())?;
        self.metrics.record_pre_execution(pre_exec_start.elapsed());

        // Execute transactions
        let exec_span = debug_span!(target: "engine::tree", "execution").entered();
        let mut transactions = transactions.into_iter();
        // Some executors may execute transactions that do not append receipts during the
        // main loop (e.g., system transactions whose receipts are added during finalization).
        // In that case, invoking the callback on every transaction would resend the previous
        // receipt with the same index and can panic the ordered root builder.
        let mut last_sent_len = 0usize;
        loop {
            // Measure time spent waiting for next transaction from iterator
            // (e.g., parallel signature recovery)
            let wait_start = Instant::now();
            let Some(tx_result) = transactions.next() else {
                break;
            };
            self.metrics.record_transaction_wait(wait_start.elapsed());

            let tx = tx_result.map_err(BlockExecutionError::other)?;
            let tx_signer = *<Tx as alloy_evm::RecoveredTx<InnerTx>>::signer(&tx);

            senders.push(tx_signer);

            let _enter = debug_span!(
                target: "engine::tree",
                "execute tx",
                tx_index = senders.len() - 1,
            )
            .entered();
            trace!(target: "engine::tree", "Executing transaction");

            let tx_start = Instant::now();
            executor.execute_transaction(tx)?;
            self.metrics.record_transaction_execution(tx_start.elapsed());

            // advance the shared counter so prewarm workers skip already-executed txs
            executed_tx_index.store(senders.len(), Ordering::Relaxed);

            let current_len = executor.receipts().len();
            if current_len > last_sent_len {
                last_sent_len = current_len;
                // Send the latest receipt to the background task for incremental root computation.
                if let Some(receipt) = executor.receipts().last() {
                    let tx_index = current_len - 1;
                    let _ = receipt_tx.send(IndexedReceipt::new(tx_index, receipt.clone()));
                }
            }
        }
        drop(exec_span);

        Ok((executor, senders))
    }

    /// Validates the block after execution.
    ///
    /// This performs:
    /// - header + pre-execution consensus validation (deferred here since this OP clone does not
    ///   use the background `spawn_convert_and_validate` path)
    /// - parent header validation
    /// - post-execution consensus validation
    ///
    /// If `receipt_root_bloom` is provided, it will be used instead of computing the receipt root
    /// and logs bloom from the receipts.
    ///
    /// Hashed-state-based post-execution validation is run separately by the caller (via
    /// [`PayloadValidator::validate_block_post_execution_with_hashed_state`]) so it can be re-run
    /// against a refreshed hashed state if the state-root job's fallback path recomputes one.
    #[instrument(level = "debug", target = "engine::tree::payload_validator", skip_all)]
    fn validate_post_execution<T: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>>(
        &self,
        block: &RecoveredBlock<N::Block>,
        parent_block: &SealedHeader<N::BlockHeader>,
        output: &BlockExecutionOutput<N::Receipt>,
        ctx: &mut TreeCtx<'_, N>,
        transaction_root: Option<B256>,
        receipt_root_bloom: Option<ReceiptRootBloom>,
    ) -> Result<(), InsertBlockErrorKind>
    where
        V: PayloadValidator<T, Block = N::Block>,
    {
        let start = Instant::now();

        trace!(target: "engine::tree::payload_validator", block=?block.num_hash(), "Validating block consensus");
        // validate block consensus rules
        if let Err(e) = self.validate_block_inner(block, transaction_root) {
            return Err(e.into());
        }

        // now validate against the parent
        let _enter = debug_span!(target: "engine::tree::payload_validator", "validate_header_against_parent").entered();
        if let Err(e) =
            self.consensus.validate_header_against_parent(block.sealed_header(), parent_block)
        {
            warn!(target: "engine::tree::payload_validator", ?block, "Failed to validate header {} against parent: {e}", block.hash());
            return Err(e.into());
        }
        drop(_enter);

        // Validate block post-execution rules
        let _enter =
            debug_span!(target: "engine::tree::payload_validator", "validate_block_post_execution")
                .entered();
        if let Err(err) =
            self.consensus.validate_block_post_execution(block, output, receipt_root_bloom, None)
        {
            // call post-block hook
            self.on_invalid_block(parent_block, block, output, None, ctx.state_mut());
            return Err(err.into());
        }
        drop(_enter);

        // record post-execution validation duration
        self.metrics
            .block_validation
            .post_execution_validation_duration
            .record(start.elapsed().as_secs_f64());

        Ok(())
    }

    /// Spawns transaction conversion and cache prewarming for payload validation.
    ///
    /// State-root tasks are prepared before this method and can provide capabilities that
    /// prewarm uses for BAL-derived authoritative updates or transaction-derived hints.
    #[instrument(
        level = "debug",
        target = "engine::tree::payload_validator",
        skip_all,
        fields(
            has_hint_stream = hint_stream.is_some(),
            has_hashed_update_stream = hashed_update_stream.is_some(),
            parallel_bal_execution
        )
    )]
    fn spawn_payload_processor<T: ExecutableTxIterator<Evm>>(
        &self,
        env: ExecutionEnv<Evm>,
        txs: T,
        provider_builder: StateProviderBuilder<N, P>,
        hint_stream: Option<StateRootHintStream>,
        hashed_update_stream: Option<StateRootUpdateStream>,
        parallel_bal_execution: bool,
    ) -> Result<
        PayloadHandle<
            impl ExecutableTxFor<Evm> + use<N, P, Evm, V, T>,
            impl core::error::Error + Send + Sync + 'static + use<N, P, Evm, V, T>,
            N::Receipt,
        >,
        InsertBlockErrorKind,
    > {
        let start = Instant::now();
        let handle = self.payload_processor.spawn_with_state_root_streams(
            env,
            txs,
            provider_builder,
            hint_stream,
            hashed_update_stream,
            parallel_bal_execution,
        );

        self.metrics.block_validation.spawn_payload_processor.record(start.elapsed().as_secs_f64());

        Ok(handle)
    }

    /// Creates a `StateProviderBuilder` for the given parent hash.
    ///
    /// This method checks if the parent is in the tree state (in-memory) or persisted to disk,
    /// and creates the appropriate provider builder.
    fn state_provider_builder(
        &self,
        hash: B256,
        state: &EngineApiTreeState<N>,
    ) -> ProviderResult<Option<StateProviderBuilder<N, P>>> {
        if let Some((historical, blocks)) = state.tree_state().blocks_by_hash(hash) {
            debug!(target: "engine::tree::payload_validator", %hash, %historical, "found canonical state for block in memory, creating provider builder");
            // the block leads back to the canonical chain
            return Ok(Some(StateProviderBuilder::new(
                self.provider.clone(),
                historical,
                Some(blocks),
            )));
        }

        // Check if the block is persisted
        if let Some(header) = self.provider.header(hash)? {
            debug!(target: "engine::tree::payload_validator", %hash, number = %header.number(), "found canonical state for block in database, creating provider builder");
            // For persisted blocks, we create a builder that will fetch state directly from the
            // database
            return Ok(Some(StateProviderBuilder::new(self.provider.clone(), hash, None)));
        }

        debug!(target: "engine::tree::payload_validator", %hash, "no canonical state found for block");
        Ok(None)
    }

    /// Called when an invalid block is encountered during validation.
    fn on_invalid_block(
        &self,
        parent_header: &SealedHeader<N::BlockHeader>,
        block: &RecoveredBlock<N::Block>,
        output: &BlockExecutionOutput<N::Receipt>,
        trie_updates: Option<(&TrieUpdates, B256)>,
        state: &mut EngineApiTreeState<N>,
    ) {
        if state.has_invalid_header(&block.hash()) {
            // we already marked this block as invalid
            return;
        }
        self.invalid_block_hook.on_invalid_block(parent_header, block, output, trie_updates);
    }

    /// Compute state root for the given hashed post state in serial.
    ///
    /// Uses the same provider construction path as main execution and computes the state root and
    /// trie updates for this block directly via
    /// [`reth_provider::StateRootProvider::state_root_with_updates`].
    fn compute_state_root_serial(
        state_provider: StateProviderBox,
        hashed_state: &LazyHashedPostState,
    ) -> ProviderResult<(B256, TrieUpdates)> {
        state_provider.state_root_with_updates(hashed_state.get().clone())
    }

    /// Spawns a background task to compute and sort trie data for the executed block.
    ///
    /// This function creates a [`LazyTrieData`] handle and spawns a blocking task that:
    /// 1. Sort the block's hashed state and trie updates
    /// 2. Publishes the result so subsequent calls return immediately
    ///
    /// If the background task hasn't completed when `trie_data()` is called, callers wait for the
    /// publishing task instead of computing synchronously.
    ///
    /// The validation hot path can return immediately after state root verification,
    /// while consumers (DB writes, overlay providers, proofs) get trie data from the completed
    /// task.
    fn spawn_deferred_trie_task(
        &self,
        block: RecoveredBlock<N::Block>,
        execution_outcome: Arc<BlockExecutionOutput<N::Receipt>>,
        hashed_state: LazyHashedPostState,
        trie_output: Arc<TrieUpdates>,
        changed_paths: Option<Arc<TriePrefixSetsMut>>,
    ) -> ExecutedBlock<N> {
        // Create deferred handle and task that owns the unsorted inputs.
        // Resolve the lazy handle into Arc<HashedPostState>. By this point the hashed state has
        // already been computed and used for state root verification, so .get() returns instantly.
        let hashed_state = match hashed_state.try_into_inner() {
            Ok(state) => Arc::new(state),
            Err(handle) => Arc::new(handle.get().clone()),
        };
        let (deferred_trie_data, deferred_trie_task) =
            LazyTrieData::pending(hashed_state, trie_output, changed_paths);
        let block_validation_metrics = self.metrics.block_validation.clone();

        // Capture block info for tracing.
        let block_number = block.number();

        // Spawn background task to compute trie data.
        let compute_trie_input_task = move || {
            let _span = debug_span!(
                target: "engine::tree::payload_validator",
                "compute_trie_input_task",
                block_number
            )
            .entered();

            let compute_start = Instant::now();
            let computed = deferred_trie_task.compute_and_publish();
            block_validation_metrics
                .deferred_trie_compute_duration
                .record(compute_start.elapsed().as_secs_f64());

            // Record sizes of the computed trie data
            block_validation_metrics
                .hashed_post_state_size
                .record(computed.sorted.hashed_state.total_len() as f64);
            block_validation_metrics
                .trie_updates_sorted_size
                .record(computed.sorted.trie_updates.total_len() as f64);
        };

        // Spawn task that computes trie data asynchronously.
        self.runtime.spawn_blocking_named(DEFERRED_TRIE_WORKER_NAME, compute_trie_input_task);

        ExecutedBlock::with_deferred_trie_data(
            Arc::new(block),
            execution_outcome,
            deferred_trie_data,
        )
    }

    fn calculate_timing_stats(
        &self,
        block: &RecoveredBlock<N::Block>,
        provider_stats: Arc<StateProviderStats>,
        cache_stats: Option<Arc<CacheStats>>,
        output: &BlockExecutionOutput<N::Receipt>,
        execution_duration: Duration,
        state_hash_duration: Duration,
    ) -> Box<ExecutionTimingStats> {
        let accounts_read = provider_stats.total_account_fetches();
        let storage_read = provider_stats.total_storage_fetches();
        let code_read = provider_stats.total_code_fetches();
        let code_bytes_read = provider_stats.total_code_fetched_bytes();

        // Write stats from BundleState (final state changes)
        let accounts_changed = output.state.state.len();
        let accounts_deleted =
            output.state.state.values().filter(|acc| acc.was_destroyed()).count();
        let storage_slots_changed =
            output.state.state.values().map(|account| account.storage.len()).sum::<usize>();
        let storage_slots_deleted = output
            .state
            .state
            .values()
            .flat_map(|account| account.storage.values())
            .filter(|slot| {
                slot.present_value.is_zero() && !slot.previous_or_original_value.is_zero()
            })
            .count();

        // Helper: check if account represents a new contract deployment
        let is_new_deployment = |acc: &BundleAccount| -> bool {
            let has_code_now = acc.info.as_ref().is_some_and(|info| info.code_hash != KECCAK_EMPTY);
            let had_no_code_before = acc
                .original_info
                .as_ref()
                .map(|info| info.code_hash == KECCAK_EMPTY)
                .unwrap_or(true);
            has_code_now && had_no_code_before
        };

        let bytecodes_changed =
            output.state.state.values().filter(|acc| is_new_deployment(acc)).count();

        // Unique new code hashes to count actual bytes persisted (deduplicated)
        let unique_new_code_hashes: B256Set = output
            .state
            .state
            .values()
            .filter(|acc| is_new_deployment(acc))
            .filter_map(|acc| acc.info.as_ref().map(|info| info.code_hash))
            .collect();
        let code_bytes_written: usize = unique_new_code_hashes
            .iter()
            .filter_map(|hash| {
                output.state.contracts.get(hash).map(|bytecode| bytecode.original_bytes().len())
            })
            .sum();

        // Total time spent fetching state during execution
        let state_read_duration = provider_stats.total_account_fetch_latency() +
            provider_stats.total_storage_fetch_latency() +
            provider_stats.total_code_fetch_latency();

        // EIP-7702 delegation tracking from bytecode changes
        // Count new EIP-7702 bytecodes as delegations set
        let eip7702_delegations_set =
            output.state.contracts.values().filter(|bytecode| bytecode.is_eip7702()).count();
        // Delegations cleared: accounts where bytecode changed FROM EIP-7702 TO empty
        // This detects when an EIP-7702 delegation is removed by setting code to empty
        // Note: Clearing a delegation does NOT destroy the account - it just empties the
        // bytecode
        let eip7702_delegations_cleared = output
            .state
            .state
            .values()
            .filter(|acc| {
                // Check if original bytecode was EIP-7702
                let original_was_eip7702 = acc
                    .original_info
                    .as_ref()
                    .and_then(|info| info.code.as_ref())
                    .map(|bytecode| bytecode.is_eip7702())
                    .unwrap_or(false);

                // Check if current code is empty (delegation cleared)
                let code_now_empty =
                    acc.info.as_ref().map(|info| info.code_hash == KECCAK_EMPTY).unwrap_or(false);

                original_was_eip7702 && code_now_empty
            })
            .count();

        // Get cache statistics for detailed block logging
        let (account_cache_hits, account_cache_misses) = cache_stats
            .as_ref()
            .map(|s| (s.account_hits(), s.account_misses()))
            .unwrap_or_default();
        let (storage_cache_hits, storage_cache_misses) = cache_stats
            .as_ref()
            .map(|s| (s.storage_hits(), s.storage_misses()))
            .unwrap_or_default();
        let (code_cache_hits, code_cache_misses) =
            cache_stats.as_ref().map(|s| (s.code_hits(), s.code_misses())).unwrap_or_default();

        // Build execution timing stats for detailed block logging
        Box::new(ExecutionTimingStats {
            block_number: block.number(),
            block_hash: block.hash(),
            gas_used: output.result.gas_used,
            tx_count: block.transaction_count(),
            execution_duration,
            state_read_duration,
            state_hash_duration,
            accounts_read,
            storage_read,
            code_read,
            code_bytes_read,
            accounts_changed,
            accounts_deleted,
            storage_slots_changed,
            storage_slots_deleted,
            bytecodes_changed,
            code_bytes_written,
            eip7702_delegations_set,
            eip7702_delegations_cleared,
            account_cache_hits,
            account_cache_misses,
            storage_cache_hits,
            storage_cache_misses,
            code_cache_hits,
            code_cache_misses,
        })
    }
}

impl<N, Types, P, Evm, V> EngineValidator<Types> for OpFirehoseEngineValidator<P, Evm, V>
where
    P: DatabaseProviderFactory<
            Provider: BlockReader
                          + StageCheckpointReader
                          + PruneCheckpointReader
                          + ChangeSetReader
                          + StorageChangeSetReader
                          + BlockNumReader
                          + StorageSettingsCache,
        > + BlockReader<Header = N::BlockHeader>
        + StateProviderFactory
        + StateReader
        + ChangeSetReader
        + BlockNumReader
        + HashedPostStateProvider
        + Clone
        + 'static,
    OverlayStateProviderFactory<P, N>: DatabaseProviderROFactory<Provider: TrieCursorFactory + HashedCursorFactory>
        + Clone
        + Send
        + Sync
        + 'static,
    N: NodePrimitives,
    V: PayloadValidator<Types, Block = N::Block> + Clone,
    Evm: ConfigureEngineEvm<Types::ExecutionData, Primitives = N> + 'static,
    Evm::BlockExecutorFactory:
        alloy_evm::block::BlockExecutorFactory<EvmFactory = OpEvmFactory<OpTx>>,
    Types: PayloadTypes<BuiltPayload: BuiltPayload<Primitives = N>>,
    TxTy<N>:
        alloy_consensus::Transaction + alloy_consensus::transaction::TxHashRef + SignatureFields,
{
    fn validate_payload_attributes_against_header(
        &self,
        attr: &Types::PayloadAttributes,
        header: &N::BlockHeader,
    ) -> Result<(), InvalidPayloadAttributesError> {
        self.validator.validate_payload_attributes_against_header(attr, header)
    }

    fn convert_payload_to_block(
        &self,
        payload: Types::ExecutionData,
    ) -> Result<SealedBlock<N::Block>, NewPayloadError> {
        let block = self.validator.convert_payload_to_block(payload)?;
        Ok(block)
    }

    fn validate_payload(
        &mut self,
        payload: Types::ExecutionData,
        ctx: TreeCtx<'_, N>,
    ) -> ValidationOutcome<N> {
        self.validate_block_with_state(BlockOrPayload::Payload(payload), ctx).map(
            |(executed_block, execution_timing_stats)| {
                ValidationOutput::new(executed_block, execution_timing_stats)
            },
        )
    }

    fn validate_block(
        &mut self,
        block: SealedBlock<N::Block>,
        ctx: TreeCtx<'_, N>,
    ) -> ValidationOutcome<N> {
        self.validate_block_with_state(BlockOrPayload::Block(block), ctx).map(
            |(executed_block, execution_timing_stats)| {
                ValidationOutput::new(executed_block, execution_timing_stats)
            },
        )
    }

    fn on_inserted_executed_block(
        &self,
        block: BuiltPayloadExecutedBlock<N>,
    ) -> ProviderResult<ExecutedBlock<N>> {
        self.payload_processor.on_inserted_executed_block(
            block.recovered_block.block_with_parent(),
            &block.execution_output.state,
        );

        Ok(self.spawn_deferred_trie_task(
            (*block.recovered_block).clone(),
            block.execution_output,
            reth_tasks::LazyHandle::ready((*block.hashed_state).clone()),
            block.trie_updates,
            block.changed_paths,
        ))
    }

    fn cache_for(&self, block_hash: B256) -> Option<SavedCache> {
        Some(self.payload_processor.cache_for(block_hash))
    }

    fn payload_state_root_handle_for(
        &self,
        _parent_hash: B256,
        _parent_header: &N::BlockHeader,
        _timestamp: u64,
        _state: &mut EngineApiTreeState<N>,
    ) -> Option<PayloadStateRootHandle> {
        // Always decline: per this method's own contract ("Returns `None` when the strategy
        // declines, in which case the payload builder computes the state root itself"), `None`
        // is a fully supported, correct answer — not a stub. We cannot construct a
        // `PayloadStateRootJobContext` to hand to a custom `StateRootStrategy` here (its `::new`
        // is `pub(crate)` in `reth_engine_tree`; see the module-doc note above this file's
        // `state_root_strategy` import), so the payload builder always falls back to computing
        // its own state root synchronously. This has no effect on Firehose tracing.
        None
    }
}

impl<P, Evm, V> WaitForCaches for OpFirehoseEngineValidator<P, Evm, V>
where
    Evm: ConfigureEvm,
{
    fn wait_for_caches(&self) -> CacheWaitDurations {
        debug!(target: "engine::tree::payload_validator", "Waiting for sparse trie locks");

        // `PayloadProcessor::execution_cache()` is `pub(crate)` in `reth_engine_tree` and
        // inaccessible here (see the module-doc note near this file's `state_root_strategy`
        // import), so we cannot wait on the shared execution cache directly. This trait method is
        // only reachable via the opt-in `RethNewPayload` engine RPC extension's
        // `wait_for_caches: bool` flag (a reth-specific debug/benchmarking request, not part of
        // standard `engine_newPayload`), so reporting a zero wait here does not affect standard
        // validation, Firehose tracing, or consensus behavior — only that extension's reported
        // timing when `wait_for_caches` is explicitly requested.
        let execution_cache = Duration::ZERO;
        let state_trie_overlays = self.state_trie_overlays.clone();
        let (sparse_trie_tx, sparse_trie_rx) = std::sync::mpsc::channel();

        self.runtime.spawn_blocking_named("wait-sparse-tri", move || {
            let _ = sparse_trie_tx.send(state_trie_overlays.wait_for_sparse_trie_availability());
        });

        let sparse_trie =
            sparse_trie_rx.recv().expect("sparse trie wait task failed to send result");
        debug!(
            target: "engine::tree::payload_validator",
            ?execution_cache,
            ?sparse_trie,
            "Execution cache and sparse trie locks acquired"
        );
        CacheWaitDurations { execution_cache, sparse_trie }
    }
}

/// [`reth_node_builder::rpc::EngineValidatorBuilder`] for the OP Stack Firehose engine validator.
///
/// Mirrors the upstream `BasicEngineValidatorBuilder` from `reth_node_builder::rpc` but
/// constructs an [`OpFirehoseEngineValidator`] instead of `BasicEngineValidator`. The OP node
/// installs this builder in its [`reth_node_builder::Node::AddOns`] tuple to wire the live
/// engine-API execution path through the Firehose `execute_and_trace_block` codepath when the
/// global tracer is initialized.
#[derive(Debug, Clone)]
pub struct OpFirehoseEngineValidatorBuilder<EV> {
    /// The payload validator builder used to create the engine validator.
    payload_validator_builder: EV,
}

impl<EV> OpFirehoseEngineValidatorBuilder<EV> {
    /// Creates a new instance with the given payload validator builder.
    pub const fn new(payload_validator_builder: EV) -> Self {
        Self { payload_validator_builder }
    }
}

impl<EV> Default for OpFirehoseEngineValidatorBuilder<EV>
where
    EV: Default,
{
    fn default() -> Self {
        Self::new(EV::default())
    }
}

impl<Node, EV> reth_node_builder::rpc::EngineValidatorBuilder<Node>
    for OpFirehoseEngineValidatorBuilder<EV>
where
    Node: reth_node_api::FullNodeComponents<
            Evm: ConfigureEngineEvm<
                <<Node::Types as reth_node_api::NodeTypes>::Payload as PayloadTypes>::ExecutionData,
            > + ConfigureEvm<
                BlockExecutorFactory: alloy_evm::block::BlockExecutorFactory<
                    EvmFactory = OpEvmFactory<OpTx>,
                >,
            >,
        >,
    EV: reth_node_builder::rpc::PayloadValidatorBuilder<Node>,
    EV::Validator: reth_engine_primitives::PayloadValidator<
            <Node::Types as reth_node_api::NodeTypes>::Payload,
            Block = reth_node_api::BlockTy<Node::Types>,
        > + Clone,
    TxTy<<Node::Types as reth_node_api::NodeTypes>::Primitives>:
        alloy_consensus::Transaction + alloy_consensus::transaction::TxHashRef + SignatureFields,
{
    type EngineValidator = OpFirehoseEngineValidator<Node::Provider, Node::Evm, EV::Validator>;

    async fn build_tree_validator(
        self,
        ctx: &reth_node_api::AddOnsContext<'_, Node>,
        tree_config: TreeConfig,
        changeset_cache: ChangesetCache,
        state_trie_overlays: StateTrieOverlayManager<
            <Node::Types as reth_node_api::NodeTypes>::Primitives,
        >,
    ) -> eyre::Result<Self::EngineValidator> {
        use reth_chainspec::EthChainSpec;
        use reth_node_builder::invalid_block_hook::InvalidBlockHookExt;
        let validator = self.payload_validator_builder.build(ctx).await?;
        let data_dir = ctx.config.datadir.clone().resolve_datadir(ctx.config.chain.chain());
        let invalid_block_hook = ctx.create_invalid_block_hook(&data_dir).await?;

        Ok(OpFirehoseEngineValidator::new(
            ctx.node.provider().clone(),
            std::sync::Arc::new(ctx.node.consensus().clone()),
            ctx.node.evm_config().clone(),
            validator,
            tree_config,
            invalid_block_hook,
            changeset_cache,
            state_trie_overlays,
            ctx.node.task_executor().clone(),
        ))
    }
}
