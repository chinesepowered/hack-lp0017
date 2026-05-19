/**
 * Metadata envelope broadcast over Logos Delivery after a successful upload.
 *
 * The envelope is the canonical, hash-stable representation of a document's
 * metadata. Its serialised form is what gets hashed for on-chain anchoring,
 * so the serialisation must be deterministic across producers and consumers.
 */
export interface DocumentEnvelope {
  /** Content identifier returned by Logos Storage. */
  cid: string;
  /** Short human-readable title (1..256 bytes UTF-8). */
  title: string;
  /** Longer description (0..4096 bytes UTF-8). */
  description: string;
  /** MIME type, e.g. "application/pdf". */
  content_type: string;
  /** Document size in bytes as reported by storage. */
  size_bytes: number;
  /** Unix epoch milliseconds at which the envelope was produced. */
  timestamp: number;
  /** Optional free-form tags. Order is preserved but not semantically meaningful. */
  tags?: string[];
  /** Envelope schema version. Bump on breaking changes. */
  schema: "whistleblower/v1";
}

export const ENVELOPE_SCHEMA = "whistleblower/v1" as const;

/**
 * Canonical (deterministic) JSON serialisation of an envelope.
 *
 * Keys are emitted in a fixed order so that two producers running the same
 * indexing-module version always agree on the bytes that get hashed.
 * Optional `tags` is included only when present and non-empty.
 */
export function canonicalize(env: DocumentEnvelope): string {
  const ordered: Record<string, unknown> = {
    schema: env.schema,
    cid: env.cid,
    title: env.title,
    description: env.description,
    content_type: env.content_type,
    size_bytes: env.size_bytes,
    timestamp: env.timestamp,
  };
  if (env.tags && env.tags.length > 0) {
    ordered.tags = [...env.tags];
  }
  return JSON.stringify(ordered);
}

const MAX_TITLE = 256;
const MAX_DESC = 4096;
const MAX_TAG_LEN = 64;
const MAX_TAGS = 32;

export function validateEnvelope(env: DocumentEnvelope): void {
  if (env.schema !== ENVELOPE_SCHEMA) {
    throw new Error(`unsupported envelope schema: ${env.schema}`);
  }
  if (!env.cid || typeof env.cid !== "string") {
    throw new Error("cid is required");
  }
  if (utf8Bytes(env.title) > MAX_TITLE) {
    throw new Error(`title exceeds ${MAX_TITLE} bytes`);
  }
  if (utf8Bytes(env.description) > MAX_DESC) {
    throw new Error(`description exceeds ${MAX_DESC} bytes`);
  }
  if (!env.content_type) {
    throw new Error("content_type is required");
  }
  if (!Number.isFinite(env.size_bytes) || env.size_bytes < 0) {
    throw new Error("size_bytes must be a non-negative finite number");
  }
  if (!Number.isFinite(env.timestamp) || env.timestamp <= 0) {
    throw new Error("timestamp must be a positive epoch-millis value");
  }
  if (env.tags) {
    if (env.tags.length > MAX_TAGS) {
      throw new Error(`too many tags (>${MAX_TAGS})`);
    }
    for (const t of env.tags) {
      if (utf8Bytes(t) > MAX_TAG_LEN) {
        throw new Error(`tag exceeds ${MAX_TAG_LEN} bytes: ${t}`);
      }
    }
  }
}

function utf8Bytes(s: string): number {
  return new TextEncoder().encode(s).length;
}
