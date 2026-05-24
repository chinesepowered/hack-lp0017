import type {
  AnchorAdapter,
  AnchorEntry,
  AnchorReceipt,
  DeliveryAdapter,
  StorageAdapter,
} from "./types.js";
import type { DocumentEnvelope } from "../envelope.js";
import { sha256, toHex } from "../hash.js";

/**
 * In-memory adapters that mirror the contract of the real Logos clients.
 * Used by the integration tests, the demo script, and any consumer that
 * wants to develop against the indexing module without a running stack.
 */

export class InMemoryStorage implements StorageAdapter {
  readonly objects = new Map<string, Uint8Array>();

  async put(bytes: Uint8Array): Promise<{ cid: string; sizeBytes: number }> {
    const hash = await sha256(bytes);
    const cid = "bafy" + toHex(hash).slice(0, 56);
    if (!this.objects.has(cid)) {
      this.objects.set(cid, bytes);
    }
    return { cid, sizeBytes: bytes.byteLength };
  }

  async get(cid: string): Promise<Uint8Array> {
    const v = this.objects.get(cid);
    if (!v) throw new Error(`object not found: ${cid}`);
    return v;
  }
}

export class InMemoryDelivery implements DeliveryAdapter {
  private topics = new Map<string, Set<(env: DocumentEnvelope) => void | Promise<void>>>();
  /** Tracks already-published (topic, cid) tuples for dedup. */
  readonly published = new Map<string, Set<string>>();

  async publish(topic: string, envelope: DocumentEnvelope): Promise<void> {
    const seen = this.published.get(topic) ?? new Set<string>();
    if (seen.has(envelope.cid)) return; // dedup
    seen.add(envelope.cid);
    this.published.set(topic, seen);

    const handlers = this.topics.get(topic);
    if (!handlers) return;
    for (const h of handlers) await h(envelope);
  }

  async subscribe(
    topic: string,
    handler: (env: DocumentEnvelope) => void | Promise<void>,
  ): Promise<() => Promise<void>> {
    let set = this.topics.get(topic);
    if (!set) {
      set = new Set();
      this.topics.set(topic, set);
    }
    set.add(handler);
    return async () => {
      this.topics.get(topic)?.delete(handler);
    };
  }
}

export class InMemoryAnchor implements AnchorAdapter {
  /** cid → { metadataHash, anchorTimestamp } */
  readonly registry = new Map<string, { metadataHash: Uint8Array; anchorTimestamp: number }>();
  private nextTx = 1;

  async anchorSingle(entry: AnchorEntry): Promise<AnchorReceipt> {
    return this.anchorBatch([entry]);
  }

  async anchorBatch(entries: AnchorEntry[]): Promise<AnchorReceipt> {
    let newlyAnchored = 0;
    const now = Date.now();
    for (const e of entries) {
      if (!this.registry.has(e.cid)) {
        this.registry.set(e.cid, { metadataHash: e.metadataHash, anchorTimestamp: now });
        newlyAnchored++;
      }
    }
    return {
      tx: `tx-${this.nextTx++}`,
      newlyAnchored,
      // Synthetic CU cost roughly modelled on the real registry:
      // ~5_000 fixed + ~2_000 per entry. The real numbers are recorded
      // by the benchmark script against a live LEZ devnet.
      computeUnits: 5_000 + entries.length * 2_000,
    };
  }

  async isAnchored(cid: string): Promise<boolean> {
    return this.registry.has(cid);
  }
}
