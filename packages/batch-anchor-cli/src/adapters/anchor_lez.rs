use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::process::Command;

use whistleblower_core::adapters::{AnchorAdapter, AnchorEntry, AnchorReceipt};

/// On-chain anchor adapter for the LEZ registry program.
///
/// Calls `spel` as a subprocess against the live sequencer. This is the
/// same pattern logos-co/whisper-wall uses for its CLI — `spel` reads
/// `spel.toml` to locate the IDL + binary and exposes each program
/// instruction as a CLI subcommand.
pub async fn connect(
    rpc_url: &str,
    wallet: &Path,
    program_id_hex: &str,
) -> anyhow::Result<Arc<dyn AnchorAdapter>> {
    if which::which("spel").is_err() && which::which("logos-scaffold").is_err() {
        anyhow::bail!(
            "`spel` not on PATH and `logos-scaffold` not installed. \
             Install via:\n  \
             cargo install --git https://github.com/logos-co/spel spel\n  \
             cargo install --git https://github.com/logos-co/logos-scaffold"
        )
    }
    Ok(Arc::new(LezAnchor {
        rpc_url: rpc_url.to_string(),
        wallet: wallet.to_path_buf(),
        program_id_hex: program_id_hex.to_string(),
    }))
}

struct LezAnchor {
    rpc_url: String,
    wallet: PathBuf,
    program_id_hex: String,
}

impl LezAnchor {
    async fn run_spel(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = Command::new("spel")
            .env("NSSA_WALLET_HOME_DIR", &self.wallet)
            .env("LEZ_RPC_URL", &self.rpc_url)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "spel {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn entries_to_args(entries: &[AnchorEntry]) -> Vec<String> {
        // Encoded as `cid|hex_hash` pairs joined by `,`. The registry's
        // `anchor_batch` instruction parses this format into a Vec<(String, [u8;32])>.
        let payload = entries
            .iter()
            .map(|e| format!("{}|{}", e.cid, hex::encode(e.metadata_hash)))
            .collect::<Vec<_>>()
            .join(",");
        vec!["--entries".to_string(), payload]
    }
}

#[async_trait]
impl AnchorAdapter for LezAnchor {
    async fn anchor_single(&self, entry: AnchorEntry) -> anyhow::Result<AnchorReceipt> {
        self.anchor_batch(vec![entry]).await
    }

    async fn anchor_batch(&self, entries: Vec<AnchorEntry>) -> anyhow::Result<AnchorReceipt> {
        let entries_arg = Self::entries_to_args(&entries);
        let mut args = vec![
            "--dry-run=json",
            "--program-id",
            self.program_id_hex.as_str(),
            "--",
            "anchor_batch",
        ];
        for a in &entries_arg {
            args.push(a);
        }
        let stdout = self.run_spel(&args).await?;
        // `spel --dry-run=json` returns a JSON object with `signature` and
        // a `cu_used` field on the receipt. Parse defensively.
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
        let tx = v
            .get("signature")
            .and_then(|s| s.as_str())
            .unwrap_or("dry-run")
            .to_string();
        let cu = v.get("cu_used").and_then(|n| n.as_u64());
        let newly = v
            .get("newly_anchored")
            .and_then(|n| n.as_u64())
            .unwrap_or(entries.len() as u64) as u32;
        Ok(AnchorReceipt {
            tx,
            newly_anchored: newly,
            compute_units: cu,
        })
    }

    async fn is_anchored(&self, cid: &str) -> anyhow::Result<bool> {
        let stdout = self
            .run_spel(&[
                "--dry-run=json",
                "--program-id",
                self.program_id_hex.as_str(),
                "--",
                "lookup",
                "--cid",
                cid,
            ])
            .await?;
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
        Ok(v.get("found").and_then(|b| b.as_bool()).unwrap_or(false))
    }
}

// `which` brought in as an inline mini-impl to avoid pulling in another dep.
mod which {
    use std::path::PathBuf;
    pub fn which(name: &str) -> Result<PathBuf, ()> {
        let Some(path) = std::env::var_os("PATH") else {
            return Err(());
        };
        for dir in std::env::split_paths(&path) {
            let p = dir.join(name);
            if p.is_file() {
                return Ok(p);
            }
            let p_exe = dir.join(format!("{name}.exe"));
            if p_exe.is_file() {
                return Ok(p_exe);
            }
        }
        Err(())
    }
}
