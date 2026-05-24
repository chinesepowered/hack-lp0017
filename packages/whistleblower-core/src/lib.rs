//! whistleblower-core — document-indexing module for the Logos stack.
//!
//! The crate is intentionally minimal: it owns the envelope schema, the
//! canonical hash, the retry helper, and three adapter traits that abstract
//! away the actual Logos clients. Concrete adapter implementations live in
//! sibling crates (the batch-anchor CLI, the Basecamp app FFI) so this
//! crate stays usable from `no_std`-adjacent contexts.

pub mod adapters;
pub mod batch_anchor;
pub mod envelope;
pub mod hash;
pub mod publisher;
pub mod retry;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use adapters::{
    AnchorAdapter, AnchorEntry, AnchorReceipt, DeliveryAdapter, EnvelopeHandler, StorageAdapter,
};
pub use batch_anchor::{BatchAnchor, BatchAnchorConfig, BatchAnchorEvent, BatchAnchorStatus};
pub use envelope::{canonicalize, validate, DocumentEnvelope, ENVELOPE_SCHEMA};
pub use hash::metadata_hash;
pub use publisher::{AnchorOutcome, PublishMeta, PublishResult, Publisher, PublisherConfig};
pub use retry::{retry, RetryConfig, RetryError};

/// Canonical Logos Delivery topic used by the Whistleblower app.
pub const DEFAULT_DELIVERY_TOPIC: &str = "/logos/whistleblower/v1/documents";
