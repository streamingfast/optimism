# StreamingFast op-reth Changelog

All notable StreamingFast-specific changes to this fork are documented in this
file. It tracks only the `.sf`-suffixed StreamingFast additions (Firehose
instrumentation, Docker image, release flow) on top of upstream op-reth.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
The `sf-release.yml` workflow publishes the top-most version section here as the
GitHub release notes (via `sfreleaser changelog extract-section`), so keep the
most recent release at the top.

## Unreleased

### Added

- StreamingFast Docker image build, push and release flow (`Dockerfile.sf`,
  `.github/workflows/sf-release.yml`). Pushing a `*-fh*` tag builds the
  Firehose-instrumented `op-reth` binary, publishes a `linux/amd64` image to
  `ghcr.io`, and creates a GitHub release with the binary attached and these
  notes as the body.
