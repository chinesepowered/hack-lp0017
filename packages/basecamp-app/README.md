# Whistleblower — Basecamp app

A Logos Basecamp app that lets anyone upload a document, broadcast it
over Logos Delivery, and (optionally) anchor it on-chain via the LEZ
registry.

The app ships in two flavours from a single source of truth:

- **Web preview** (this directory's `index.html` + Vite build) — runs in
  any modern browser, talks to the `window.logos.bridge` injected by
  Basecamp. With no bridge present it falls back to the in-memory
  adapters from `@whistleblower/indexing-module/adapters/in-memory` so
  you can demo the flow without Logos infrastructure.
- **Native QML** (`qml/Main.qml` + the manifest) — loadable by Logos
  Basecamp as a first-class native app. Calls the same pipeline through
  the C FFI exposed by `whistleblower-core` (build with
  `cargo build -p whistleblower-core --features ffi --release`).

Both share the indexing pipeline; both broadcast on the same Logos
Delivery topic; both write to the same LEZ registry.

## Build the web preview

```bash
npm install
npm run build
# dist/ is the asset bundle. Open dist/index.html in a browser, or run
# `npm run preview` for the dev server at http://127.0.0.1:4173/
```

## Build the QML app for Basecamp

The Basecamp shell expects a directory containing:

```
basecamp.manifest.json
dist/                       (web fallback assets — already produced by `npm run build`)
qml/Main.qml                (native UI)
lib/libwhistleblower_core.* (FFI binary, copied from `target/release` after `cargo build -p whistleblower-core --features ffi --release`)
assets/icon.svg
```

Bundle it as a `.lgx` (the Logos app format) following
[`logos-co/logos-basecamp`](https://github.com/logos-co/logos-basecamp).

## Loading into Basecamp

```bash
logos-basecamp install ./whistleblower.lgx
logos-basecamp launch io.logos.apps.whistleblower
```

## Permissions requested

| Permission                    | Why                                                                      |
| ----------------------------- | ------------------------------------------------------------------------ |
| `logos.storage:put`           | Upload the file to Logos Storage and obtain a CID.                       |
| `logos.delivery:publish`      | Broadcast the metadata envelope on the topic.                            |
| `logos.delivery:subscribe`    | Render the live feed of inbound envelopes from other publishers.         |
| `logos.lez:call`              | Invoke the `anchor_single` instruction on the registry program (opt-in). |

The app does NOT request long-term storage, identity, or contact access.
