import { describe, expect, test } from "vitest";
import { canonicalize, validateEnvelope, type DocumentEnvelope } from "../src/envelope.js";
import { sha256, toHex } from "../src/hash.js";

const base: DocumentEnvelope = {
  schema: "whistleblower/v1",
  cid: "bafy0001",
  title: "Title",
  description: "Description",
  content_type: "application/pdf",
  size_bytes: 1234,
  timestamp: 1_700_000_000_000,
};

describe("envelope canonicalisation", () => {
  test("is stable across reorderings of input keys", () => {
    const a = canonicalize({ ...base });
    const b = canonicalize({
      timestamp: base.timestamp,
      cid: base.cid,
      schema: base.schema,
      size_bytes: base.size_bytes,
      title: base.title,
      content_type: base.content_type,
      description: base.description,
    });
    expect(a).toBe(b);
  });

  test("omits empty tags but includes non-empty tags", () => {
    expect(canonicalize({ ...base, tags: [] })).toBe(canonicalize(base));
    const withTags = canonicalize({ ...base, tags: ["leak", "finance"] });
    expect(withTags).toContain('"tags":["leak","finance"]');
  });

  test("metadata_hash is reproducible byte-for-byte", async () => {
    const h1 = toHex(await sha256(canonicalize(base)));
    const h2 = toHex(await sha256(canonicalize({ ...base })));
    expect(h1).toBe(h2);
    expect(h1).toMatch(/^[0-9a-f]{64}$/);
  });

  test("validation rejects oversized fields", () => {
    expect(() => validateEnvelope({ ...base, title: "x".repeat(300) })).toThrow();
    expect(() => validateEnvelope({ ...base, size_bytes: -1 })).toThrow();
    expect(() => validateEnvelope({ ...base, timestamp: 0 })).toThrow();
    expect(() => validateEnvelope({ ...base, tags: Array(40).fill("t") })).toThrow();
  });

  test("validation accepts the canonical example", () => {
    expect(() => validateEnvelope(base)).not.toThrow();
    expect(() => validateEnvelope({ ...base, tags: ["a", "b"] })).not.toThrow();
  });
});
