use std::path::Path;

use serde::{Deserialize, Serialize};

/// Persistent state. The CLI writes this on every successful batch anchor
/// so a restart resumes from the same watermark.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub seen_cids: Vec<String>,
    pub last_batch_tx: Option<String>,
}

pub async fn load(path: &Path) -> anyhow::Result<State> {
    let bytes = tokio::fs::read(path).await.ok();
    match bytes {
        Some(b) => Ok(serde_json::from_slice(&b)?),
        None => Ok(State::default()),
    }
}

/// Synchronous save — used from the `on_batch_anchored` callback which is
/// itself sync. Atomically writes via tmp-rename so a crash mid-write
/// doesn't leave a half-written state file.
pub fn save_blocking(path: &Path, state: &State) -> anyhow::Result<()> {
    let json = serde_json::to_vec_pretty(state)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
