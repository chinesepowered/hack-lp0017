//! C ABI surface for native callers (Logos Basecamp C++/QML apps).
//!
//! Mirrors the pattern from `logos-execution-zone/wallet-ffi`: a small,
//! stable C surface that owns its allocations and exposes them as
//! null-terminated UTF-8 strings or length-tagged byte buffers.
//!
//! Build:
//!
//! ```bash
//! cargo build -p whistleblower-core --features ffi --release
//! ```
//!
//! The resulting `libwhistleblower_core.{so,dylib,dll}` plus the generated
//! `whistleblower_core.h` (see `cbindgen.toml`) are what the Basecamp app
//! links against.

use std::ffi::{c_char, CStr, CString};
use std::ptr;

use crate::envelope::{validate, DocumentEnvelope, ENVELOPE_SCHEMA};
use crate::hash::{metadata_hash, to_hex};

/// Result of `wb_envelope_hash_hex`. The caller must free `out` with
/// `wb_string_free` regardless of `ok`.
#[repr(C)]
pub struct WbStringResult {
    pub ok: bool,
    pub out: *mut c_char,
}

/// Compute the canonical SHA-256 of an envelope and return it as a
/// lowercase hex string. Inputs are UTF-8 nul-terminated strings; missing
/// fields can be passed as empty strings (validation will reject them).
///
/// # Safety
///
/// All pointer arguments must be valid nul-terminated UTF-8 strings.
/// `tags_json` is a JSON array of strings, e.g. `["leak","finance"]`, or
/// the empty string for "no tags".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wb_envelope_hash_hex(
    cid: *const c_char,
    title: *const c_char,
    description: *const c_char,
    content_type: *const c_char,
    size_bytes: u64,
    timestamp_ms: u64,
    tags_json: *const c_char,
) -> WbStringResult {
    let env = match build_envelope(
        cid,
        title,
        description,
        content_type,
        size_bytes,
        timestamp_ms,
        tags_json,
    ) {
        Ok(e) => e,
        Err(s) => return error_result(&s),
    };
    if let Err(e) = validate(&env) {
        return error_result(&format!("invalid envelope: {e}"));
    }
    match metadata_hash(&env) {
        Ok(h) => {
            let hex = to_hex(&h);
            match CString::new(hex) {
                Ok(c) => WbStringResult {
                    ok: true,
                    out: c.into_raw(),
                },
                Err(_) => error_result("internal nul in hex output"),
            }
        }
        Err(e) => error_result(&format!("hash failed: {e}")),
    }
}

/// Free a string returned by this library.
///
/// # Safety
///
/// `s` must be a pointer previously returned by this library in a
/// `WbStringResult::out` field, or null (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wb_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: caller pinky-promises `s` came from a `CString::into_raw`
        // call within this crate.
        let _ = unsafe { CString::from_raw(s) };
    }
}

/// Returns the canonical Logos Delivery topic string. Caller must NOT free.
#[unsafe(no_mangle)]
pub extern "C" fn wb_default_topic() -> *const c_char {
    static TOPIC: &[u8] = b"/logos/whistleblower/v1/documents\0";
    TOPIC.as_ptr() as *const c_char
}

fn build_envelope(
    cid: *const c_char,
    title: *const c_char,
    description: *const c_char,
    content_type: *const c_char,
    size_bytes: u64,
    timestamp_ms: u64,
    tags_json: *const c_char,
) -> Result<DocumentEnvelope, String> {
    let cid = unsafe { cstr_to_string(cid) }?;
    let title = unsafe { cstr_to_string(title) }?;
    let description = unsafe { cstr_to_string(description) }?;
    let content_type = unsafe { cstr_to_string(content_type) }?;
    let tags = if tags_json.is_null() {
        None
    } else {
        let s = unsafe { cstr_to_string(tags_json) }?;
        if s.is_empty() {
            None
        } else {
            Some(
                serde_json::from_str::<Vec<String>>(&s)
                    .map_err(|e| format!("tags_json parse: {e}"))?,
            )
        }
    };
    Ok(DocumentEnvelope {
        schema: ENVELOPE_SCHEMA.into(),
        cid,
        title,
        description,
        content_type,
        size_bytes,
        timestamp: timestamp_ms,
        tags,
    })
}

unsafe fn cstr_to_string(p: *const c_char) -> Result<String, String> {
    if p.is_null() {
        return Ok(String::new());
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| format!("invalid UTF-8: {e}"))
}

fn error_result(msg: &str) -> WbStringResult {
    let c = CString::new(msg).unwrap_or_else(|_| CString::new("error").unwrap());
    WbStringResult {
        ok: false,
        out: c.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_hash_roundtrip() {
        let cid = CString::new("bafy0001").unwrap();
        let title = CString::new("t").unwrap();
        let desc = CString::new("d").unwrap();
        let ct = CString::new("text/plain").unwrap();
        let r = unsafe {
            wb_envelope_hash_hex(
                cid.as_ptr(),
                title.as_ptr(),
                desc.as_ptr(),
                ct.as_ptr(),
                1,
                1_700_000_000_000,
                ptr::null(),
            )
        };
        assert!(r.ok);
        let s = unsafe { CStr::from_ptr(r.out) }
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(s.len(), 64);
        unsafe { wb_string_free(r.out) };
    }
}
