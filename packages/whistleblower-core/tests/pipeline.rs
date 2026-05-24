//! End-to-end pipeline tests using the in-memory adapters.

use std::sync::Arc;
use std::time::Duration;

use whistleblower_core::adapters::in_memory::{InMemoryAnchor, InMemoryDelivery, InMemoryStorage};
use whistleblower_core::adapters::{AnchorAdapter, DeliveryAdapter, StorageAdapter};
use whistleblower_core::{
    BatchAnchor, BatchAnchorConfig, PublishMeta, Publisher, PublisherConfig, DEFAULT_DELIVERY_TOPIC,
};

#[tokio::test]
async fn upload_broadcast_anchor_round_trip() {
    let storage: Arc<dyn StorageAdapter> = Arc::new(InMemoryStorage::new());
    let delivery: Arc<dyn DeliveryAdapter> = Arc::new(InMemoryDelivery::new());
    let anchor: Arc<dyn AnchorAdapter> = Arc::new(InMemoryAnchor::new());

    let publisher = Publisher::new(
        storage.clone(),
        delivery.clone(),
        Some(anchor.clone()),
        PublisherConfig::default(),
    );

    let mut batcher = BatchAnchor::new(
        delivery.clone(),
        anchor.clone(),
        BatchAnchorConfig {
            min_batch: 1,
            max_batch: 50,
            max_buffer: Duration::from_secs(60),
            ..Default::default()
        },
    );
    batcher.start().await.unwrap();

    let bytes = b"This is a leaked memo.".to_vec();
    let result = publisher
        .publish(
            &bytes,
            PublishMeta {
                title: "Internal memo".into(),
                description: "Q3 forecast revisions".into(),
                content_type: "text/plain".into(),
                tags: vec!["leak".into(), "finance".into()],
            },
        )
        .await
        .unwrap();

    assert!(batcher.wait_for_envelope(Duration::from_secs(2)).await);
    let ev = batcher.flush().await.unwrap();
    assert!(ev.is_some(), "expected a batch event");
    let ev = ev.unwrap();
    assert_eq!(ev.newly_anchored, 1);
    assert_eq!(ev.cids, vec![result.envelope.cid.clone()]);
    assert!(ev.compute_units.is_some());

    assert!(anchor.is_anchored(&result.envelope.cid).await.unwrap());
    batcher.stop().await;
}

#[tokio::test]
async fn batch_anchor_is_idempotent_across_restart() {
    let storage: Arc<dyn StorageAdapter> = Arc::new(InMemoryStorage::new());
    let delivery: Arc<dyn DeliveryAdapter> = Arc::new(InMemoryDelivery::new());
    let anchor: Arc<dyn AnchorAdapter> = Arc::new(InMemoryAnchor::new());

    let publisher = Publisher::new(
        storage.clone(),
        delivery.clone(),
        None,
        PublisherConfig::default(),
    );

    let mut b1 = BatchAnchor::new(
        delivery.clone(),
        anchor.clone(),
        BatchAnchorConfig {
            min_batch: 1,
            max_batch: 50,
            max_buffer: Duration::from_secs(60),
            ..Default::default()
        },
    );
    b1.start().await.unwrap();

    let r = publisher
        .publish(
            b"hello",
            PublishMeta {
                title: "doc".into(),
                description: String::new(),
                content_type: "text/plain".into(),
                tags: vec![],
            },
        )
        .await
        .unwrap();

    assert!(b1.wait_for_envelope(Duration::from_secs(2)).await);
    b1.flush().await.unwrap();
    let snapshot = b1.snapshot().await;
    b1.stop().await;
    assert!(snapshot.iter().any(|c| c == &r.envelope.cid));

    // Simulate a network restart: fresh delivery instance with no dedup
    // memory, replays the same envelope. The batcher resumes from persisted
    // state and must skip the already-anchored CID without enqueuing it.
    let delivery2: Arc<dyn DeliveryAdapter> = Arc::new(InMemoryDelivery::new());
    let mut b2 = BatchAnchor::new(
        delivery2.clone(),
        anchor.clone(),
        BatchAnchorConfig {
            min_batch: 1,
            max_batch: 50,
            max_buffer: Duration::from_secs(60),
            ..Default::default()
        },
    );
    b2.set_initial_seen(snapshot.clone()).await;
    b2.start().await.unwrap();
    delivery2
        .publish(DEFAULT_DELIVERY_TOPIC, &r.envelope)
        .await
        .unwrap();
    // Wait for handler to see + process the envelope (it'll skip without adding to pending).
    assert!(b2.wait_for_envelope(Duration::from_secs(2)).await);
    let pending_after = b2.status().await.pending_cids;
    assert_eq!(
        pending_after, 0,
        "batcher must skip already-seen CID on restart"
    );
    b2.stop().await;
}

#[tokio::test]
async fn delivery_dedups_repeated_publishes() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let delivery = Arc::new(InMemoryDelivery::new());
    let count = Arc::new(AtomicUsize::new(0));
    let count_h = count.clone();

    let handler: whistleblower_core::EnvelopeHandler = Arc::new(move |_env| {
        let c = count_h.clone();
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
        })
    });

    let _h = delivery.subscribe("/t", handler).await.unwrap();
    let env = whistleblower_core::DocumentEnvelope {
        schema: whistleblower_core::ENVELOPE_SCHEMA.into(),
        cid: "bafy-x".into(),
        title: "t".into(),
        description: String::new(),
        content_type: "text/plain".into(),
        size_bytes: 1,
        timestamp: 1,
        tags: None,
    };
    for _ in 0..3 {
        delivery.publish("/t", &env).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "dedup must collapse identical publishes"
    );
}
