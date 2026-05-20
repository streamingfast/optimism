//! [`reth_firehose::mapper::SignatureFields`] impl for [`OpTxEnvelope`].
//!
//! This impl must live in `op-alloy-consensus` (the crate that defines `OpTxEnvelope`) to
//! satisfy Rust's orphan rules — both `SignatureFields` and `OpTxEnvelope` are otherwise
//! foreign types to any potential implementor crate (`reth-optimism-firehose`,
//! `reth-optimism-primitives`, ...).
//!
//! The impl is enabled by the optional `firehose` feature on `op-alloy-consensus` so the
//! published crate does not unconditionally pull in `reth-firehose`. Consumers needing
//! Firehose tracing (today: `reth-optimism-firehose`) opt in by enabling the feature.

use alloy_consensus::Transaction as _;
use alloy_primitives::{B256, Bytes};
use reth_firehose::mapper::{SignatureFields, u64_to_trimmed_bytes};

use crate::OpTxEnvelope;

impl SignatureFields for OpTxEnvelope {
    fn signature_fields(&self) -> (B256, B256, Bytes) {
        // OP Deposit and PostExec txs are unsigned — return zero r/s and an empty v, matching
        // base-reth's convention.
        let Some(sig) = self.signature() else {
            return (B256::ZERO, B256::ZERO, Bytes::new());
        };
        let y_parity = sig.v() as u64;
        let v = match self {
            // Legacy without EIP-155: V = 27 or 28
            // Legacy with EIP-155: V = chain_id * 2 + 35 + y_parity
            Self::Legacy(signed) => match signed.tx().chain_id() {
                Some(chain_id) => chain_id * 2 + 35 + y_parity,
                None => 27 + y_parity,
            },
            _ => y_parity,
        };
        (
            B256::new(sig.r().to_be_bytes::<32>()),
            B256::new(sig.s().to_be_bytes::<32>()),
            u64_to_trimmed_bytes(v),
        )
    }
}
