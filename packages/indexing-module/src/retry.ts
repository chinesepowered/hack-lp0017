export interface RetryOptions {
  /** Maximum number of attempts (including the first). Default: 5. */
  maxAttempts?: number;
  /** Base delay in milliseconds for exponential back-off. Default: 200. */
  baseDelayMs?: number;
  /** Maximum delay in milliseconds between attempts. Default: 10_000. */
  maxDelayMs?: number;
  /** Multiplicative random jitter factor in [0, 1]. Default: 0.25. */
  jitter?: number;
  /** Returns true when an error should be retried. Default: always. */
  isTransient?: (err: unknown) => boolean;
  /** Optional sleep injection point, useful for tests. */
  sleep?: (ms: number) => Promise<void>;
  /** Optional logger called on each retry. */
  onRetry?: (info: { attempt: number; delayMs: number; error: unknown }) => void;
}

/**
 * Retries `fn` with exponential back-off and full random jitter.
 *
 * The final failure is wrapped in a `RetryExhaustedError` whose `.cause`
 * references the last underlying error — callers should surface this to the
 * user rather than swallowing it.
 */
export async function retry<T>(fn: () => Promise<T>, opts: RetryOptions = {}): Promise<T> {
  const maxAttempts = opts.maxAttempts ?? 5;
  const base = opts.baseDelayMs ?? 200;
  const cap = opts.maxDelayMs ?? 10_000;
  const jitter = opts.jitter ?? 0.25;
  const isTransient = opts.isTransient ?? (() => true);
  const sleep = opts.sleep ?? defaultSleep;

  let lastErr: unknown;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastErr = err;
      if (attempt === maxAttempts || !isTransient(err)) break;
      const expo = Math.min(cap, base * 2 ** (attempt - 1));
      const j = expo * jitter * Math.random();
      const delayMs = Math.floor(expo - expo * jitter + 2 * j);
      opts.onRetry?.({ attempt, delayMs, error: err });
      await sleep(delayMs);
    }
  }
  throw new RetryExhaustedError(maxAttempts, lastErr);
}

export class RetryExhaustedError extends Error {
  constructor(public readonly attempts: number, public readonly cause: unknown) {
    super(`retry exhausted after ${attempts} attempts: ${describe(cause)}`);
    this.name = "RetryExhaustedError";
  }
}

function describe(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

function defaultSleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
