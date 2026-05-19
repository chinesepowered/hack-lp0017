import { describe, expect, test } from "vitest";
import { canonicalize, type DocumentEnvelope } from "../src/envelope.js";
import { sha256, toHex } from "../src/hash.js";

/**
 * Cross-language fixture.
 *
 * The Rust port (`packages/whistleblower-core/tests/cross_lang.rs`)
 * asserts the SAME hash for the SAME envelope. If you change envelope
 * canonicalisation, both fixtures must be updated together — otherwise
 * publishers and on-chain anchorers will compute different
 * `metadata_hash` values and the registry will reject as malformed.
 */
describe("cross-language envelope fixture", () => {
  const env: DocumentEnvelope = {
    schema: "whistleblower/v1",
    cid: "bafy0001",
    title: "Internal memo",
    description: "Q3 forecast revisions",
    content_type: "application/pdf",
    size_bytes: 1234,
    timestamp: 1_700_000_000_000,
    tags: ["leak", "finance"],
  };

  test("canonical bytes match the pinned form", () => {
    expect(canonicalize(env)).toBe(
      '{"schema":"whistleblower/v1","cid":"bafy0001","title":"Internal memo","description":"Q3 forecast revisions","content_type":"application/pdf","size_bytes":1234,"timestamp":1700000000000,"tags":["leak","finance"]}',
    );
  });

  test("metadata_hash matches the Rust port byte-for-byte", async () => {
    const h = toHex(await sha256(canonicalize(env)));
    expect(h).toBe("fce3429b051749a9a401d054bd23efcbb04288a47347f008417c025bcf545d3c");
  });
});
