# Update op-reth to v2.4.0

**Chain:** Optimism (op-reth, Rust execution client)
**Repository:** streamingfast/optimism (fork of ethereum-optimism/optimism)
**Firehose branch:** `firehose/2.x`
**Feature branch:** `feature/update-to-op-reth-version-2-4-0`
**Last merged upstream:** `op-reth/v2.3.3` (fork tags `v2.3.3-fh`, `-fh-1`, `-fh-2`)
**Target upstream:** `op-reth/v2.4.0`
**Rust workspace:** `rust/` (op-reth crates under `rust/op-reth/crates/`)

## Reth bump validation — NOT NEEDED

Compared upstream `rust/Cargo.toml` reth/revm/alloy dependency pins between
`op-reth/v2.3.3` and `op-reth/v2.4.0`:

- `[workspace.dependencies]` block: **no diff at all** between the two tags.
- Core reth crates pinned to `paradigmxyz/reth` **tag `v2.3.0`** in both.
- Published reth crates unchanged: `reth-codecs`, `reth-primitives-traits`,
  `reth-rpc-traits`, `reth-zstd-compressors` all `0.4.1`.
- Only new dep in v2.4.0 is `alloy-signer-local` in `node/Cargo.toml`
  (test/dev dep, already defined at workspace level) — not a reth crate.

Our fork maps `paradigmxyz/reth v2.3.0` → `streamingfast/reth.git` tag
**`v2.3.0-fh-2`** (74 crates). Since upstream reth pin did not move,
`v2.3.0-fh-2` already covers v2.4.0. **No new streamingfast/reth tag required.**

## Merge scope (op-reth changes v2.3.3 → v2.4.0)

39 upstream commits. Firehose-relevant op-reth files changed upstream:

- `rust/op-reth/crates/node/src/node.rs` (~155 lines) — node wiring; Firehose
  tracer init lives near here → **likely conflict**
- `rust/op-reth/crates/payload/src/builder.rs` (~65) + `builder/tests.rs` —
  Firehose traces built blocks (fork commit "trace-built-blocks") → **likely conflict**
- `rust/op-reth/crates/payload/src/lib.rs`
- `rust/op-reth/crates/txpool/src/lib.rs` (~30)
- `rust/op-reth/crates/node/tests/it/custom_pool/*` — new upstream tests
- `rust/Cargo.toml` — will conflict (our streamingfast/reth pins vs upstream
  paradigmxyz pins); resolve by KEEPING our streamingfast pins.

Firehose crate `rust/op-reth/crates/firehose/` NOT touched upstream (our addition).

## Plan

1. [x] `git merge op-reth/v2.4.0` into feature branch — **conflict-free** (merge commit `2c0eff61d0`).
2. [x] Cargo.toml — no conflict; streamingfast/reth pins retained (74 @ `v2.3.0-fh-2`).
3. [x] node.rs / payload builder — auto-merged; Firehose hooks intact in disjoint functions.
4. [x] `cargo check` firehose+node+payload+txpool — PASSED (exit 0).
5. [x] `cargo test` firehose+payload+txpool — **56 passed, 0 failed**.
6. [x] Update `rust/op-reth/CHANGELOG.sf.md`.
7. [ ] Battlefield reth tests (reth-dev + reth-devnet) — NOT RUN (offer to user).
8. [ ] Commit (await user request — git policy).

## Notes / learnings

- Reth bump NOT needed — upstream reth pin unchanged v2.3.3→v2.4.0 (see above).
- Build prerequisite: `superchain-registry` submodule must be checked out or
  `reth-optimism-chainspec` build.rs panics. `just update-superchain-registry-submodule`
  fails (broken `justfiles/` import in worktree); use
  `git submodule update --init --force superchain-registry` directly.
- Merge was clean because upstream v2.4.0 op-reth changes (execute_best_transactions
  commit-hook trait, custom_pool tests) live in different functions than the Firehose
  hooks (`build_payload` no_tx_pool branch, node builder-type wiring).
