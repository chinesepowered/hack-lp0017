import type { DocumentEnvelope } from "../envelope.js";

/**
 * Adapter for Logos Storage. Concrete implementations wrap the real client
 * (`@logos/storage` once shipped) or a local mock used in tests / CI.
 */
export interface StorageAdapter {
  /**
   * Uploads bytes durably and returns the resulting content identifier.
   * Implementations SHOULD treat already-present content as a no-op and
   * return the same CID — uploads are content-addressed.
   */
  put(bytes: Uint8Array, hint?: { contentType?: string }): Promise<{ cid: string; sizeBytes: number }>;
  /** Optional fetch path used by demos and tests. */
  get?(cid: string): Promise<Uint8Array>;
}

/**
 * Adapter for Logos Delivery — the gossip/pubsub fabric used to broadcast
 * the envelope so subscribers (including the batch anchor tool) discover
 * new documents in real time.
 */
export interface DeliveryAdapter {
  /** Publishes an envelope to `topic`. MUST be idempotent on (topic, cid). */
  publish(topic: string, envelope: DocumentEnvelope): Promise<void>;
  /** Subscribes to a topic. Returns an `unsubscribe` thunk. */
  subscribe(
    topic: string,
    handler: (envelope: DocumentEnvelope) => void | Promise<void>,
  ): Promise<() => Promise<void>>;
}

/**
 * Adapter for the on-chain registry. Either:
 *   (a) a LEZ program client (default), or
 *   (b) a direct zone-SDK submission client.
 *
 * `anchorBatch` MUST be idempotent — re-submitting an already-anchored CID
 * is required to succeed silently (the registry program enforces this
 * server-side, but adapters should not retry on the resulting no-op).
 */
export interface AnchorAdapter {
  anchorSingle(entry: AnchorEntry): Promise<AnchorReceipt>;
  anchorBatch(entries: AnchorEntry[]): Promise<AnchorReceipt>;
  /** Queries the registry to confirm a CID is anchored. */
  isAnchored(cid: string): Promise<boolean>;
}

export interface AnchorEntry {
  cid: string;
  /** 32-byte SHA-256 of the canonical envelope. */
  metadataHash: Uint8Array;
}

export interface AnchorReceipt {
  /** Transaction signature / hash. */
  tx: string;
  /** Number of entries newly registered (excludes idempotent duplicates). */
  newlyAnchored: number;
  /** Reported compute units, if the underlying chain exposes them. */
  computeUnits?: number;
}
