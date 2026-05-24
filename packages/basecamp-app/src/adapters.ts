import type { StorageAdapter, DeliveryAdapter, AnchorAdapter } from "@whistleblower/indexing-module";

/**
 * Connect to the running Logos services via the Basecamp bridge.
 *
 * The bridge is a tiny JSON-RPC endpoint exposed by the Basecamp host
 * at `window.logos.bridge` — it forwards storage/delivery calls to the
 * native C++ modules (`logos-delivery-module`, the storage app) and
 * anchor calls to the local LEZ sequencer via `spel`.
 *
 * When the bridge is not present (browser preview, dev mode), this
 * function returns `{ mode: "mock" }` and the caller falls back to
 * the in-memory adapters.
 */
export interface AdapterBundle {
  mode: "mock" | "lez";
  storage?: StorageAdapter;
  delivery?: DeliveryAdapter;
  anchor?: AnchorAdapter;
}

declare global {
  interface Window {
    logos?: {
      bridge?: {
        storagePut(bytes: Uint8Array, hint?: { contentType?: string }): Promise<{ cid: string; sizeBytes: number }>;
        deliveryPublish(topic: string, envelope: unknown): Promise<void>;
        deliverySubscribe(topic: string, handler: (env: unknown) => void): Promise<() => Promise<void>>;
        anchorBatch(entries: { cid: string; metadataHashHex: string }[]): Promise<{ tx: string; newlyAnchored: number; computeUnits?: number }>;
        anchorIsAnchored(cid: string): Promise<boolean>;
      };
    };
  }
}

export async function connectAdapters(): Promise<AdapterBundle> {
  const bridge = window.logos?.bridge;
  if (!bridge) return { mode: "mock" };

  const storage: StorageAdapter = {
    put: (bytes, hint) => bridge.storagePut(bytes, hint),
  };
  const delivery: DeliveryAdapter = {
    publish: (topic, envelope) => bridge.deliveryPublish(topic, envelope),
    subscribe: async (topic, handler) => {
      const unsub = await bridge.deliverySubscribe(topic, (raw) => handler(raw as never));
      return unsub;
    },
  };
  const anchor: AnchorAdapter = {
    anchorSingle: (entry) => bridge.anchorBatch([{ cid: entry.cid, metadataHashHex: toHex(entry.metadataHash) }]),
    anchorBatch: (entries) => bridge.anchorBatch(entries.map((e) => ({ cid: e.cid, metadataHashHex: toHex(e.metadataHash) }))),
    isAnchored: (cid) => bridge.anchorIsAnchored(cid),
  };
  return { mode: "lez", storage, delivery, anchor };
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
}
