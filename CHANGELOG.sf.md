# StreamingFast op-reth Changelog

All notable StreamingFast-specific changes to this fork are documented in this
file. It tracks only the `.sf`-suffixed StreamingFast additions (Firehose
instrumentation, Docker image, release flow) on top of upstream op-reth.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
The `sf-release.yml` workflow publishes the top-most version section here as the
GitHub release notes (via `sfreleaser changelog extract-section`), so keep the
most recent release at the top.

## world-chain-v2.4.3-fh3.2

Re-pins the reth dependency graph onto `streamingfast/reth` `op-reth-v2.4.2-fh3.2`, which stops
a block from advertising a finalized block that is not one of its own ancestors.

### Changed

* Bumped the 74 `streamingfast/reth` pins in `rust/Cargo.toml` from `op-reth-v2.4.2-fh3.1` to
  `op-reth-v2.4.2-fh3.2`. No other dependency moves.

### Fixed

* A block emitted from a side branch no longer carries the canonical chain's finalized head
  (`streamingfast/reth` streamingfast/reth#30). Firehose transmits finality as a bare block
  number, so the consumer resolved it against its own block at that height — a different one —
  and marked it irreversible, then saw the reorg replace it. The advertised block is now clamped
  to the point where the emitted block's branch meets the canonical chain.

## world-chain-v2.4.3-fh3.1

Moves the SF op-reth fork to upstream commit `96ffbb2a`, the untagged `develop` rev that
world-chain `v2.4.3` pins. There is no op-reth release tag on it, hence the world-chain
name on this one: `96ffbb2a` exists to carry the superchain-registry update for the World
Chain Karst activations.

### Changed

- Merged 107 upstream commits (`op-reth/v2.4.2`..`96ffbb2a`). Reth pin is unchanged —
  upstream still pins `op-rs/reth` rev `aef8d3ef`, so `streamingfast/reth` tag
  `op-reth-v2.4.2-fh3.1` still applies.
- `op-reth` now parses its CLI with upstream's `parse_with_denied_args()`. The Firehose
  tracer is still initialized first, before any argument parsing.
- Dropped the `clap` dependency from the `op-reth` binary, matching upstream; nothing in
  the crate used it.

### Notes

- The chainspec build script asserts that the gitignored
  `rust/op-reth/crates/chainspec/res/superchain-configs.tar` matches the committed
  `.sha256`. This merge updates that hash, so an existing checkout needs its
  `superchain-registry` submodule updated and the stale tar removed before it will build.

## v2.4.2-fh3.1

Bumps the SF op-reth fork to upstream `op-reth/v2.4.2`. Upstream `v2.4.1` is an
ancestor of `v2.4.2`, so this one merge brings in its commits too.

### Changed

- Reth pin moved from `streamingfast/reth` tag `v2.3.0-fh-8` to
  **`op-rs-aef8d3e-fh-1`**, the Firehose rebase of the `op-rs/reth` rev
  (`aef8d3ef`) that upstream `op-reth/v2.4.2` pins.
- `[patch.crates-io]` `alloy-evm` moved to `streamingfast/evm` tag `v0.37.0-sf`
  (upstream moved `alloy-evm` 0.36 -> 0.37). The lock pins `alloy-evm` to
  `0.37.0` so the patch actually applies — at `0.37.1` cargo silently drops it
  and the block traces lose all `systemCalls`.
- Firehose engine validator adapted to the new reth payload-validator API: the
  inline multiproof / state-root-task machinery was replaced upstream by a
  `state_root_strategy` framework whose constructors are `pub(crate)`, so the
  validator now computes state roots synchronously. This changes only the
  state-root algorithm; no Firehose event is added, dropped or altered.
- `Dockerfile.sf`: cargo-chef base moved to `rust-1.95` (the workspace
  `rust-version` moved to 1.95), apt fetches retry, and the builder stage now
  receives `GIT_VERSION` / `GIT_COMMIT` / `GIT_DATE` so the new upstream
  `op-version` crate can stamp `op-reth --version`.

## v2.4.0-fh3.1

Bumps the SF op-reth fork to upstream `op-reth/v2.4.0`.

## v2.3.3-fh

Bumps the SF op-reth fork to upstream `op-reth/v2.3.3` (the intervening
`op-reth/v2.3.2` was merged but not separately released). The reth pin is
unchanged (`streamingfast/reth` tag `v2.3.0-fh`), so Firehose tracing behaviour
is identical to `v2.3.1-fh` — validated against the battlefield-ethereum
op-reth-devnet suite.

### Changed

- Updated to upstream `op-reth/v2.3.3`. Enabled the `reth-codec` feature on the
  firehose crate's `reth-optimism-primitives` dependency: upstream made
  `reth-codec` opt-in (#21483), which had dropped the `Compact` impls for
  `OpReceipt`/`OpTxEnvelope`.
- `Dockerfile.sf` / `sf-release.yml`: supply the `superchain-registry` submodule
  to the Docker build as a named build context. Upstream's chainspec `build.rs`
  now generates its chain configs from that submodule (#21397, #21474), which
  lives at the repo root outside the `rust` build context.

## v2.3.1-fh

### Added

- StreamingFast Docker image build, push and release flow (`Dockerfile.sf`,
  `.github/workflows/sf-release.yml`). Pushing a `*-fh*` tag builds the
  Firehose-instrumented `op-reth` binary, publishes a `linux/amd64` image to
  `ghcr.io`, and creates a GitHub release with the binary attached and these
  notes as the body.
