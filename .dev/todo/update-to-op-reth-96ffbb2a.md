# op-reth bump to upstream `96ffbb2a` (world-chain-v2.4.3-fh3.1)

## Why this rev

World-chain `v2.4.3` moved its optimism-monorepo pin from `tag = "op-reth/v2.4.2"` to
`rev = "96ffbb2a94f19886fe7e27c45f3310e64ccd18b3"`, an untagged commit on upstream
`develop`, 107 commits ahead. Its tip is *"superchain: update registry pin for World Chain
Karst activations (#22624)"*, paired with world-chain's own
`feat(chainspec): set Karst upgrade timestamps (#1070)`.

Following it is not optional. World-chain's
`[patch."https://github.com/ethereum-optimism/optimism"]` is keyed by source URL alone —
cargo never checks that the replacement matches the requested rev — so leaving this fork on
`op-reth-v2.4.2-fh3.1-1` would have built and run cleanly while executing pre-Karst op-reth.

## What did not move

`op-reth` pins `op-rs/reth` rev `aef8d3ef` at both `op-reth/v2.4.2` and `96ffbb2a`, so the
`streamingfast/reth` tag `op-reth-v2.4.2-fh3.1` in `rust/Cargo.toml` stays as is. That tag
string has to remain byte-identical to the one in world-chain's
`[patch."https://github.com/op-rs/reth"]`; if the two drift, cargo resolves two copies of
every reth crate and the executor links against the untraced one.

## Conflicts and how they were resolved

1. `rust/op-reth/bin/Cargo.toml` — upstream removed the `clap` dependency, which sat in the
   same hunk as our `reth-firehose` / `firehose-tracer` entries. Kept the Firehose deps,
   dropped `clap` (only mention left in the crate is a comment in `main.rs`).
2. `rust/op-reth/bin/src/main.rs` — upstream changed `Cli::…::parse().run(…)` to
   `parse_with_denied_args()`, reflowing the closure across the lines where our
   `reth_firehose::init_tracer(…)` call sits. Kept `init_tracer`, took upstream's call form.

Auto-merged, checked by hand: `rust/alloy-op-evm/src/lib.rs` keeps our
`inspect_system_call_with_caller` routing (without it, block-level system calls are missing
from `systemCalls` in the trace) next to upstream's new `POST_EXEC_TX_TYPE_ID` short-circuit
and its move of `mod tests` into `src/tests.rs`.

## Build gotcha

The merge updates `rust/op-reth/crates/chainspec/res/superchain-configs.tar.sha256`, and the
chainspec `build.rs` asserts the gitignored `res/superchain-configs.tar` matches it. On an
existing checkout: update the `superchain-registry` submodule to the recorded gitlink and
delete the stale tar so the build regenerates it.

## Test results

- `cargo check -p op-reth -p reth-optimism-firehose -p alloy-op-evm` — clean.
- `cargo test -p reth-optimism-firehose -p alloy-op-evm` — 76 passed, 0 failed.
- `cargo test -p reth-optimism-node --test it` — 14 passed, 0 failed, including upstream's
  new `debug_trace_post_exec` and `estimate_gas_7825` Karst tests running through the
  Firehose-wrapped executor.
