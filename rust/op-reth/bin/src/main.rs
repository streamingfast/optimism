#![allow(missing_docs, rustdoc::missing_crate_level_docs)]

use clap::Parser;
use reth_optimism_cli::{Cli, chainspec::OpChainSpecParser};
use reth_optimism_node::{args::RollupArgs, proof_history};
use tracing::info;

#[global_allocator]
static ALLOC: reth_cli_util::allocator::Allocator = reth_cli_util::allocator::new_allocator();

#[cfg(all(feature = "jemalloc-prof", unix))]
#[unsafe(export_name = "_rjem_malloc_conf")]
static MALLOC_CONF: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

fn main() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        unsafe {
            std::env::set_var("RUST_BACKTRACE", "1");
        }
    }

    // Initialize the process-wide Firehose tracer. This is firehose-instrumented op-reth, so the
    // tracer is always on (no CLI flag) — mirrors the SF reth fork's `bin/reth/src/main.rs`. Until
    // this runs, `reth_firehose::is_tracer_initialized()` returns false and the engine-API live
    // path in `OpFirehoseEngineValidator` skips tracing, emitting no Firehose output. `op-reth` is
    // reth-based, so `ChainClient::Reth` is the correct client identity.
    reth_firehose::init_tracer(firehose_tracer::config::Config {
        chain_client: firehose_tracer::config::ChainClient::Reth,
        ..Default::default()
    });

    if let Err(err) =
        Cli::<OpChainSpecParser, RollupArgs>::parse().run(async move |builder, args| {
            info!(target: "reth::cli", "Launching node");
            proof_history::launch_node(builder, args).await
        })
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
