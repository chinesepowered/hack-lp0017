use std::time::Duration;
use thiserror::Error;

/// Exponential back-off retry policy with random jitter.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    /// Jitter factor in [0.0, 1.0]. 0.25 means actual delay is in [0.75x, 1.25x].
    pub jitter: f32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            jitter: 0.25,
        }
    }
}

#[derive(Debug, Error)]
#[error("retry exhausted after {attempts} attempts: {source}")]
pub struct RetryError {
    pub attempts: u32,
    #[source]
    pub source: anyhow::Error,
}

/// Retry an async fallible operation with exponential back-off + jitter.
///
/// `is_transient` returns `true` for errors that should be retried. Returning
/// `false` aborts the loop immediately and surfaces the underlying error.
#[cfg(feature = "tokio")]
pub async fn retry<T, F, Fut, P>(
    cfg: RetryConfig,
    is_transient: P,
    mut op: F,
) -> Result<T, RetryError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
    P: Fn(&anyhow::Error) -> bool,
{
    let mut last: Option<anyhow::Error> = None;
    for attempt in 1..=cfg.max_attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let transient = is_transient(&e);
                if attempt == cfg.max_attempts || !transient {
                    return Err(RetryError {
                        attempts: attempt,
                        source: e,
                    });
                }
                last = Some(e);
                let delay = backoff_delay(cfg, attempt);
                tokio::time::sleep(delay).await;
            }
        }
    }
    Err(RetryError {
        attempts: cfg.max_attempts,
        source: last.unwrap_or_else(|| anyhow::anyhow!("retry exhausted")),
    })
}

fn backoff_delay(cfg: RetryConfig, attempt: u32) -> Duration {
    let base_ms = cfg.base_delay.as_millis() as u64;
    let max_ms = cfg.max_delay.as_millis() as u64;
    let expo = base_ms
        .saturating_mul(2u64.saturating_pow(attempt.saturating_sub(1)))
        .min(max_ms);
    let jitter = cfg.jitter.clamp(0.0, 1.0);
    let rand = pseudo_rand_unit();
    let lo = (expo as f32 * (1.0 - jitter)) as u64;
    let hi = (expo as f32 * (1.0 + jitter)) as u64;
    let span = hi.saturating_sub(lo);
    Duration::from_millis(lo + ((span as f32 * rand) as u64).min(span))
}

/// Cheap pseudo-random unit value derived from monotonic time. Sufficient
/// for back-off jitter; not cryptographic.
fn pseudo_rand_unit() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 10_000) as f32 / 10_000.0
}
