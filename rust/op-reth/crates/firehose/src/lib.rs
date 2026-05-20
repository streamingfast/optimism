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
