import type { StorageAdapter, DeliveryAdapter, AnchorAdapter, AnchorEntry } from "./adapters/types.js";
import { type DocumentEnvelope, ENVELOPE_SCHEMA, canonicalize, validateEnvelope } from "./envelope.js";
import { sha256 } from "./hash.js";
import { retry, type RetryOptions } from "./retry.js";

export interface PublisherOptions {
  storage: StorageAdapter;
  delivery: DeliveryAdapter;
  /** Optional — only required if `anchor()` is invoked on the publisher side. */
  anchor?: AnchorAdapter;
  /** Logos Delivery topic. Defaults to the canonical Whistleblower topic. */
  topic?: string;
  /** Retry policy for storage uploads. */
  retry?: RetryOptions;
  /** Clock injection point for deterministic tests. */
  now?: () => number;
}

export interface PublishResult {
  envelope: DocumentEnvelope;
  /** SHA-256 over the canonical envelope bytes; the value committed on-chain. */
  metadataHash: Uint8Array;
}

/**
 * Publisher: the "uploader-side" half of the pipeline.
 *
 * Workflow:
 *   1. `publish(bytes, meta)` — upload to Logos Storage with retry,
 *      build the canonical envelope, broadcast on Logos Delivery.
 *   2. `anchor(result)` — OPTIONAL, only if the publisher wants to anchor
 *      their own document immediately. Equivalent functionality is also
 *      offered to any third party via the `BatchAnchor` class.
 *
 * The two halves are decoupled by design: a publisher with no on-chain
 * funds can still upload+broadcast and rely on an external batch anchorer.
 */
export class Publisher {
  private readonly topic: string;
  private readonly now: () => number;

  constructor(private readonly opts: PublisherOptions) {
    this.topic = opts.topic ?? "/logos/whistleblower/v1/documents";
    this.now = opts.now ?? (() => Date.now());
  }

  async publish(
    bytes: Uint8Array,
    meta: {
      title: string;
      description: string;
      content_type: string;
      tags?: string[];
    },
  ): Promise<PublishResult> {
    const uploaded = await retry(
      () => this.opts.storage.put(bytes, { contentType: meta.content_type }),
      this.opts.retry,
    );

    const envelope: DocumentEnvelope = {
      schema: ENVELOPE_SCHEMA,
      cid: uploaded.cid,
      title: meta.title,
      description: meta.description,
      content_type: meta.content_type,
      size_bytes: uploaded.sizeBytes,
      timestamp: this.now(),
      ...(meta.tags && meta.tags.length > 0 ? { tags: meta.tags } : {}),
    };
    validateEnvelope(envelope);

    const metadataHash = await sha256(canonicalize(envelope));

    // Delivery publish: idempotent on (topic, cid) at the adapter layer.
    await this.opts.delivery.publish(this.topic, envelope);

    return { envelope, metadataHash };
  }

  /**
   * Publisher-side anchor — distinct from the basic upload+broadcast path.
   * Idempotent: if the registry already has this CID it is a no-op.
   */
  async anchor(result: PublishResult): Promise<{ tx: string; alreadyAnchored: boolean }> {
    if (!this.opts.anchor) {
      throw new Error("anchor adapter not configured");
    }
    const entry: AnchorEntry = { cid: result.envelope.cid, metadataHash: result.metadataHash };
    if (await this.opts.anchor.isAnchored(entry.cid)) {
      return { tx: "", alreadyAnchored: true };
    }
    const receipt = await this.opts.anchor.anchorSingle(entry);
    return { tx: receipt.tx, alreadyAnchored: receipt.newlyAnchored === 0 };
  }
}
