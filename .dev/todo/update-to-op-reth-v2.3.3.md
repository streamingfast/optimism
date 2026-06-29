# Update SF op-reth fork → upstream op-reth/v2.3.3

- **Repo**: streamingfast/optimism (firehose fork), branch `firehose/2.x`
- **Chain**: optimism (op-reth, L2). Rust workspace in `rust/`.
- **Last merged upstream**: `op-reth/v2.3.2` (PR #3, commit ed917f785c)
- **Target**: `op-reth/v2.3.3` (sha 20636578)
- **Merge branch**: `firehose/v2.3.3`

## Context / scope analysis

- 72 upstream commits v2.3.2..v2.3.3. Most are non-firehose (docs, op-node,
  contracts, kona, op-rbuilder, ci). Relevant surface = `rust/op-reth/`.
- **reth pin NOT bumped**: upstream did not change reth/revm/alloy versions.
  Fork stays on `streamingfast/reth` tag `v2.3.0-fh`. No `-fh` re-pin needed
  (unlike v2.3.1→v2.3.2 which bumped reth to v2.3.0).
- New upstream workspace deps (kona-sp1 related, not reth/firehose):
  sp1-sdk 6.2.4, kzg-rs 0.2.6, opentelemetry 0.31, brotli-decompressor 5.0.1.

## Files changed by BOTH fork and upstream (conflict candidates)

- [ ] rust/Cargo.toml
- [ ] rust/Cargo.lock
- [ ] rust/alloy-op-evm/src/lib.rs
- [ ] rust/op-reth/crates/node/Cargo.toml
- [ ] rust/op-reth/crates/node/src/node.rs
- [ ] rust/op-reth/crates/node/tests/it/builder.rs

## Firehose surface to preserve (fork-only files, no upstream change)

- rust/op-reth/crates/firehose/**  (engine_validator, evm_config, extras, lib)
- rust/op-alloy/crates/consensus/src/firehose.rs (+ lib.rs, Cargo.toml)
- rust/op-reth/crates/node/src/proof_history.rs
- rust/op-reth/bin/src/main.rs, bin/Cargo.toml
- rust/op-reth/crates/cli/src/app.rs

## Steps

1. [ ] `git merge op-reth/v2.3.3` on branch `firehose/v2.3.3`
2. [ ] Resolve conflicts (preserve firehose tracing)
3. [ ] `cd rust && cargo check` (reth-optimism-firehose + workspace)
4. [ ] `cargo test -p reth-firehose -p reth-firehose-tests` (MANDATORY)
5. [ ] op-reth binary build
6. [ ] Battlefield op-reth-devnet suite
7. [ ] Update CHANGELOG.sf.md, commit, PR

## Resolutions log

- **rust/Cargo.lock**: only conflict. Took `--ours` (fork firehose git pins) then
  `cargo metadata` regenerated against merged Cargo.toml (pulled new sp1/slop/
  opentelemetry/brotli deps). No reth/revm version change.
- **rust/Cargo.toml**: auto-merged clean. 74 `v2.3.0-fh` reth pins intact, new
  upstream deps present. No re-pin needed.
- **node/src/node.rs**: auto-merged clean. Firehose wrappers preserved
  (`OpFirehoseEngineValidatorBuilder`, `OpFirehoseEvmConfig`), upstream
  interop_failsafe + import reformat kept.
- **node/tests/it/builder.rs**: auto-merged clean. `.inner` unwrap of
  `OpFirehoseEvmConfig` preserved.
- **alloy-op-evm/src/lib.rs**: auto-merged clean. `InspectSystemCallEvm`
  inspect-mode system-call branch (firehose tracing fix) preserved.
- **node/Cargo.toml**: auto-merged clean (only firehose dep line added).
- **FIX — firehose/Cargo.toml**: upstream #21483 made `reth-codec` opt-in on
  `reth-optimism-primitives`. Firehose crate pulled it plain → `OpReceipt`/
  `OpTxEnvelope: Compact` unsatisfied. Added `features = ["reth-codec"]`,
  matching payload/rpc/storage/post-exec-replay. `cargo check -p
  reth-optimism-firehose` ✅.
- **engine_validator.rs**: NO change needed. Mirrors reth fork
  `firehose/2.x` payload_validator; reth pin unchanged (`v2.3.0-fh`).
  Upstream op-reth engine.rs delta was test-only (BASE_SEPOLIA→OP_SEPOLIA).
