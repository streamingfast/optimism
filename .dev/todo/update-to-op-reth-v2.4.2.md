# Update op-reth to v2.4.2

**Chain:** Optimism (op-reth, Rust execution client)
**Repository:** streamingfast/optimism (fork of ethereum-optimism/optimism)
**Release branch:** `release/2.x`
**Bump branch:** `bump/op-reth-v2.4.2`
**Last merged upstream:** `op-reth/v2.4.0` (fork tags `v2.4.0-fh3.1`, `-fh3.1-1`)
**Target upstream:** `op-reth/v2.4.2` — 389 commits. `v2.4.1` is an ancestor of `v2.4.2`,
so merging `v2.4.2` brings in its commits too; there is no separate `v2.4.1` merge.
**Rust workspace:** `rust/` (op-reth crates under `rust/op-reth/crates/`)

## Reth bump — REQUIRED (already available)

Upstream moved the reth pin twice in this range:

| tag | reth pin | crates.io reth-* |
|-----|----------|------------------|
| v2.4.0 | `paradigmxyz/reth` tag `v2.3.0` | 0.4.1 |
| v2.4.1 | `paradigmxyz/reth` rev `f2eecc65` | 0.5.0 |
| v2.4.2 | `op-rs/reth` rev `aef8d3ef92117f91455e16969f0adf5bf7c6e9e1` | 0.5.0 |

`streamingfast/reth` already carries the Firehose rebase of that exact rev on
branch `firehose/op-reth-2.4.x-fh`, tagged **`op-rs-aef8d3e-fh-1`** (includes the
genesis-block-on-empty-chain fix). All 75 workspace pins now point at it.

`alloy-evm` also moved 0.36.0 -> 0.37.x, so the `[patch.crates-io]` SF fork pin
moved to `streamingfast/evm` tag `v0.37.0-sf`. Upstream's lock resolved
`alloy-evm 0.37.1`, which silently makes the 0.37.0 patch *unused* (cargo warns
`patch ... was not used in the crate graph`) and drops all `systemCalls` from the
Firehose block traces. Fixed with
`cargo update -p alloy-evm@0.37.1 --precise 0.37.0`. **Check this warning on every
future bump.**

## Merge scope

Conflicts: `rust/Cargo.toml`, `rust/Cargo.lock` only (plus two rerere-resolved
files, `rust/op-reth/bin/Cargo.toml` and `bin/src/main.rs`). Everything else
auto-merged.

`rust/op-reth/crates/firehose/` (SF-only crate) does not conflict but stopped
compiling against the new reth: 29 errors.

## Plan

1. [x] `git merge op-reth/v2.4.2` into `bump/op-reth-v2.4.2` — commit `b4a5320942`.
2. [x] `rust/Cargo.toml` — keep our 75 SF pins, retag to `op-rs-aef8d3e-fh-1`;
       bump the `alloy-evm` patch to `v0.37.0-sf`.
3. [x] `rust/Cargo.lock` — take upstream's, then pin `alloy-evm` to 0.37.0 so the
       SF patch applies.
4. [x] Firehose adaptation (`engine_validator.rs`, `evm_config.rs`) — commit
       `48a5f6be2c`. Ported from `release/world-chain-2.4.x` (commit `4628c8a8c5`),
       **minus** the finalized-head-as-LIB change, which is still open in PR #9
       against `release/2.x`.
5. [x] `Dockerfile.sf` — cargo-chef `rust-1.95` (workspace `rust-version` moved to
       1.95), apt retries, and `GIT_VERSION`/`GIT_COMMIT`/`GIT_DATE` build args for
       the new `op-version` crate; `sf-release.yml` supplies them.
6. [x] `cargo +nightly-2026-02-20 fmt --all` (also picked up pre-existing
       fmt drift in `firehose/src/{extras,lib}.rs`).
7. [x] `cargo check --all-targets` on `reth-optimism-firehose`, `reth-optimism-node`,
       `reth-optimism-payload-builder`, `reth-optimism-cli`, `op-reth` — clean.
8. [x] `cargo test -p reth-optimism-firehose -p reth-optimism-payload-builder
       -p reth-optimism-txpool` — 87 passed, 0 failed.
9. [x] `cargo clippy --all-targets` on the same crates — no errors. Dropped the two
       dependencies the adaptation made unused; the remaining warnings (doc backticks
       in `extras.rs` / `lib.rs`) pre-date this merge.
10. [x] Battlefield `op-reth-devnet` — **80 passing, 0 failing, 5 pending**, and
       `compare-blocks-rpc` blocks 0-250 all identical.
11. [x] `CHANGELOG.sf.md` — release named `v2.4.2-fh3.1`.

## Battlefield gotchas (cost ~15 min this run)

- `scripts/compare-blocks.sh` hardcodes `http://localhost:8545`, which in the **optimism**
  devnet is the **L1** RPC — it diffs L2 Firehose blocks against L1, prints garbage, then
  panics inside `fireeth`'s own `blockfetcher.fetchBlockReceipts`. Run the underlying
  command against the L2 op-reth RPC instead:
  `fireeth tools compare-blocks-rpc --plaintext localhost:8089 http://localhost:28545 0 <head-200>`
- The firehose gRPC endpoint on `:8089` does not open until the info server can read
  `first_streamable_block`, which needs the **first merged 100-block bundle**. The merger
  only bundles once LIB passes 100, and LIB tracks L1 finality — so expect ~10 minutes of
  `ConnectError: [unavailable]` after launch before the suite can run. Not a regression.

## Upstream API changes that hit the Firehose crate

- Payload validator's inline multiproof / state-root-task machinery replaced by a
  pluggable `state_root_strategy` framework whose context constructors are
  `pub(crate)` — unusable from an external crate. The Firehose validator falls
  back to synchronous state-root computation. `ParallelStateRoot` was removed
  upstream (folded into the now-private sparse-trie job). **State-root strategy
  only — no tracer event is emitted or altered by this.**
- `ConfigurePostExecEvm` gained an associated `Snapshot` type; `OpFirehoseEvmConfig`
  delegates it to the inner config.
- Engine validator builder gained a `state_trie_overlays` parameter.
- `PayloadProcessor::{executor,spawn_state_root,wait_for_caches}` and
  `ChangesetCache::register_pending` are gone / private.
- State hooks moved from the executor to the revm `State` (alloy-evm #366): set
  via `executor.evm_mut().db_mut().set_state_hook(..)`.

## Notes / learnings

- `release/world-chain-2.4.x` had already merged `op-reth/v2.4.2`; its Firehose
  adaptation is the reference resolution. The two branches' Firehose crates are
  otherwise identical apart from PR #9 (LIB).
- Build prerequisite: `git submodule update --init --force superchain-registry`,
  otherwise `reth-optimism-chainspec`'s `build.rs` panics.
