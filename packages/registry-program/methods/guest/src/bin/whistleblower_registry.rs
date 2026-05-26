#![no_main]

use spel_framework::prelude::*;
use whistleblower_registry_core::{AnchorEntry, MAX_BATCH};

risc0_zkvm::guest::entry!(main);

#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct AnchorAccount {
    pub cid: String,
    pub metadata_hash: [u8; 32],
    pub anchor_timestamp: u64,
    pub anchored_by: [u8; 32],
}

#[lez_program]
mod whistleblower_registry {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    pub fn anchor_single(
        #[account(init, pda = [const("wb_v1"), arg("cid")])]
        mut record: AccountWithMetadata,
        #[account(signer)]
        anchorer: AccountWithMetadata,
        cid: String,
        metadata_hash: [u8; 32],
        block_time: u64,
    ) -> SpelResult {
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

    #[instruction]
    pub fn anchor_batch(
        #[account(signer)]
        anchorer: AccountWithMetadata,
        #[account(init, pda = [const("wb_v1"), arg("entries[i].cid")])]
        mut records: Vec<AccountWithMetadata>,
        entries: Vec<AnchorEntry>,
        block_time: u64,
    ) -> SpelResult {
        if entries.is_empty() {
            return Err(SpelError::Custom {
                code: 1,
                message: "batch must contain at least one entry".to_string(),
            });
        }
        if entries.len() > MAX_BATCH {
            return Err(SpelError::Custom {
                code: 2,
                message: format!("batch size {} exceeds MAX_BATCH {}", entries.len(), MAX_BATCH),
            });
        }
        if records.len() != entries.len() {
            return Err(SpelError::Custom {
                code: 3,
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

        let mut all = vec![anchorer];
        all.extend(records);
        Ok(SpelOutput::execute(all, vec![]))
    }

    #[instruction]
    pub fn lookup(
        #[account(pda = [const("wb_v1"), arg("cid")])]
        record: AccountWithMetadata,
        cid: String,
    ) -> SpelResult {
        let _ = cid;
        Ok(SpelOutput::execute(vec![record], vec![]))
    }
}
