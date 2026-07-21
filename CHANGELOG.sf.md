# StreamingFast op-reth Changelog

All notable StreamingFast-specific changes to this fork are documented in this
file. It tracks only the `.sf`-suffixed StreamingFast additions (Firehose
instrumentation, Docker image, release flow) on top of upstream op-reth.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
The `sf-release.yml` workflow publishes the top-most version section here as the
GitHub release notes (via `sfreleaser changelog extract-section`), so keep the
most recent release at the top.

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
