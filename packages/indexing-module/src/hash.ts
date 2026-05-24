/**
 * SHA-256 over the canonical envelope bytes. This is the value that goes
 * on-chain as `metadata_hash`, so it MUST be reproducible byte-for-byte
 * by any party that has the envelope.
 *
 * We deliberately avoid the Node-only `crypto` import to keep the module
 * runnable in browsers (Basecamp app) and Node (CLI) from one entrypoint.
 */
export async function sha256(bytes: Uint8Array | string): Promise<Uint8Array> {
  const src = typeof bytes === "string" ? new TextEncoder().encode(bytes) : bytes;
  // Copy into a fresh ArrayBuffer (not SharedArrayBuffer) so the input
  // matches Web Crypto's BufferSource type across browser + Node.
  const buf = new ArrayBuffer(src.byteLength);
  new Uint8Array(buf).set(src);
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return new Uint8Array(digest);
}

export function toHex(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) {
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}

export function fromHex(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) throw new Error("invalid hex length");
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }
  return out;
}
