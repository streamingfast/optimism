# Network activations

Every `OpHardfork` the op-reth of this fork defines, with its status on the networks the Firehose
op-reth image serves and its effect on the Firehose block. Review the entries that are new or whose
status changed at every upstream bump, and when an activation is announced between bumps.

Statuses come from the timestamps compiled into `op-reth-v2.9.0-fh3.1`: OP Mainnet from
`rust/alloy-op-hardforks/src/optimism/mainnet.rs`, Unichain from the `superchain-registry`
submodule (`88ee79a3`) that the chainspec embeds at build time. Checked 2026-11-16. op-reth v2.9.0
defines no new fork, and its registry update only adds the `sepolia-devnet-4`, `-5` and `-6`
configs: the OP Mainnet and Unichain fork times are unchanged. "Not scheduled"
means the fork has no timestamp in the code: it reaches the tracer only once a later release, or a
registry update picked up by a later release, schedules it.

World Chain runs from the `release/world-chain-2.x` line and is not covered here.

| Fork | OP Mainnet | Unichain | Effect on the Firehose block |
|------|------------|----------|------------------------------|
| Bedrock, Regolith | Active (Bedrock at block 105235063) | Active (genesis) | Deposit transactions (type 126) are traced with the sender's pre-execution nonce and an `IncreaseMint` balance change (`OpPreTxAdjust`). |
| Canyon, Ecotone, Fjord, Granite, Holocene | Active (2024-01-11 to 2025-01-09) | Active (genesis to 2025-01-09) | Covered by the Firehose releases before this file existed; not re-reviewed for this file. Ecotone adds the EIP-4788 beacon root system call, traced in `systemCalls` through the `alloy-op-evm` inspector patch. |
| Isthmus | Active (2025-05-09) | Active (2025-05-09) | EIP-2935 parent hash system call; EIP-7702 transactions; the OperatorFeeVault (`0x4200…001b`) credit, emitted by `OpPostTxExtras` through upstream `operator_fee_charge`. |
| Jovian | Active (2025-12-02) | Active (2025-12-02) | Operator fee formula change, followed because `OpPostTxExtras` calls upstream `operator_fee_charge` with the active spec rather than a copy. Header `blobGasUsed` carries the DA footprint and is mapped as any header value. |
| Karst | Active (2026-07-08) | Active (2026-07-08) | Osaka EVM rules (EIP-7825 transaction gas cap with deposits exempt, MODEXP and P256VERIFY changes, BN254 pairing input cap). No new transaction type, fee vault or system call. |
| Lagoon | Not scheduled in the code. Optimism's [interop preparation notice](https://docs.optimism.io/notices/interop-prep) targets January 11, 2027, pending governance, with no timestamp published | Not scheduled in the code; same notice and target as OP Mainnet | **Value diffs to review before activation.** Enables the trailing `0x7D` post-exec transaction (`OpTxEnvelope::PostExec`) and post-exec gas refunds (SDM settlement). Since `op-reth-v2.9.0-fh3.1`, `firehose-tracer` 5.5.1 maps `0x7D` to `TRX_TYPE_OPTIMISM_POST_EXEC`, so the node no longer panics on it. The block executor short-circuits that transaction without running the EVM, so its trace has no call, and the tracer then drops its receipt (status unknown). `OpPostTxExtras` still runs for it, reading the previous transaction's environment; the fee vault change it computes is discarded with the empty transaction but uses up a block ordinal. Settlement refunds are applied before the Firehose hooks, which see the settled gas: the coinbase and fee vault credits match the net state change, but the sender's `GasRefund` leaves out the operator fee part of the refund when operator fees are nonzero. No Battlefield devnet activates Lagoon. |
