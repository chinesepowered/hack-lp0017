use sha2::{Digest, Sha256};

use crate::envelope::{canonicalize, DocumentEnvelope, EnvelopeError};

/// SHA-256 over the canonical envelope bytes. Returns the 32-byte digest
/// that goes on-chain as `metadata_hash`.
pub fn metadata_hash(env: &DocumentEnvelope) -> Result<[u8; 32], EnvelopeError> {
    let bytes = canonicalize(env)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(h.finalize().into())
}

/// Lower-cased hex string of a digest, with no `0x` prefix.
pub fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub fn from_hex(s: &str) -> Result<Vec<u8>, hex::FromHexError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    hex::decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::ENVELOPE_SCHEMA;

    #[test]
    fn deterministic_hash() {
        let env = DocumentEnvelope {
            schema: ENVELOPE_SCHEMA.into(),
            cid: "bafy0001".into(),
            title: "t".into(),
            description: "d".into(),
            content_type: "text/plain".into(),
            size_bytes: 1,
            timestamp: 1_700_000_000_000,
            tags: None,
        };
        let h1 = metadata_hash(&env).unwrap();
        let h2 = metadata_hash(&env).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 32);
    }
}
