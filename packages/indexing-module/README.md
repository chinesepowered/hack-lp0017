# @whistleblower/indexing-module

Reusable upload → broadcast → anchor pipeline for the Logos stack. Extracted
from the Whistleblower Basecamp app (LP-0017) so any Logos application that
needs censorship-resistant content publication can drop it in.

## Why this exists

The bounty (LP-0017) requires that the Whistleblower app's pipeline be
extractable as a "self-contained module with a documented API, reusable by
other Logos apps without depending on the Whistleblower Basecamp app
itself." This package is that module.

The same logic is also published as a Rust crate (`whistleblower-core`) for
native callers — the batch-anchor CLI, the QML Basecamp app FFI, and any
other Rust client. Both implementations share the envelope schema and
canonical serialisation byte-for-byte, so producers and consumers in
different languages always agree on the `metadata_hash` value committed
on-chain.

## Three capabilities, decoupled

| Class           | Role                                                                                  |
| --------------- | ------------------------------------------------------------------------------------- |
| `Publisher`     | upload bytes to Logos Storage, broadcast envelope on Logos Delivery                   |
| `BatchAnchor`   | subscribe to a Delivery topic and commit accumulated CIDs in batched transactions     |
| adapter traits  | swap storage / delivery / anchor backends (real Logos clients vs. in-memory for test) |

The publisher does NOT depend on the anchor — a publisher with no on-chain
funds can still upload + broadcast and let an altruistic third party anchor
later. This is the whole point of the design.

## Install

```bash
npm i @whistleblower/indexing-module
```

## Quick start

```ts
import {
  Publisher,
  BatchAnchor,
  DEFAULT_DELIVERY_TOPIC,
} from "@whistleblower/indexing-module";
import {
  InMemoryStorage,
  InMemoryDelivery,
  InMemoryAnchor,
} from "@whistleblower/indexing-module/adapters/in-memory";

const storage = new InMemoryStorage();   // swap for real Logos Storage client
const delivery = new InMemoryDelivery(); // swap for real Logos Delivery client
const anchor = new InMemoryAnchor();     // swap for real LEZ registry client

// Publisher side: upload + broadcast.
const publisher = new Publisher({ storage, delivery, anchor });
const result = await publisher.publish(fileBytes, {
  title: "Internal memo",
  description: "Q3 forecast revisions, page 4 redacted",
  content_type: "application/pdf",
  tags: ["leak", "finance"],
});
console.log("broadcast envelope:", result.envelope);

// Optional: publisher anchors their own document immediately.
await publisher.anchor(result);

// Altruistic-third-party side: accumulate + batch anchor.
const batcher = new BatchAnchor({ delivery, anchor, minBatch: 10, maxBatch: 50 });
await batcher.start();
// … long-running …
await batcher.flush();
```

## API

### `Publisher`

```ts
new Publisher({ storage, delivery, anchor?, topic?, retry?, now? })

publisher.publish(bytes, meta): Promise<PublishResult>
publisher.anchor(result):       Promise<{ tx, alreadyAnchored }>
```

`publish` does three things atomically from the caller's POV:

1. Uploads bytes to `StorageAdapter.put` with exponential back-off retry on
   transient failures (`retry` option). Surfaces `RetryExhaustedError`
   when the retry budget is exhausted.
2. Builds a deterministic envelope and hashes it (`sha256(canonicalize(env))`).
   The hash is what later goes on-chain.
3. Publishes the envelope to the Logos Delivery topic. The adapter is
   responsible for deduplicating identical (topic, cid) publications.

### `BatchAnchor`

```ts
new BatchAnchor({ delivery, anchor, topic?, minBatch?, maxBatch?, maxBufferMs?, onBatchAnchored?, initialState? })

batcher.start()          // subscribe to topic
batcher.stop()           // unsubscribe
batcher.flush()          // force-flush partial batch
batcher.status()         // { pendingCids, totalAnchored, lastBatchTx }
batcher.snapshot()       // { seenCids } — persist to disk for resume
```

Resume semantics: the `seenCids` snapshot is the union of (a) CIDs the
batcher has already anchored and (b) CIDs it observed as already-anchored
on the chain. Persist it on every `onBatchAnchored` callback and feed it
back as `initialState` after a restart — guaranteed not to re-anchor
already-registered CIDs even if the Delivery topic re-broadcasts.

### Envelope schema (`whistleblower/v1`)

```ts
{
  schema:        "whistleblower/v1",
  cid:           string,
  title:         string,    // ≤ 256 UTF-8 bytes
  description:   string,    // ≤ 4096 UTF-8 bytes
  content_type:  string,    // e.g. "application/pdf"
  size_bytes:    number,    // non-negative finite integer
  timestamp:     number,    // unix epoch millis
  tags?:         string[],  // ≤ 32 tags, each ≤ 64 UTF-8 bytes
}
```

The canonical serialisation orders keys as listed above and OMITS `tags`
when it is missing or empty. `metadata_hash = SHA-256(canonicalized JSON)`.

### Adapter traits

```ts
interface StorageAdapter  { put(bytes, hint?): Promise<{ cid, sizeBytes }>; get?(cid): Promise<Uint8Array> }
interface DeliveryAdapter { publish(topic, envelope): Promise<void>;        subscribe(topic, handler): Promise<unsub> }
interface AnchorAdapter   { anchorSingle(e): Promise<r>; anchorBatch(es): Promise<r>; isAnchored(cid): Promise<boolean> }
```

The in-memory implementations under `adapters/in-memory` are the reference
mocks — production code wires the real Logos clients here.

## Wiring real Logos clients

The `logos-co/logos-delivery-module` and `logos-co/logos-basecamp` modules
expose Qt/C++ APIs. Two bridging options:

- **Web Basecamp app (this package)**: a thin JSON-RPC bridge exposed by
  the Basecamp host calls the C++ Delivery module. See `../basecamp-app`
  for the bridge code.
- **Native (Rust / C++)**: import `whistleblower-core` (the Rust crate)
  and link directly via C FFI. See the workspace README.

## License

MIT OR Apache-2.0
