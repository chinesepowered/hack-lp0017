use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// On-chain record for a single anchored document.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorRecord {
    /// Content identifier as returned by Logos Storage. Stored as a UTF-8 string.
    pub cid: String,
    /// SHA-256 over the canonical Whistleblower envelope (see indexing module).
    pub metadata_hash: [u8; 32],
    /// Block-time the entry was first recorded.
    pub anchor_timestamp: u64,
}

/// Encoded batch payload accepted by `anchor_batch`. Borsh-serialized.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Default)]
pub struct AnchorBatch {
    pub entries: Vec<AnchorEntry>,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct AnchorEntry {
    pub cid: String,
    pub metadata_hash: [u8; 32],
}

/// Maximum entries per batch — chosen so a serialised payload fits comfortably
/// inside an LEZ transaction even at the worst-case CID length.
pub const MAX_BATCH: usize = 50;
/// Minimum CIDs the batch instruction MUST accept (success criterion).
pub const MIN_REQUIRED_BATCH: usize = 10;
