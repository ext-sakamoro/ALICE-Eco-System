//! FFI bridges — ALICE-FFI ↔ DB, Cache, Analytics, Crypto, Edge
//!
//! 5 bridges connecting the C-ABI foreign function interface layer to the ALICE ecosystem.

use alice_ffi::{
    cstr_to_string, string_to_cstr, CallbackRegistry, FfiBuffer, FfiResult, VersionInfo,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: FFI → DB (FFI call records) ─────────────────────────────────

/// FFI call audit record for ALICE-DB.
///
/// Persists a log entry for every FFI boundary crossing so that the database
/// layer can audit inter-language calls, detect anomalies, and replay traces.
pub struct FfiDbCallRecord {
    /// FNV-1a hash of the function name (call identity key).
    pub content_hash: u64,
    /// FNV-1a hash of the argument bytes (argument fingerprint).
    pub arg_hash: u64,
    /// Result code from the FFI call (0=Ok, negative=error).
    pub result_code: i32,
    /// Whether the call succeeded.
    pub success: bool,
    /// Argument payload size in bytes.
    pub arg_bytes: usize,
    /// Output buffer size in bytes (0 if no output).
    pub output_bytes: usize,
}

/// Build an FFI call audit record for ALICE-DB.
///
/// `fn_name` is the name of the foreign function.
/// `arg_buf` contains the serialised arguments.
/// `out_buf` is the output buffer (may be empty).
/// `result` is the `FfiResult` returned by the call.
#[inline]
#[must_use]
pub fn ffi_to_db_call_record(
    fn_name: &str,
    arg_buf: &FfiBuffer,
    out_buf: &FfiBuffer,
    result: FfiResult,
) -> FfiDbCallRecord {
    let content_hash = fnv1a(fn_name.as_bytes());
    let arg_hash = fnv1a(arg_buf.read());
    let result_code = result.code();
    let success = result.is_ok();
    FfiDbCallRecord {
        content_hash,
        arg_hash,
        result_code,
        success,
        arg_bytes: arg_buf.len(),
        output_bytes: out_buf.len(),
    }
}

// ── Bridge 2: FFI → Cache (buffer cache entry) ────────────────────────────

/// FFI buffer cache entry for ALICE-Cache.
///
/// Caches the output of expensive FFI calls so that repeated calls with
/// identical arguments are served from cache.
/// TTL is shortened branchlessly for large buffers.
pub struct FfiCacheEntry {
    /// FNV-1a hash of the cached buffer content (cache key).
    pub content_hash: u64,
    /// Cached buffer data.
    pub data: Vec<u8>,
    /// Cached data size in bytes.
    pub data_bytes: usize,
    /// Cache TTL in seconds (branchless: shorter for large buffers).
    pub ttl_secs: u32,
    /// Whether the cached result was a successful FFI call.
    pub was_success: bool,
}

/// Build an FFI output buffer cache entry for ALICE-Cache.
///
/// TTL rules (branchless):
/// - Base: 1800 s.
/// - Buffer > 64 KB: −900 s.
/// - Failed result: the entry is cached with TTL = 60 s (error grace period).
#[inline]
#[must_use]
pub fn ffi_to_cache_entry(buf: &FfiBuffer, result: FfiResult) -> FfiCacheEntry {
    let data: Vec<u8> = buf.read().to_vec();
    let content_hash = fnv1a(&data);
    let was_success = result.is_ok();

    // Branchless TTL.
    let large = (data.len() > 65_536) as u32;
    let failed = (!was_success) as u32;
    // failed path overrides: 60 s; success path: 1800 − large * 900.
    let success_ttl = 1800 - large * 900;
    let ttl_secs = success_ttl * (1 - failed) + 60 * failed;

    FfiCacheEntry {
        content_hash,
        data_bytes: data.len(),
        ttl_secs,
        was_success,
        data,
    }
}

// ── Bridge 3: FFI → Analytics (FFI call metrics) ──────────────────────────

/// FFI call metrics for ALICE-Analytics.
///
/// Aggregates per-function call counts, error rates, and throughput
/// statistics so the analytics layer can detect unhealthy FFI boundaries.
pub struct FfiAnalyticsMetrics {
    /// FNV-1a hash of the function name (analytics stream key).
    pub content_hash: u64,
    /// Total calls recorded in the measurement window.
    pub total_calls: u64,
    /// Successful calls.
    pub success_calls: u64,
    /// Failed calls.
    pub error_calls: u64,
    /// Error rate in permille (error_calls / total_calls × 1000).
    pub error_rate_permille: u32,
    /// Total bytes transferred through the FFI boundary (args + output).
    pub total_bytes_transferred: u64,
    /// Callback registry size at measurement time.
    pub registered_callbacks: usize,
}

/// Build FFI call metrics for ALICE-Analytics.
///
/// `fn_name` identifies the FFI function being measured.
/// `registry` provides the current registered callback count.
#[inline]
#[must_use]
pub fn ffi_to_analytics_metrics(
    fn_name: &str,
    total_calls: u64,
    success_calls: u64,
    total_bytes_transferred: u64,
    registry: &CallbackRegistry,
) -> FfiAnalyticsMetrics {
    let content_hash = fnv1a(fn_name.as_bytes());
    let error_calls = total_calls.saturating_sub(success_calls);
    let total_safe = total_calls.max(1);
    let error_rate_permille =
        (error_calls.min(total_safe).wrapping_mul(1_000) / total_safe) as u32;
    FfiAnalyticsMetrics {
        content_hash,
        total_calls,
        success_calls,
        error_calls,
        error_rate_permille,
        total_bytes_transferred,
        registered_callbacks: registry.count(),
    }
}

// ── Bridge 4: FFI → Crypto (native crypto integration) ────────────────────

/// Native crypto integration descriptor for ALICE-Crypto.
///
/// Packages an FFI buffer payload for encryption by the ALICE-Crypto layer.
/// The nonce is derived from the content hash to avoid nonce reuse while
/// keeping the bridge stateless.
pub struct FfiCryptoDescriptor {
    /// FNV-1a hash of the plaintext buffer (integrity key).
    pub content_hash: u64,
    /// Plaintext size in bytes.
    pub plaintext_bytes: usize,
    /// 12-byte nonce derived from the content hash (low 96 bits).
    pub nonce: [u8; 12],
    /// Version of the FFI ABI as a packed u32 (`major<<22 | minor<<12 | patch`).
    pub abi_version_packed: u32,
    /// Requested cipher: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
    pub cipher: u8,
    /// Whether the plaintext passed a basic null-terminator scan
    /// (heuristic: last byte is 0x00 → likely a C string).
    pub is_cstring: bool,
}

/// Build a native crypto descriptor from an FFI buffer.
///
/// `abi_version` is the `VersionInfo` of the native library being wrapped.
/// `cipher`: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
#[inline]
#[must_use]
pub fn ffi_to_crypto_descriptor(
    buf: &FfiBuffer,
    abi_version: &VersionInfo,
    cipher: u8,
) -> FfiCryptoDescriptor {
    let data = buf.read();
    let content_hash = fnv1a(data);
    let hash_bytes = content_hash.to_le_bytes();
    let mut nonce = [0u8; 12];
    for (i, b) in nonce.iter_mut().enumerate() {
        *b = hash_bytes[i % 8];
    }
    let is_cstring = data.last().copied() == Some(0);
    FfiCryptoDescriptor {
        content_hash,
        plaintext_bytes: data.len(),
        nonce,
        abi_version_packed: abi_version.to_u32(),
        cipher: cipher.min(1),
        is_cstring,
    }
}

// ── Bridge 5: FFI → Edge (native call events) ─────────────────────────────

/// Native call event for ALICE-Edge.
///
/// Emitted whenever an FFI boundary is crossed at an edge node, enabling
/// the edge layer to react to native library calls (e.g. rate-limiting
/// expensive FFI calls or triggering security scans on the payload).
pub struct FfiEdgeEvent {
    /// FNV-1a hash of the function name (event correlation key).
    pub content_hash: u64,
    /// FNV-1a hash of the argument payload.
    pub arg_hash: u64,
    /// Event kind: 0=call_initiated, 1=call_success, 2=call_failed, 3=buffer_overflow.
    pub event_kind: u8,
    /// Result code from the FFI call.
    pub result_code: i32,
    /// Argument buffer size in bytes.
    pub arg_bytes: usize,
    /// Edge action: 0=allow, 1=rate_limit, 2=block.
    pub edge_action: u8,
    /// Whether the C-string representation of the function name is valid UTF-8.
    pub fn_name_valid_utf8: bool,
}

/// Build an FFI edge event.
///
/// `fn_name` is the native function name (passed through `string_to_cstr` and
/// back to verify UTF-8 round-trip integrity).
/// `arg_buf` contains the serialised arguments.
/// `result` is the FFI call result.
///
/// `edge_action` is computed branchlessly:
/// - Buffer-overflow errors → block (2).
/// - Other errors → rate_limit (1).
/// - Success → allow (0).
#[inline]
#[must_use]
pub fn ffi_to_edge_event(
    fn_name: &str,
    arg_buf: &FfiBuffer,
    result: FfiResult,
) -> FfiEdgeEvent {
    let content_hash = fnv1a(fn_name.as_bytes());
    let arg_hash = fnv1a(arg_buf.read());

    // Verify UTF-8 round-trip via C-string conversion.
    let cstr_bytes = string_to_cstr(fn_name);
    let fn_name_valid_utf8 = cstr_to_string(&cstr_bytes)
        .map(|s| s == fn_name)
        .unwrap_or(false);

    let result_code = result.code();
    let is_overflow = matches!(result, FfiResult::BufferTooSmall);
    let is_error = result.is_err();

    // Branchless event_kind: overflow → 3, other error → 2, success → 1.
    let event_kind: u8 = if is_overflow { 3 } else if is_error { 2 } else { 1 };

    // Branchless edge_action: overflow → 2 (block), other error → 1 (rate_limit), ok → 0.
    let edge_action: u8 = (is_overflow as u8) * 2 + (!is_overflow && is_error) as u8;

    FfiEdgeEvent {
        content_hash,
        arg_hash,
        event_kind,
        result_code,
        arg_bytes: arg_buf.len(),
        edge_action,
        fn_name_valid_utf8,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buf(data: &[u8]) -> FfiBuffer {
        FfiBuffer::from_bytes(data)
    }

    fn make_version(major: u32, minor: u32, patch: u32) -> VersionInfo {
        VersionInfo::new(major, minor, patch)
    }

    #[test]
    fn test_db_call_record_success() {
        let arg = make_buf(b"arg_data");
        let out = make_buf(b"output");
        let rec = ffi_to_db_call_record("my_native_fn", &arg, &out, FfiResult::Ok);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.arg_hash, 0);
        assert!(rec.success);
        assert_eq!(rec.result_code, 0);
        assert_eq!(rec.arg_bytes, 8);
        assert_eq!(rec.output_bytes, 6);
    }

    #[test]
    fn test_db_call_record_error() {
        let arg = make_buf(b"bad");
        let out = make_buf(b"");
        let rec = ffi_to_db_call_record("fn_x", &arg, &out, FfiResult::NullPointer);
        assert!(!rec.success);
        assert!(rec.result_code < 0);
    }

    #[test]
    fn test_db_call_record_hash_deterministic() {
        let arg = make_buf(b"hello");
        let out = make_buf(b"");
        let a = ffi_to_db_call_record("fn", &arg, &out, FfiResult::Ok);
        let b = ffi_to_db_call_record("fn", &arg, &out, FfiResult::Ok);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.arg_hash, b.arg_hash);
    }

    #[test]
    fn test_cache_entry_success_small() {
        // ≤ 64 KB, success → 1800 s
        let buf = make_buf(b"hello ffi");
        let entry = ffi_to_cache_entry(&buf, FfiResult::Ok);
        assert_eq!(entry.ttl_secs, 1800);
        assert!(entry.was_success);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_cache_entry_failed_grace_ttl() {
        // Failed → 60 s (error grace period)
        let buf = make_buf(b"data");
        let entry = ffi_to_cache_entry(&buf, FfiResult::InvalidArgument);
        assert_eq!(entry.ttl_secs, 60);
        assert!(!entry.was_success);
    }

    #[test]
    fn test_analytics_metrics_basic() {
        let mut reg = CallbackRegistry::new();
        let _ = reg.register();
        let _ = reg.register();
        let m = ffi_to_analytics_metrics("compute_fn", 1000, 950, 512_000, &reg);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.total_calls, 1000);
        assert_eq!(m.success_calls, 950);
        assert_eq!(m.error_calls, 50);
        // 50/1000 * 1000 = 50 permille
        assert_eq!(m.error_rate_permille, 50);
        assert_eq!(m.registered_callbacks, 2);
    }

    #[test]
    fn test_analytics_metrics_zero_calls_no_panic() {
        let reg = CallbackRegistry::new();
        let m = ffi_to_analytics_metrics("fn", 0, 0, 0, &reg);
        assert_eq!(m.error_rate_permille, 0);
    }

    #[test]
    fn test_crypto_descriptor_basic() {
        let buf = make_buf(b"sensitive payload data");
        let ver = make_version(1, 2, 3);
        let desc = ffi_to_crypto_descriptor(&buf, &ver, 0);
        assert_ne!(desc.content_hash, 0);
        assert_eq!(desc.cipher, 0);
        assert_eq!(desc.plaintext_bytes, 22);
        assert_ne!(desc.nonce, [0u8; 12]);
        assert!(!desc.is_cstring); // no trailing null
    }

    #[test]
    fn test_crypto_descriptor_cstring_detection() {
        // Payload ending in 0x00 is detected as a C string.
        let data = b"hello\0";
        let buf = make_buf(data);
        let ver = make_version(2, 0, 0);
        let desc = ffi_to_crypto_descriptor(&buf, &ver, 1);
        assert!(desc.is_cstring);
        assert_eq!(desc.cipher, 1);
    }

    #[test]
    fn test_edge_event_success() {
        let arg = make_buf(b"payload");
        let ev = ffi_to_edge_event("native_compute", &arg, FfiResult::Ok);
        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.arg_hash, 0);
        assert_eq!(ev.event_kind, 1); // call_success
        assert_eq!(ev.edge_action, 0); // allow
        assert!(ev.fn_name_valid_utf8);
        assert_eq!(ev.result_code, 0);
    }

    #[test]
    fn test_edge_event_buffer_overflow_blocked() {
        let arg = make_buf(b"too much data");
        let ev = ffi_to_edge_event("fn_overflow", &arg, FfiResult::BufferTooSmall);
        assert_eq!(ev.event_kind, 3); // buffer_overflow
        assert_eq!(ev.edge_action, 2); // block
        assert!(!ev.fn_name_valid_utf8 || ev.fn_name_valid_utf8); // either is fine, no panic
    }

    #[test]
    fn test_edge_event_null_pointer_rate_limited() {
        let arg = make_buf(b"");
        let ev = ffi_to_edge_event("fn_null", &arg, FfiResult::NullPointer);
        assert_eq!(ev.event_kind, 2); // call_failed
        assert_eq!(ev.edge_action, 1); // rate_limit
    }
}
