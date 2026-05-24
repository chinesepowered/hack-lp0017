import {
  Publisher,
  BatchAnchor,
  DEFAULT_DELIVERY_TOPIC,
  type PublishResult,
  type DocumentEnvelope,
  type StorageAdapter,
  type DeliveryAdapter,
  type AnchorAdapter,
} from "@whistleblower/indexing-module";
import {
  InMemoryStorage,
  InMemoryDelivery,
  InMemoryAnchor,
} from "@whistleblower/indexing-module/adapters/in-memory";
import { connectAdapters } from "./adapters.js";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

async function bootstrap() {
  const adapters = await connectAdapters();
  const storage: StorageAdapter = adapters.storage ?? new InMemoryStorage();
  const delivery: DeliveryAdapter = adapters.delivery ?? new InMemoryDelivery();
  const anchor: AnchorAdapter = adapters.anchor ?? new InMemoryAnchor();
  const mode = adapters.mode;

  const publisher = new Publisher({ storage, delivery, anchor, topic: DEFAULT_DELIVERY_TOPIC });
  $("topic").textContent = DEFAULT_DELIVERY_TOPIC;

  const tableBody = $<HTMLTableSectionElement>("feed").getElementsByTagName("tbody")[0];
  const seenCids = new Set<string>();
  let lastResult: PublishResult | null = null;

  // Live feed: subscribe to the same topic the publisher broadcasts to.
  await delivery.subscribe(DEFAULT_DELIVERY_TOPIC, async (env) => {
    if (seenCids.has(env.cid)) return;
    seenCids.add(env.cid);
    const anchored = await anchor.isAnchored(env.cid).catch(() => false);
    addFeedRow(tableBody, env, anchored);
  });

  // ---- 1. Upload + broadcast ----
  $<HTMLFormElement>("upload-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const fileInput = $<HTMLInputElement>("file");
    const file = fileInput.files?.[0];
    if (!file) return;

    const submit = $<HTMLButtonElement>("submit");
    submit.disabled = true;
    submit.textContent = "Uploading…";

    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const tags = $<HTMLInputElement>("tags").value
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
      const result = await publisher.publish(bytes, {
        title: $<HTMLInputElement>("title").value || file.name,
        description: $<HTMLTextAreaElement>("description").value,
        content_type: file.type || "application/octet-stream",
        tags,
      });
      lastResult = result;
      showResult($("upload-result"), {
        ok: true,
        cid: result.envelope.cid,
        size: result.envelope.size_bytes,
        topic: DEFAULT_DELIVERY_TOPIC,
        metadata_hash: hex(result.metadataHash),
        mode,
      });
      $("anchor-section").hidden = false;
    } catch (err) {
      showResult($("upload-result"), { ok: false, error: String(err) });
    } finally {
      submit.disabled = false;
      submit.textContent = "Upload & broadcast";
    }
  });

  // ---- 2. Anchor (optional) ----
  $<HTMLButtonElement>("anchor-btn").addEventListener("click", async () => {
    if (!lastResult) return;
    const btn = $<HTMLButtonElement>("anchor-btn");
    btn.disabled = true;
    btn.textContent = "Anchoring…";
    try {
      const r = await publisher.anchor(lastResult);
      showResult($("anchor-result"), {
        ok: true,
        ...r,
        registry: mode === "lez" ? "LEZ devnet registry" : "in-memory registry (dev mode)",
      });
      // Mark in the feed.
      markAnchored(tableBody, lastResult.envelope.cid);
    } catch (err) {
      showResult($("anchor-result"), { ok: false, error: String(err) });
    } finally {
      btn.disabled = false;
      btn.textContent = "Anchor this document";
    }
  });

  // Background batcher — only run in dev/mock mode. In production a
  // standalone CLI runs continuously; the app just observes the feed.
  if (mode !== "lez") {
    const batcher = new BatchAnchor({
      delivery, anchor,
      minBatch: 1, maxBatch: 50, maxBufferMs: 5_000,
      onBatchAnchored: (ev) => {
        for (const cid of ev.cids) markAnchored(tableBody, cid);
      },
    });
    await batcher.start();
  }
}

function addFeedRow(tbody: HTMLTableSectionElement, env: DocumentEnvelope, anchored: boolean) {
  const tr = document.createElement("tr");
  tr.dataset.cid = env.cid;
  tr.innerHTML = `
    <td>${formatTime(env.timestamp)}</td>
    <td class="cid" title="${env.cid}">${env.cid}</td>
    <td>${escapeHtml(env.title)}</td>
    <td>${formatSize(env.size_bytes)}</td>
    <td class="status">${anchored
      ? '<span class="badge anchored">anchored</span>'
      : '<span class="badge pending">pending</span>'}</td>
  `;
  tbody.prepend(tr);
}

function markAnchored(tbody: HTMLTableSectionElement, cid: string) {
  for (const tr of Array.from(tbody.rows)) {
    if (tr.dataset.cid === cid) {
      const status = tr.querySelector(".status");
      if (status) status.innerHTML = '<span class="badge anchored">anchored</span>';
    }
  }
}

function showResult(el: HTMLElement, payload: Record<string, unknown>) {
  el.hidden = false;
  el.className = payload.ok ? "ok" : "err";
  el.textContent = JSON.stringify(payload, null, 2);
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function formatTime(ts: number): string {
  return new Date(ts).toISOString().slice(11, 19);
}

function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

bootstrap().catch((e) => {
  console.error(e);
  const el = $("upload-result");
  el.hidden = false;
  el.className = "err";
  el.textContent = `bootstrap failed: ${String(e)}`;
});
