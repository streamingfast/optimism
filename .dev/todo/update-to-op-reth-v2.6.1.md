# op-reth bump to upstream `op-reth/v2.6.1` (op-reth-v2.6.1-fh3.1)

## Branch layout

- `release/op-reth-2.x` follows tagged upstream `op-reth/vX.Y.Z` releases. Tags:
  `op-reth-vX.Y.Z-fh3.N[-M]`.
- Work branch: `bump/op-reth-v2.6.1`. `op-reth/v2.5.0` is an ancestor of `op-reth/v2.6.1`
  (68 upstream commits; `v2.6.0` is 65 of them, `v2.6.1` adds a Go toolchain bump, an apko
  certificate bump and a webpki bump).

## Reth dependency

Upstream `v2.6.1` moves the reth pin from op-rs/reth `aef8d3ef` (reth v2.4.0 plus 10 commits) to
ethereum-optimism/reth `0fbe4285` (reth v2.5.2 plus 2 commits: an engine handoff fix and a CI
change), with revm 41 -> 42 and alloy-evm 0.37 -> 0.38. `aef8d3ef` is not an ancestor of
`0fbe4285` (it carries an op-rs cherry-pick).

streamingfast/reth branch `bump/op-reth-v2.6.1` (PR into `release/optimism-2.x`):
- Merges `0fbe4285` into `release/optimism-2.x`. Conflicts: `Cargo.lock`, `crates/engine/tree/Cargo.toml`,
  `crates/engine/tree/src/tree/payload_validator.rs`, `crates/rpc/rpc/Cargo.toml`,
  `crates/trie/parallel/src/state_root_task.rs`. git rerere replayed the resolutions recorded
  when the reth-2.x line merged v2.5.2 for the first four; each was checked against tag
  `reth-v2.5.2-fh3.1-3`, and the only differences are reth-2.x-only features
  (`FirehoseLiveHooks`, the EIP-7928 BAL RLP patch). `state_root_task.rs` takes upstream's side.
- Every SF-modified file outside `crates/firehose` now matches `reth-v2.5.2-fh3.1-3`, except
  branch-specific files (changelog, `sf-release.yml`) and the reth-2.x-only features above.
- Firehose adaptations taken from the reth-2.x v2.5.x merges: `JournalEntry::CodeChange` carries
  `had_code_hash` / `had_code` in revm 42 (`inspector.rs`), and `TreeState::state_trie_overlays()`
  returns the new `OverlayManager`.
- `alloy-evm` patch moves to `streamingfast/evm` `v0.38.0-sf`. `firehose-tracer` stays locked at
  5.5.0. The lockfile equals the reth-2.x v2.5.2 one plus upstream's five bumps (aws-lc-rs,
  aws-lc-sys, h2, rustls, rustls-webpki).
- Cherry-picks reth-2.x `4b7797bcf7` (Emit Firehose init and genesis without an ExEx), which
  deletes `runner.rs` (`run_exex`) and the `reth-exex` dependency, and adds `init.rs`.

The PR merged as `d4875c13` and is tagged `op-reth-v2.6.1-fh3.1`, with no file changes from the
branch head `e2428f7e`. This repo points every reth crate at `tag = "op-reth-v2.6.1-fh3.1"`.

## Upstream changes that touch op-reth

Checked against the Firehose bug patterns (serial tracing, emit after acceptance, network
activations, unmapped fields), with code evidence:
- `op-revm` handler rewrite for revm 42's `GasTracker` (#22495): `reward_beneficiary` is byte
  identical between the two tags, so `firehose/src/extras.rs` (BaseFeeVault, L1FeeVault,
  OperatorFeeVault credits) still uses the same formulas and the same Isthmus gate. The `gas_used`
  Firehose passes to the extras is `tx_gas_used()`, which equals the handler's
  `used() - reservoir` after the EIP-7623 floor; reservoir is 0 on every OP fork.
- `exempt deposits from the gas cap in OpHandler::tx_gas` (#22993): moves the existing deposit
  exemption from alloy-op-evm `transact_raw` into the handler. Deposit gas used is unchanged.
- `fix Bedrock deposit decoding` (#22946): serde only (`isSystemTx` as a JSON bool). RLP and
  execution are untouched, so `OpPreTxAdjust` assumptions hold.
- `zero invalid sequencer SDM refunds` (#22963), `enforce canonical PostExec entries` (#23003):
  SDM refund settlement runs inside `execute_transaction_without_commit`, before Firehose reads
  gas, so the vault deltas Firehose emits still match. SDM is gated on Lagoon.
- No new rayon/spawn or system call on the execution path; `evm_env_for_payload` refactor
  (#22968) builds the same `EvmEnv`.
- Network activations: no change. `alloy-op-hardforks`, `op-reth/crates/chainspec` and the
  `superchain-registry` pin (`08d6a449`) are identical at both tags. OP Mainnet and Unichain have
  Karst since 2026-07-08 (OP Sepolia / Unichain Sepolia since 2026-06-17); Lagoon is not scheduled
  anywhere.
- Known gap, not from this range: Lagoon adds the PostExec transaction type `0x7D`, which
  `firehose-tracer` 5.5.0 `TxType` does not know. `reth_firehose::mapper` converts the type with
  `.expect("All transaction types handled for now")`, so the first `0x7D` transaction in a traced
  block would panic the node. Must be mapped before Lagoon is scheduled on a chain we serve.

Everything else is kona-sp1, op-devstack, contracts, Go services, CI and docs.

## Conflicts and how they were resolved

- `rust/Cargo.toml`: upstream rewrote the reth crate block for the new pin and added
  `reth-storage-overlay`. Kept our streamingfast/reth block (plus `reth-engine-tree`,
  `reth-execution-cache`, `reth-firehose`, `reth-trie-parallel`, `firehose-tracer`), pointed it at
  `branch = "bump/op-reth-v2.6.1"`, and added `reth-storage-overlay` from the same branch. The
  `alloy-evm` patch moves to `v0.38.0-sf`.
- `rust/Cargo.lock`: took upstream's and let cargo re-resolve. Against upstream's lock, the only
  differences are the reth source swap, `alloy-evm` from `streamingfast/evm`, `firehose-tracer`
  5.5.0 (pinned with `cargo update --precise`), `reth-firehose` and `reth-optimism-firehose`.

## Firehose code changes

- `op-reth/crates/firehose/src/engine_validator.rs` is a copy of reth's `BasicEngineValidator`.
  Followed the reth v2.5.2 APIs: `OverlayManager` replaces `StateTrieOverlayManager` +
  `ChangesetCache`; `StateProviderBuilder::new(provider, hash, overlay_manager)`; the hashed post
  state comes from `HashedPostState::from_bundle_state::<KeccakKeyHasher>`;
  `validate_block_post_execution_with_hashed_state` takes the parent header; `LazyTrieData::pending`
  and `BuiltPayloadExecutedBlock` drop `changed_paths`; `EngineValidator::cache_for` /
  `payload_state_root_handle_for` become `payload_builder_resources` (shares the execution cache
  only with `--engine.share-execution-cache-with-payload-builder`, as upstream, and pauses JIT
  helpers while the payload builder runs); `ExecutionTimingStats` gains txpool snapshot counters.
- Txpool prewarming (new upstream, off by default, `--engine.txpool-prewarming`) is not wired in:
  its worker handle is private to `reth_engine_tree`. `ExecutionEnv.txpool_snapshot` is `None`.
- No Firehose ExEx, matching vanilla reth and bnb-reth. op-reth never installed the Firehose ExEx
  (live tracing goes through `OpFirehoseEngineValidator`); the reth fork still shipped the unused
  `run_exex` code, now removed by the cherry-pick above. `FIRE INIT` used to be emitted before
  launch and the genesis block from `on_node_started`; both now come from
  `reth_optimism_firehose::init_blockchain`, called from `on_component_initialized` in both launch
  paths of `op-reth/crates/node/src/proof_history.rs`. `FIRE INIT` still reports `"reth"` with this
  crate's version. The only ExEx left is upstream's `proofs-history`, installed only with
  `--proofs-history`.

## Test results

streamingfast/reth `bump/op-reth-v2.6.1`:
- `cargo test --locked -p reth-firehose -p reth-firehose-tests` — 33 + 2 passed, 0 failed (before
  and after the ExEx cherry-pick).
- `cargo test --locked -p reth-engine-tree --test firehose_live_tracing` — 1 passed.

This repo:
- `cargo check --locked -p op-reth -p reth-optimism-firehose -p reth-optimism-node -p reth-optimism-cli -p reth-optimism-payload-builder -p alloy-op-evm -p op-alloy-consensus` — clean.
- `cargo test --locked -p reth-optimism-firehose -p alloy-op-evm -p op-alloy-consensus` — 188
  passed (`reth-optimism-firehose` itself has no tests).
- `cargo test --locked -p reth-optimism-node --lib --test it` — 33 + 15 passed.
- `cargo test --locked -p reth-optimism-cli` — 9 passed.
- Battlefield (`op-reth-devnet`, battlefield-ethereum `master` @ `ffd9260`, release `op-reth`):
  78 passing, 24 pending, 0 failing. The reader emitted the genesis block and stayed at the
  sequencer head for the whole run.
  - The 24 pending are skipped by the suite on this network: Amsterdam EIPs (7708, 7843, 7928,
    8037, 8282), Cancun blobs, BSC Jenner/CAS20, and "Firehose blocks match RPC blocks", which
    skips while the devnet has no finalized block. The 9 more than on v2.5.0 are tests added to
    Battlefield since.
  - Same local edit of `scripts/optimism/run_optimism_devnet.sh` as on v2.5.0: send the funding
    tx to op-rbuilder (port 8547) and wait up to 60s. The first devnet start failed with bproxy
    unable to resolve `op-rbuilder` (container start race); the retry worked.
- The Battlefield OP devnet stops at Isthmus with zero operator fee params, so Jovian and Karst
  (live on OP Mainnet) and the OperatorFeeVault credit path are not covered by this run.

## Before merging

- Tag this repo `op-reth-v2.6.1-fh3.1` after merge.
