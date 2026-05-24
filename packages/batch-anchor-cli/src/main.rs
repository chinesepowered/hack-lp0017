//! Permissionless batch anchor CLI.
//!
//! Subscribes to the Logos Delivery topic, accumulates (CID, metadata_hash)
//! tuples, and submits them in batches to the on-chain registry. Anyone
//! can run this tool — the original publisher, an NGO, an automated
//! guardian — there is no coordination requirement.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use whistleblower_core::adapters::in_memory::{InMemoryAnchor, InMemoryDelivery};
use whistleblower_core::adapters::{AnchorAdapter, DeliveryAdapter};
use whistleblower_core::{BatchAnchor, BatchAnchorConfig, DEFAULT_DELIVERY_TOPIC};

mod adapters;
mod state;

#[derive(Parser)]
#[command(
    name = "batch-anchor",
    version,
    about = "Permissionless batch-anchor for Whistleblower documents"
)]
struct Cli {
    /// Path to the on-disk state file used to resume across restarts.
    #[arg(
        long,
        default_value = ".batch-anchor-state.json",
        env = "BATCH_ANCHOR_STATE"
    )]
    state: PathBuf,

    /// Logos Delivery topic to subscribe to.
    #[arg(long, default_value = DEFAULT_DELIVERY_TOPIC, env = "BATCH_ANCHOR_TOPIC")]
    topic: String,

    /// Minimum batch size before opportunistic flush.
    #[arg(long, default_value_t = 10)]
    min_batch: usize,

    /// Maximum batch size per on-chain transaction.
    #[arg(long, default_value_t = 50)]
    max_batch: usize,

    /// Buffer-fill latency in seconds before flushing a partial batch.
    #[arg(long, default_value_t = 15)]
    max_buffer_secs: u64,

    /// Backend selection. `mock` uses in-memory adapters (good for demo /
    /// CI). `lez` talks to a real LEZ sequencer + Logos Delivery.
    #[arg(long, value_enum, default_value_t = Backend::Mock, env = "BATCH_ANCHOR_BACKEND")]
    backend: Backend,

    /// LEZ sequencer RPC URL (used when --backend lez).
    #[arg(long, default_value = "http://127.0.0.1:3040", env = "LEZ_RPC_URL")]
    rpc_url: String,

    /// Path to the wallet directory (used when --backend lez).
    #[arg(long, env = "NSSA_WALLET_HOME_DIR")]
    wallet: Option<PathBuf>,

    /// On-chain registry program id (32-byte hex, no 0x prefix) when --backend lez.
    #[arg(long, env = "WHISTLEBLOWER_PROGRAM_ID")]
    program_id: Option<String>,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the batch anchorer until killed. (Default.)
    Run,
    /// Show the current state file contents.
    Status,
    /// Reset (delete) the state file. Re-anchors all CIDs on next run.
    Reset,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, Serialize, Deserialize)]
enum Backend {
    Mock,
    Lez,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match cli.cmd.as_ref().unwrap_or(&Cmd::Run) {
        Cmd::Status => return cmd_status(&cli).await,
        Cmd::Reset => return cmd_reset(&cli).await,
        Cmd::Run => {}
    }

    let (delivery, anchor): (Arc<dyn DeliveryAdapter>, Arc<dyn AnchorAdapter>) = match cli.backend {
        Backend::Mock => (
            Arc::new(InMemoryDelivery::new()) as Arc<dyn DeliveryAdapter>,
            Arc::new(InMemoryAnchor::new()) as Arc<dyn AnchorAdapter>,
        ),
        Backend::Lez => {
            let program_id_hex = cli
                .program_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--program-id is required with --backend lez"))?;
            let wallet = cli
                .wallet
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--wallet is required with --backend lez"))?;
            (
                adapters::delivery_lez::connect(&cli.rpc_url).await?,
                adapters::anchor_lez::connect(&cli.rpc_url, wallet, program_id_hex).await?,
            )
        }
    };

    let cfg = BatchAnchorConfig {
        topic: cli.topic.clone(),
        min_batch: cli.min_batch,
        max_batch: cli.max_batch,
        max_buffer: Duration::from_secs(cli.max_buffer_secs),
    };

    let prior = state::load(&cli.state).await.unwrap_or_default();
    info!(
        prior_cids = prior.seen_cids.len(),
        topic = %cfg.topic,
        "resuming batch anchor with persisted state"
    );

    let mut batcher = BatchAnchor::new(delivery, anchor, cfg);
    batcher.set_initial_seen(prior.seen_cids.clone()).await;

    let state_path = cli.state.clone();
    let prior_seen = prior.seen_cids.clone();
    batcher.on_batch_anchored(move |ev| {
        // Persist the new watermark synchronously on every batch.
        let mut all = prior_seen.clone();
        all.extend(ev.cids.iter().cloned());
        let st = state::State {
            seen_cids: all,
            last_batch_tx: Some(ev.tx.clone()),
        };
        if let Err(e) = state::save_blocking(&state_path, &st) {
            warn!(error = %e, "failed to persist state");
        }
        info!(
            tx = %ev.tx,
            cids = ev.cids.len(),
            newly_anchored = ev.newly_anchored,
            compute_units = ?ev.compute_units,
            "batch anchored"
        );
    });

    batcher.start().await?;
    info!(state = %cli.state.display(), "batch anchor running, ^C to stop");

    signal::ctrl_c().await?;
    info!("shutdown requested, flushing pending entries");
    batcher.flush().await?;
    batcher.stop().await;
    Ok(())
}

async fn cmd_status(cli: &Cli) -> anyhow::Result<()> {
    let s = state::load(&cli.state).await.unwrap_or_default();
    println!("state file: {}", cli.state.display());
    println!("seen CIDs : {}", s.seen_cids.len());
    println!(
        "last tx   : {}",
        s.last_batch_tx.as_deref().unwrap_or("(none)")
    );
    Ok(())
}

async fn cmd_reset(cli: &Cli) -> anyhow::Result<()> {
    if cli.state.exists() {
        tokio::fs::remove_file(&cli.state).await?;
        println!("removed {}", cli.state.display());
    } else {
        println!("(no state file)");
    }
    Ok(())
}
