#![no_main]

//! Whistleblower registry — a SPEL program that anchors document CIDs.
//!
//! The on-chain registry stores `(cid, metadata_hash, anchor_timestamp)`
//! per document. It accepts both single-CID and batched submissions (up
//! to MAX_BATCH per transaction). Submissions are idempotent: re-anchoring
//! an already-registered CID is a no-op (does NOT fail).
//!
//! Storage layout: one PDA per CID, keyed by `["wb_v1", cid]`. This lets
//! us look up a single anchor in O(1) with `spel inspect`, and lets the
//! batch handler initialise N independent PDAs in one TX.

use spel_framework::prelude::*;
use whistleblower_registry_core::{AnchorEntry, AnchorRecord, MAX_BATCH};

risc0_zkvm::guest::entry!(main);

/// Per-CID anchor record. `#[account_type]` registers it in the IDL so
/// `spel inspect <pda> --type AnchorRecord` decodes the bytes into JSON.
#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct AnchorAccount {
    pub cid: String,
    pub metadata_hash: [u8; 32],
    pub anchor_timestamp: u64,
    /// Author of the anchor TX. Recorded for audit only — the registry
    /// itself is permissionless.
    pub anchored_by: [u8; 32],
}

#[lez_program]
mod whistleblower_registry {
    #[allow(unused_imports)]
    use super::*;

    /// Anchor a single (cid, metadata_hash). Idempotent: if the PDA for
    /// the CID already exists with the same metadata_hash, this instruction
    /// silently succeeds with no state change.
    #[instruction]
    pub fn anchor_single(
        #[account(init, pda = derive(b"wb_v1", arg("cid")))]
        mut record: AnchorWithMetadata,
        #[account(signer)]
        anchorer: AccountWithMetadata,
        cid: String,
        metadata_hash: [u8; 32],
        block_time: u64,
    ) -> SpelResult {
        // If the account is already initialised (i.e. its data is
        // non-empty), treat this as a duplicate and short-circuit.
        let data: Vec<u8> = record.account.data.clone().into();
        if !data.is_empty() {
            return Ok(SpelOutput::execute(vec![record, anchorer], vec![]));
        }

        let rec = AnchorAccount {
            cid: cid.clone(),
            metadata_hash,
            anchor_timestamp: block_time,
            anchored_by: *anchorer.account_id.value(),
        };
        let bytes = borsh::to_vec(&rec).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        record.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![record, anchorer], vec![]))
    }

    /// Anchor up to MAX_BATCH entries in a single TX. Each entry is
    /// idempotent independently — duplicates within the batch are
    /// collapsed; pre-existing on-chain entries are skipped without
    /// failing the whole batch.
    ///
    /// The N PDAs are declared via `#[account(slice, ...)]` so the SPEL
    /// runtime materialises one Account per `entries[i]`. The order of
    /// accounts MUST match the order of `entries`.
    #[instruction]
    pub fn anchor_batch(
        #[account(signer)]
        anchorer: AccountWithMetadata,
        #[account(slice, init_if_empty, pda = derive(b"wb_v1", arg("entries[i].cid")))]
        mut records: Vec<AnchorWithMetadata>,
        entries: Vec<AnchorEntry>,
        block_time: u64,
    ) -> SpelResult {
        if entries.is_empty() {
            return Err(SpelError::InvalidArgument {
                message: "batch must contain at least one entry".to_string(),
            });
        }
        if entries.len() > MAX_BATCH {
            return Err(SpelError::InvalidArgument {
                message: format!("batch size {} exceeds MAX_BATCH {}", entries.len(), MAX_BATCH),
            });
        }
        if records.len() != entries.len() {
            return Err(SpelError::InvalidArgument {
                message: format!(
                    "account/argument mismatch: {} records vs {} entries",
                    records.len(),
                    entries.len()
                ),
            });
        }

        for (record, entry) in records.iter_mut().zip(entries.iter()) {
            let data: Vec<u8> = record.account.data.clone().into();
            if !data.is_empty() {
                // Already anchored — skip silently (idempotent).
                continue;
            }
            let rec = AnchorAccount {
                cid: entry.cid.clone(),
                metadata_hash: entry.metadata_hash,
                anchor_timestamp: block_time,
                anchored_by: *anchorer.account_id.value(),
            };
            let bytes = borsh::to_vec(&rec).map_err(|e| SpelError::SerializationError {
                message: e.to_string(),
            })?;
            record.account.data = bytes.try_into().unwrap();
        }

        let mut outputs: Vec<AccountWithMetadata> = records.into_iter().map(|r| r.into()).collect();
        outputs.push(anchorer);
        Ok(SpelOutput::execute(outputs, vec![]))
    }

    /// Read-only lookup. Returns the anchor PDA unchanged so the caller
    /// can decode it via `spel inspect <pda> --type AnchorAccount`.
    #[instruction]
    pub fn lookup(
        #[account(pda = derive(b"wb_v1", arg("cid")))]
        record: AnchorWithMetadata,
        cid: String,
    ) -> SpelResult {
        let _ = cid; // bound by PDA derivation; included for IDL surface clarity
        Ok(SpelOutput::execute(vec![record], vec![]))
    }
}

// `AnchorWithMetadata` is the spel-framework alias for an
// `AccountWithMetadata` whose `data` is parsed as `AnchorAccount`. Some
// versions of the framework expose this name; older versions only have
// `AccountWithMetadata`. We alias here so the instruction signatures
// read cleanly above.
type AnchorWithMetadata = AccountWithMetadata;
