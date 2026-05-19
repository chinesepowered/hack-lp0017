import type { AnchorAdapter, AnchorEntry, AnchorReceipt, DeliveryAdapter } from "./adapters/types.js";
import { type DocumentEnvelope, canonicalize, validateEnvelope } from "./envelope.js";
import { sha256 } from "./hash.js";

export interface BatchAnchorOptions {
  delivery: DeliveryAdapter;
  anchor: AnchorAdapter;
  topic?: string;
  /** Maximum batch size per on-chain transaction. Default: 50. */
  maxBatch?: number;
  /** Minimum batch size before flushing. Default: 10 (matches success criterion). */
  minBatch?: number;
  /** Maximum buffer-fill latency before flushing a partial batch. Default: 15s. */
  maxBufferMs?: number;
  /**
   * Persistence hook called every time a batch successfully lands on-chain.
   * Implementations should durably record the watermark CID so a restart
   * resumes from there without re-anchoring already-registered entries.
   */
  onBatchAnchored?: (info: BatchAnchorEvent) => void | Promise<void>;
  /**
   * Optional state to resume from — typically loaded from disk and passed
   * back in on startup. `seenCids` is the set of CIDs already in the
   * registry from earlier runs; new envelopes for these are skipped.
   */
  initialState?: { seenCids: Iterable<string> };
}

export interface BatchAnchorEvent {
  tx: string;
  cids: string[];
  newlyAnchored: number;
  computeUnits?: number;
}

export interface BatchAnchorStatus {
  pendingCids: number;
  totalAnchored: number;
  lastBatchTx?: string;
}

/**
 * Permissionless, restartable batch anchor.
 *
 * Subscribes to the Logos Delivery topic, accumulates (CID, metadata_hash)
 * tuples, and submits them on-chain in batches of up to `maxBatch`. Already
 * anchored CIDs are filtered client-side (via `isAnchored`) AND the on-chain
 * registry is itself idempotent, so re-submission is always safe.
 */
export class BatchAnchor {
  private readonly topic: string;
  private readonly maxBatch: number;
  private readonly minBatch: number;
  private readonly maxBufferMs: number;

  private pending: Map<string, AnchorEntry> = new Map();
  private seen: Set<string>;
  private unsubscribe?: () => Promise<void>;
  private flushTimer?: ReturnType<typeof setTimeout>;
  private totalAnchored = 0;
  private lastBatchTx?: string;
  private running = false;

  constructor(private readonly opts: BatchAnchorOptions) {
    this.topic = opts.topic ?? "/logos/whistleblower/v1/documents";
    this.maxBatch = opts.maxBatch ?? 50;
    this.minBatch = opts.minBatch ?? 10;
    this.maxBufferMs = opts.maxBufferMs ?? 15_000;
    this.seen = new Set(opts.initialState?.seenCids ?? []);
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    this.unsubscribe = await this.opts.delivery.subscribe(this.topic, (env) => this.onEnvelope(env));
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.unsubscribe) {
      await this.unsubscribe();
      this.unsubscribe = undefined;
    }
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = undefined;
    }
  }

  /** Force-flush any pending entries. Returns the receipt or null if nothing pending. */
  async flush(): Promise<AnchorReceipt | null> {
    if (this.pending.size === 0) return null;
    return this.flushNow();
  }

  status(): BatchAnchorStatus {
    return {
      pendingCids: this.pending.size,
      totalAnchored: this.totalAnchored,
      lastBatchTx: this.lastBatchTx,
    };
  }

  /** Exposed for the CLI to snapshot resume state to disk. */
  snapshot(): { seenCids: string[] } {
    return { seenCids: [...this.seen] };
  }

  private async onEnvelope(env: DocumentEnvelope): Promise<void> {
    try {
      validateEnvelope(env);
    } catch {
      return; // ignore malformed envelopes — the topic is permissionless
    }

    if (this.seen.has(env.cid) || this.pending.has(env.cid)) return;

    // Client-side pre-check. The registry is also idempotent server-side,
    // but skipping here avoids burning CU on duplicates.
    if (await this.opts.anchor.isAnchored(env.cid)) {
      this.seen.add(env.cid);
      return;
    }

    const metadataHash = await sha256(canonicalize(env));
    this.pending.set(env.cid, { cid: env.cid, metadataHash });

    if (this.pending.size >= this.maxBatch) {
      await this.flushNow();
    } else if (this.pending.size >= this.minBatch && !this.flushTimer) {
      this.flushTimer = setTimeout(() => {
        this.flushNow().catch(() => {
          // Errors surface via onBatchAnchored not firing; the CLI re-attempts on next envelope.
        });
      }, this.maxBufferMs);
    }
  }

  private async flushNow(): Promise<AnchorReceipt> {
    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = undefined;
    }
    const entries = [...this.pending.values()];
    this.pending.clear();

    const receipt = await this.opts.anchor.anchorBatch(entries);
    this.lastBatchTx = receipt.tx;
    this.totalAnchored += receipt.newlyAnchored;
    for (const e of entries) this.seen.add(e.cid);

    await this.opts.onBatchAnchored?.({
      tx: receipt.tx,
      cids: entries.map((e) => e.cid),
      newlyAnchored: receipt.newlyAnchored,
      computeUnits: receipt.computeUnits,
    });

    return receipt;
  }
}
