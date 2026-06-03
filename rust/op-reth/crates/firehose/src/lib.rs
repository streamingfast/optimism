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
/// called at this point") — the genesis marker hits this immediately on the engine-API path.
///
/// The SF reth fork performs the equivalent call inside its Firehose ExEx (`reth_firehose::run_exex`).
/// `op-reth` delivers live tracing through [`OpFirehoseEngineValidator`] instead of that ExEx, so
/// the call has to be wired in explicitly here (see `proof_history::launch_node`). Mirrors the
/// fork's `ChainConfig::new(chain_id)` — fork activation is derived per-block from the header, so no
/// fork timestamps are needed. Reports the client as `"reth"` to match what the fork emits and what
/// the Firehose reader expects.
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
