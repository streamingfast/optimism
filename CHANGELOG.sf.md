# StreamingFast op-reth Changelog

All notable StreamingFast-specific changes to this fork are documented in this
file. It tracks only the `.sf`-suffixed StreamingFast additions (Firehose
instrumentation, Docker image, release flow) on top of upstream op-reth.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
The `sf-release.yml` workflow publishes the top-most version section here as the
GitHub release notes (via `sfreleaser changelog extract-section`), so keep the
most recent release at the top.

## v2.4.2-fh3.1

Bumps the SF op-reth fork to upstream `op-reth/v2.4.2` (upstream `v2.4.1` is
subsumed).

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
