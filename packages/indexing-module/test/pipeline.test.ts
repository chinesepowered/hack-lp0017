import { describe, expect, test } from "vitest";
import { Publisher, BatchAnchor } from "../src/index.js";
import { InMemoryAnchor, InMemoryDelivery, InMemoryStorage } from "../src/adapters/in-memory.js";

function fixedClock() {
  let t = 1_700_000_000_000;
  return () => t++;
}

describe("end-to-end pipeline against in-memory adapters", () => {
  test("publish → broadcast → batch anchor flows a CID through to the registry", async () => {
    const storage = new InMemoryStorage();
    const delivery = new InMemoryDelivery();
    const anchor = new InMemoryAnchor();

    const publisher = new Publisher({ storage, delivery, now: fixedClock() });
    const batcher = new BatchAnchor({ delivery, anchor, minBatch: 1, maxBatch: 50 });
    await batcher.start();

    const file = new TextEncoder().encode("This is a leaked memo.");
    const result = await publisher.publish(file, {
      title: "Internal memo",
      description: "Q3 forecast revisions",
      content_type: "text/plain",
      tags: ["leak", "finance"],
    });

    await batcher.flush();

    expect(await anchor.isAnchored(result.envelope.cid)).toBe(true);
    expect(batcher.status().totalAnchored).toBe(1);
    await batcher.stop();
  });

  test("delivery broadcast is deduplicated on (topic, cid)", async () => {
    const delivery = new InMemoryDelivery();
    const seen: string[] = [];
    await delivery.subscribe("/t", (env) => {
      seen.push(env.cid);
    });

    const env = {
      schema: "whistleblower/v1" as const,
      cid: "bafy-x",
      title: "t",
      description: "",
      content_type: "text/plain",
      size_bytes: 1,
      timestamp: 1,
    };
    await delivery.publish("/t", env);
    await delivery.publish("/t", env);
    await delivery.publish("/t", env);
    expect(seen).toEqual(["bafy-x"]);
  });

  test("batch anchor is idempotent across restart with persisted state", async () => {
    const delivery = new InMemoryDelivery();
    const anchor = new InMemoryAnchor();
    const publisher = new Publisher({ storage: new InMemoryStorage(), delivery, now: fixedClock() });

    const b1 = new BatchAnchor({ delivery, anchor, minBatch: 1 });
    await b1.start();
    const r = await publisher.publish(new Uint8Array([1, 2, 3]), {
      title: "doc",
      description: "",
      content_type: "application/octet-stream",
    });
    await b1.flush();
    const snap = b1.snapshot();
    await b1.stop();

    expect(snap.seenCids).toContain(r.envelope.cid);
    expect(await anchor.isAnchored(r.envelope.cid)).toBe(true);

    // Restart with persisted state — the same envelope is re-broadcast.
    const b2 = new BatchAnchor({
      delivery,
      anchor,
      minBatch: 1,
      initialState: { seenCids: snap.seenCids },
    });
    await b2.start();
    await delivery.publish("/logos/whistleblower/v1/documents", r.envelope);
    await b2.flush();
    expect(b2.status().pendingCids).toBe(0);
    expect(b2.status().totalAnchored).toBe(0);
    await b2.stop();
  });

  test("upload retries on transient storage failure", async () => {
    let attempts = 0;
    const flakyStorage = {
      async put(bytes: Uint8Array) {
        attempts++;
        if (attempts < 3) throw new Error("network down");
        return { cid: "bafy-after-retry", sizeBytes: bytes.byteLength };
      },
    };
    const delivery = new InMemoryDelivery();
    const publisher = new Publisher({
      storage: flakyStorage,
      delivery,
      retry: { baseDelayMs: 1, maxAttempts: 5, sleep: async () => {} },
    });
    const r = await publisher.publish(new Uint8Array([1]), {
      title: "t",
      description: "",
      content_type: "text/plain",
    });
    expect(r.envelope.cid).toBe("bafy-after-retry");
    expect(attempts).toBe(3);
  });
});
