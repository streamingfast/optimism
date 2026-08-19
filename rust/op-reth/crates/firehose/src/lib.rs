#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// `op-alloy-consensus` is depended on only to activate its `firehose` feature, which carries
// the `SignatureFields` impl for `OpTxEnvelope` that we need to satisfy
// `OpChainHooks` / `OpFirehoseEvmConfig` trait bounds. The crate is otherwise unused here.
use op_alloy_consensus as _;

mod extras;
pub use extras::{OpPostTxExtras, OpPreTxAdjust};

mod evm_config;
pub use evm_config::{OpChainHooks, OpFirehoseEvmConfig};

mod engine_validator;
pub use engine_validator::{OpFirehoseEngineValidator, OpFirehoseEngineValidatorBuilder};

/// Emits the Firehose `FIRE INIT` line and records the chain config on the process-wide tracer.
///
/// This MUST run once, after [`reth_firehose::init_tracer`], and before any block is traced.
/// Without it the tracer's `chain_config` stays `None` and the first traced block panics in
/// `firehose_tracer`'s `ensure_blockchain_init` ("the OnBlockchainInit hook should have been
/// called at this point").
///
/// The SF reth fork performs the equivalent call inside its Firehose ExEx
/// (`reth_firehose::run_exex`). `op-reth` delivers live tracing through
/// [`OpFirehoseEngineValidator`] instead of that ExEx, so the call has to be wired in explicitly
/// here (see `proof_history::launch_node`). Mirrors the fork's `ChainConfig::new(chain_id)` — fork
/// activation is derived per-block from the header, so no fork timestamps are needed. Reports the
/// client as `"reth"` to match what the fork emits and what the Firehose reader expects.
///
/// No-op when the tracer is not initialized, so non-Firehose embeddings (tests, tooling) are safe.
pub fn init_blockchain(chain_id: u64) {
    if reth_firehose::is_tracer_initialized() {
        reth_firehose::tracer().on_blockchain_init(
            "reth",
            env!("CARGO_PKG_VERSION"),
            firehose_tracer::config::ChainConfig::new(chain_id),
        );
    }
}

/// Emits the Firehose genesis block when the node starts on an empty chain (head still at the
/// chain-spec genesis height).
///
/// The genesis block is written to the DB during launch initialization without ever being
/// executed, so no tracing hook fires for it and a from-scratch Firehose stream would start one
/// block after genesis. The SF reth fork performs this emission inside its Firehose ExEx, which
/// `op-reth` does not use — wire this from `on_node_started` instead (the genesis block only
/// exists in the DB once launch has initialized it, so it cannot run alongside
/// [`init_blockchain`]).
///
/// No-op when the tracer is not initialized, so non-Firehose embeddings (tests, tooling) are safe.
pub fn emit_genesis_block_if_empty<P, C>(provider: &P, chain_spec: &C) -> eyre::Result<()>
where
    P: reth_provider::BlockReader,
    <P::Block as reth_primitives_traits::Block>::Header:
        alloy_consensus::BlockHeader + alloy_primitives::Sealable,
    <<P::Block as reth_primitives_traits::Block>::Body as reth_primitives_traits::BlockBody>::OmmerHeader:
        alloy_consensus::BlockHeader + alloy_primitives::Sealable,
    C: reth_chainspec::EthChainSpec,
{
    if !reth_firehose::is_tracer_initialized() {
        return Ok(());
    }
    reth_firehose::emit_genesis_block_on_empty_chain(provider, chain_spec.genesis())
}
