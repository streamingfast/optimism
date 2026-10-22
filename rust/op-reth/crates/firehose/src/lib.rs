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

/// Emits the Firehose `FIRE INIT` line and, when the node starts on an empty chain, the genesis
/// block.
///
/// Call it from the node builder's `on_component_initialized` hook. That hook runs once the
/// database holds the genesis block and before the consensus engine exists, so no block can be
/// traced ahead of `FIRE INIT` (the tracer panics on a block that starts before it).
///
/// Reports the client as `"reth"`, which is what the Firehose reader expects, with this crate's
/// version, so it calls [`reth_firehose::emit_genesis_block_on_empty_chain`] directly instead of
/// [`reth_firehose::init_blockchain`]. Fork activation is derived per block from the header, so
/// `ChainConfig` carries only the chain id.
///
/// No-op when the tracer is not initialized, so non-Firehose embeddings (tests, tooling) are safe.
pub fn init_blockchain<P>(provider: &P) -> eyre::Result<()>
where
    P: reth_provider::BlockReader
        + reth_provider::ChainSpecProvider<ChainSpec: reth_chainspec::EthChainSpec>,
    <P::Block as reth_primitives_traits::Block>::Header:
        alloy_consensus::BlockHeader + alloy_primitives::Sealable,
    <<P::Block as reth_primitives_traits::Block>::Body as reth_primitives_traits::BlockBody>::OmmerHeader:
        alloy_consensus::BlockHeader + alloy_primitives::Sealable,
{
    use reth_chainspec::EthChainSpec;

    if !reth_firehose::is_tracer_initialized() {
        return Ok(());
    }

    let chain_spec = provider.chain_spec();
    reth_firehose::tracer().on_blockchain_init(
        "reth",
        env!("CARGO_PKG_VERSION"),
        firehose_tracer::config::ChainConfig::new(chain_spec.chain().id()),
    );
    reth_firehose::emit_genesis_block_on_empty_chain(provider, chain_spec.genesis())
}
