/**
 * @whistleblower/indexing-module
 *
 * Reusable upload → broadcast → anchor pipeline for the Logos stack.
 *
 * Three orthogonal capabilities are exposed independently so callers can mix
 * and match against their own adapters:
 *
 *   - `Publisher`     uploads + broadcasts (the publisher side)
 *   - `BatchAnchor`   subscribes + anchors (the altruistic-third-party side)
 *   - low-level types (envelope, adapters, hash) for advanced use
 *
 * See README for the integration walkthrough.
 */

export {
  type DocumentEnvelope,
  ENVELOPE_SCHEMA,
  canonicalize,
  validateEnvelope,
} from "./envelope.js";

export { sha256, toHex, fromHex } from "./hash.js";

export {
  retry,
  RetryExhaustedError,
  type RetryOptions,
} from "./retry.js";

export {
  type StorageAdapter,
  type DeliveryAdapter,
  type AnchorAdapter,
  type AnchorEntry,
  type AnchorReceipt,
} from "./adapters/types.js";

export { Publisher, type PublisherOptions, type PublishResult } from "./publisher.js";
export { BatchAnchor, type BatchAnchorOptions, type BatchAnchorStatus } from "./batch-anchor.js";

export const DEFAULT_DELIVERY_TOPIC = "/logos/whistleblower/v1/documents";
