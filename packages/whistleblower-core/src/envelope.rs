use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Envelope schema tag. Bump on breaking schema changes.
pub const ENVELOPE_SCHEMA: &str = "whistleblower/v1";

const MAX_TITLE: usize = 256;
const MAX_DESC: usize = 4096;
const MAX_TAG_LEN: usize = 64;
const MAX_TAGS: usize = 32;

/// Metadata envelope broadcast on Logos Delivery after a successful upload.
///
/// The envelope is canonicalized via [`canonicalize`] and hashed via
/// [`crate::metadata_hash`] to produce the value committed on-chain.
/// Producers and consumers across language ports MUST agree on the
/// canonical bytes — see the cross-language fixture in
/// `packages/whistleblower-core/tests/cross_lang.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentEnvelope {
    pub schema: String,
    pub cid: String,
    pub title: String,
    pub description: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum EnvelopeError {
    #[error("unsupported envelope schema: {0}")]
    UnsupportedSchema(String),
    #[error("cid is required")]
    MissingCid,
    #[error("title exceeds {MAX_TITLE} bytes")]
    TitleTooLong,
    #[error("description exceeds {MAX_DESC} bytes")]
    DescriptionTooLong,
    #[error("content_type is required")]
    MissingContentType,
    #[error("timestamp must be > 0")]
    InvalidTimestamp,
    #[error("too many tags (>{MAX_TAGS})")]
    TooManyTags,
    #[error("tag exceeds {MAX_TAG_LEN} bytes: {0}")]
    TagTooLong(String),
    #[error("canonical serialisation failed: {0}")]
    Serialization(String),
}

/// Deterministic JSON serialisation of an envelope.
///
/// Keys are emitted in a fixed order so that two producers running the same
/// crate version always agree byte-for-byte on what gets hashed. The
/// `tags` field is included only when non-empty (matches the TypeScript port).
pub fn canonicalize(env: &DocumentEnvelope) -> Result<Vec<u8>, EnvelopeError> {
    // Build the ordered map manually so the key order is part of the contract
    // rather than at the mercy of serde_json's BTreeMap iteration.
    let include_tags = env.tags.as_ref().is_some_and(|t| !t.is_empty());
    let mut s = String::with_capacity(256 + env.description.len());
    s.push('{');
    push_kv_string(&mut s, "schema", &env.schema, true);
    push_kv_string(&mut s, "cid", &env.cid, false);
    push_kv_string(&mut s, "title", &env.title, false);
    push_kv_string(&mut s, "description", &env.description, false);
    push_kv_string(&mut s, "content_type", &env.content_type, false);
    push_kv_number(&mut s, "size_bytes", env.size_bytes, false);
    push_kv_number(&mut s, "timestamp", env.timestamp, false);
    if include_tags {
        push_kv_tags(&mut s, env.tags.as_deref().unwrap_or(&[]));
    }
    s.push('}');
    Ok(s.into_bytes())
}

fn push_kv_string(out: &mut String, k: &str, v: &str, first: bool) {
    if !first {
        out.push(',');
    }
    out.push('"');
    out.push_str(k);
    out.push_str("\":");
    push_json_string(out, v);
}

fn push_kv_number(out: &mut String, k: &str, v: u64, first: bool) {
    if !first {
        out.push(',');
    }
    out.push('"');
    out.push_str(k);
    out.push_str("\":");
    out.push_str(&v.to_string());
}

fn push_kv_tags(out: &mut String, tags: &[String]) {
    out.push_str(",\"tags\":[");
    for (i, t) in tags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_string(out, t);
    }
    out.push(']');
}

/// JSON string encoder matching `JSON.stringify` (no \u escaping of printable ASCII).
fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn validate(env: &DocumentEnvelope) -> Result<(), EnvelopeError> {
    if env.schema != ENVELOPE_SCHEMA {
        return Err(EnvelopeError::UnsupportedSchema(env.schema.clone()));
    }
    if env.cid.is_empty() {
        return Err(EnvelopeError::MissingCid);
    }
    if env.title.len() > MAX_TITLE {
        return Err(EnvelopeError::TitleTooLong);
    }
    if env.description.len() > MAX_DESC {
        return Err(EnvelopeError::DescriptionTooLong);
    }
    if env.content_type.is_empty() {
        return Err(EnvelopeError::MissingContentType);
    }
    if env.timestamp == 0 {
        return Err(EnvelopeError::InvalidTimestamp);
    }
    if let Some(tags) = &env.tags {
        if tags.len() > MAX_TAGS {
            return Err(EnvelopeError::TooManyTags);
        }
        for t in tags {
            if t.len() > MAX_TAG_LEN {
                return Err(EnvelopeError::TagTooLong(t.clone()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> DocumentEnvelope {
        DocumentEnvelope {
            schema: ENVELOPE_SCHEMA.into(),
            cid: "bafy0001".into(),
            title: "Title".into(),
            description: "Description".into(),
            content_type: "application/pdf".into(),
            size_bytes: 1234,
            timestamp: 1_700_000_000_000,
            tags: None,
        }
    }

    #[test]
    fn canonical_matches_typescript_layout() {
        let bytes = canonicalize(&base()).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        // Field order is the contract — assert the exact prefix.
        assert!(s.starts_with(r#"{"schema":"whistleblower/v1","cid":"bafy0001""#));
        // Tags omitted when None.
        assert!(!s.contains("tags"));
    }

    #[test]
    fn tags_included_only_when_non_empty() {
        let mut e = base();
        e.tags = Some(vec![]);
        let s1 = canonicalize(&e).unwrap();
        e.tags = None;
        let s2 = canonicalize(&e).unwrap();
        assert_eq!(s1, s2);

        e.tags = Some(vec!["leak".into(), "finance".into()]);
        let s3 = canonicalize(&e).unwrap();
        let s3 = std::str::from_utf8(&s3).unwrap();
        assert!(s3.contains(r#","tags":["leak","finance"]"#));
    }

    #[test]
    fn validation_rejects_oversized_fields() {
        let mut e = base();
        e.title = "x".repeat(300);
        assert!(matches!(validate(&e), Err(EnvelopeError::TitleTooLong)));
    }
}
