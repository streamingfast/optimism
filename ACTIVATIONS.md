# Network activations

Every `OpHardfork` the op-reth of this fork defines, with its status on the networks the Firehose
op-reth image serves and its effect on the Firehose block. Review the entries that are new or whose
status changed at every upstream bump, and when an activation is announced between bumps.

Statuses come from the timestamps compiled into `op-reth-v2.8.1-fh3.1`: OP Mainnet from
`rust/alloy-op-hardforks/src/optimism/mainnet.rs`, Unichain from the `superchain-registry`
submodule (`13907b9b`) that the chainspec embeds at build time. Checked 2026-11-06. "Not scheduled"
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
| Lagoon | Not scheduled | Not scheduled | **Blocker before activation.** Enables the trailing `0x7D` post-exec transaction (`OpTxEnvelope::PostExec`). `firehose-tracer` 5.5.0 `TxType` has no value 125, and the reth mapper converts with `expect("All transaction types handled for now")` (`streamingfast/reth` `crates/firehose/src/mapper.rs`), so the first such transaction panics the node. Lagoon also turns on post-exec gas refunds, which make receipt `gas_used` differ from EVM gas: check the gas passed to `OpPostTxExtras` against what the handler charges the fee vaults. v2.8.1 can read `lagoon_time` from the superchain registry and adds BLS12-381 MSM and pairing input caps. |
