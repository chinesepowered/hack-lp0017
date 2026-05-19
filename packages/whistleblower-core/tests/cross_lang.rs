//! Cross-language fixture test.
//!
//! Pins the canonical envelope bytes + metadata_hash for a known input so
//! that any divergence between the Rust and TypeScript ports surfaces in
//! CI rather than at runtime (where it would manifest as on-chain
//! mismatch and silent loss of indexing).

use whistleblower_core::{canonicalize, metadata_hash, DocumentEnvelope, ENVELOPE_SCHEMA};

#[test]
fn fixture_a_matches_pinned_hash() {
    let env = DocumentEnvelope {
        schema: ENVELOPE_SCHEMA.into(),
        cid: "bafy0001".into(),
        title: "Internal memo".into(),
        description: "Q3 forecast revisions".into(),
        content_type: "application/pdf".into(),
        size_bytes: 1234,
        timestamp: 1_700_000_000_000,
        tags: Some(vec!["leak".into(), "finance".into()]),
    };
    let bytes = canonicalize(&env).unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();

    // Pinned canonical form. Any change here MUST be replicated to the
    // TypeScript port (`packages/indexing-module/test/envelope.test.ts`)
    // before being merged.
    assert_eq!(
        s,
        r#"{"schema":"whistleblower/v1","cid":"bafy0001","title":"Internal memo","description":"Q3 forecast revisions","content_type":"application/pdf","size_bytes":1234,"timestamp":1700000000000,"tags":["leak","finance"]}"#
    );

    let hash = metadata_hash(&env).unwrap();
    let hex = hex::encode(hash);
    // Pinned: matches the TypeScript port's output for the same envelope.
    // If you change envelope canonicalisation, update BOTH this assertion
    // AND the equivalent one in `packages/indexing-module/test/cross_lang.test.ts`.
    assert_eq!(
        hex,
        "fce3429b051749a9a401d054bd23efcbb04288a47347f008417c025bcf545d3c"
    );
}
