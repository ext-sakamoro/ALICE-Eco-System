//! gRPC bridges — gRPC ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting the gRPC layer to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: gRPC → DB (call log) ───────────────────────────────────────

/// gRPC call log record for ALICE-DB.
pub struct GrpcDbRecord {
    /// Content hash (FNV-1a of method_hash + request_bytes + response_bytes + status_code).
    pub content_hash: u64,
    /// Method hash (FNV-1a of the fully-qualified method name bytes).
    pub method_hash: u64,
    /// Request payload size in bytes.
    pub request_bytes: u64,
    /// Response payload size in bytes.
    pub response_bytes: u64,
    /// gRPC status code (0 = OK).
    pub status_code: u32,
    /// Number of metadata entries.
    pub metadata_count: u16,
}

/// Serialize a gRPC call for ALICE-DB logging.
#[inline]
#[must_use]
pub fn grpc_to_db_record(
    method: &[u8],
    request_bytes: u64,
    response_bytes: u64,
    status_code: u32,
    metadata_count: u16,
) -> GrpcDbRecord {
    let method_hash = fnv1a(method);
    let mut buf = [0u8; 8 + 8 + 8 + 4 + 2];
    buf[..8].copy_from_slice(&method_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&request_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&response_bytes.to_le_bytes());
    buf[24..28].copy_from_slice(&status_code.to_le_bytes());
    buf[28..30].copy_from_slice(&metadata_count.to_le_bytes());
    GrpcDbRecord {
        content_hash: fnv1a(&buf),
        method_hash,
        request_bytes,
        response_bytes,
        status_code,
        metadata_count,
    }
}

// ── Bridge 2: gRPC → Cache (response cache) ──────────────────────────────

/// Response cache entry for ALICE-Cache.
pub struct GrpcCacheEntry {
    /// Content hash (FNV-1a of method_hash + request_bytes).
    pub content_hash: u64,
    /// Method hash (cache key component).
    pub method_hash: u64,
    /// Request bytes (cache key component).
    pub request_bytes: u64,
    /// Cache TTL in seconds (branchless: 60 if status_code == 0, else 0).
    pub ttl_secs: u32,
    /// gRPC status code.
    pub status_code: u32,
}

/// Build a response cache entry for ALICE-Cache.
///
/// `ttl_secs` is computed branchlessly: 60 when `status_code == 0` (OK), else 0.
#[inline]
#[must_use]
pub fn grpc_to_cache_entry(method: &[u8], request_bytes: u64, status_code: u32) -> GrpcCacheEntry {
    let method_hash = fnv1a(method);
    let mut buf = [0u8; 8 + 8 + 4];
    buf[..8].copy_from_slice(&method_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&request_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&status_code.to_le_bytes());
    // ブランチレスTTL: ステータスOKなら60秒、エラーなら0秒
    let is_ok = (status_code == 0) as u32;
    let ttl_secs = is_ok * 60;
    GrpcCacheEntry {
        content_hash: fnv1a(&buf),
        method_hash,
        request_bytes,
        ttl_secs,
        status_code,
    }
}

// ── Bridge 3: gRPC → Analytics (RPC metrics) ─────────────────────────────

/// RPC metrics for ALICE-Analytics.
pub struct GrpcAnalyticsMetrics {
    /// Content hash (FNV-1a of all RPC metric fields).
    pub content_hash: u64,
    /// Method hash.
    pub method_hash: u64,
    /// Request payload size in bytes.
    pub request_bytes: u64,
    /// Response payload size in bytes.
    pub response_bytes: u64,
    /// gRPC status code.
    pub status_code: u32,
    /// Number of metadata entries.
    pub metadata_count: u16,
    /// Call duration in microseconds.
    pub duration_us: u64,
}

/// Extract RPC metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn grpc_to_analytics_metrics(
    method: &[u8],
    request_bytes: u64,
    response_bytes: u64,
    status_code: u32,
    metadata_count: u16,
    duration_us: u64,
) -> GrpcAnalyticsMetrics {
    let method_hash = fnv1a(method);
    let mut buf = [0u8; 8 + 8 + 8 + 4 + 2 + 8];
    buf[..8].copy_from_slice(&method_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&request_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&response_bytes.to_le_bytes());
    buf[24..28].copy_from_slice(&status_code.to_le_bytes());
    buf[28..30].copy_from_slice(&metadata_count.to_le_bytes());
    buf[30..38].copy_from_slice(&duration_us.to_le_bytes());
    GrpcAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        method_hash,
        request_bytes,
        response_bytes,
        status_code,
        metadata_count,
        duration_us,
    }
}

// ── Bridge 4: gRPC → Monitor (health) ────────────────────────────────────

/// gRPC server health snapshot for ALICE-Monitor.
pub struct GrpcMonitorHealth {
    /// Content hash (FNV-1a of request_bytes + response_bytes + status_code + duration_us).
    pub content_hash: u64,
    /// Total request bytes in the current window.
    pub request_bytes: u64,
    /// Total response bytes in the current window.
    pub response_bytes: u64,
    /// gRPC status code of the last call.
    pub status_code: u32,
    /// Number of metadata entries on the last call.
    pub metadata_count: u16,
    /// Last call duration in microseconds.
    pub duration_us: u64,
}

/// Build a gRPC server health snapshot for ALICE-Monitor.
#[inline]
#[must_use]
pub fn grpc_to_monitor_health(
    request_bytes: u64,
    response_bytes: u64,
    status_code: u32,
    metadata_count: u16,
    duration_us: u64,
) -> GrpcMonitorHealth {
    let mut buf = [0u8; 8 + 8 + 4 + 2 + 8];
    buf[..8].copy_from_slice(&request_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&response_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&status_code.to_le_bytes());
    buf[20..22].copy_from_slice(&metadata_count.to_le_bytes());
    buf[22..30].copy_from_slice(&duration_us.to_le_bytes());
    GrpcMonitorHealth {
        content_hash: fnv1a(&buf),
        request_bytes,
        response_bytes,
        status_code,
        metadata_count,
        duration_us,
    }
}

// ── Bridge 5: gRPC → API (service registry) ──────────────────────────────

/// Service registry entry for ALICE-API.
pub struct GrpcApiRegistryEntry {
    /// Content hash (FNV-1a of method_hash + metadata_count + status_code).
    pub content_hash: u64,
    /// Method hash (FNV-1a of fully-qualified method name).
    pub method_hash: u64,
    /// Number of metadata entries (headers + trailers).
    pub metadata_count: u16,
    /// gRPC status code.
    pub status_code: u32,
    /// Request payload size in bytes.
    pub request_bytes: u64,
    /// Response payload size in bytes.
    pub response_bytes: u64,
}

/// Build a service registry entry for ALICE-API.
#[inline]
#[must_use]
pub fn grpc_to_api_registry_entry(
    method: &[u8],
    metadata_count: u16,
    status_code: u32,
    request_bytes: u64,
    response_bytes: u64,
) -> GrpcApiRegistryEntry {
    let method_hash = fnv1a(method);
    let mut buf = [0u8; 8 + 2 + 4 + 8 + 8];
    buf[..8].copy_from_slice(&method_hash.to_le_bytes());
    buf[8..10].copy_from_slice(&metadata_count.to_le_bytes());
    buf[10..14].copy_from_slice(&status_code.to_le_bytes());
    buf[14..22].copy_from_slice(&request_bytes.to_le_bytes());
    buf[22..30].copy_from_slice(&response_bytes.to_le_bytes());
    GrpcApiRegistryEntry {
        content_hash: fnv1a(&buf),
        method_hash,
        metadata_count,
        status_code,
        request_bytes,
        response_bytes,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const METHOD: &[u8] = b"/alice.Service/Execute";

    #[test]
    fn test_grpc_to_db_record_basic() {
        let rec = grpc_to_db_record(METHOD, 1024, 2048, 0, 4);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.method_hash, 0);
        assert_eq!(rec.request_bytes, 1024);
        assert_eq!(rec.response_bytes, 2048);
        assert_eq!(rec.status_code, 0);
        assert_eq!(rec.metadata_count, 4);
    }

    #[test]
    fn test_grpc_to_db_record_determinism() {
        let r1 = grpc_to_db_record(METHOD, 512, 1024, 0, 2);
        let r2 = grpc_to_db_record(METHOD, 512, 1024, 0, 2);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_grpc_to_cache_entry_ok_ttl() {
        // status_code == 0 (OK) → ttl_secs = 60
        let e = grpc_to_cache_entry(METHOD, 512, 0);
        assert_eq!(e.ttl_secs, 60);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_grpc_to_cache_entry_error_ttl() {
        // status_code != 0 → ttl_secs = 0
        let e = grpc_to_cache_entry(METHOD, 512, 14); // 14 = UNAVAILABLE
        assert_eq!(e.ttl_secs, 0);
    }

    #[test]
    fn test_grpc_to_analytics_metrics_basic() {
        let m = grpc_to_analytics_metrics(METHOD, 1024, 2048, 0, 3, 5_500);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.duration_us, 5_500);
        assert_eq!(m.metadata_count, 3);
    }

    #[test]
    fn test_grpc_to_monitor_health_basic() {
        let h = grpc_to_monitor_health(4096, 8192, 0, 5, 12_000);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.status_code, 0);
        assert_eq!(h.duration_us, 12_000);
    }

    #[test]
    fn test_grpc_to_api_registry_entry_basic() {
        let e = grpc_to_api_registry_entry(METHOD, 6, 0, 2048, 4096);
        assert_ne!(e.content_hash, 0);
        assert_ne!(e.method_hash, 0);
        assert_eq!(e.metadata_count, 6);
        assert_eq!(e.status_code, 0);
    }

    #[test]
    fn test_grpc_method_hash_differs() {
        // 異なるメソッド名は異なるハッシュを生成する
        let r1 = grpc_to_db_record(b"/alice.Service/Execute", 512, 1024, 0, 2);
        let r2 = grpc_to_db_record(b"/alice.Service/Query", 512, 1024, 0, 2);
        assert_ne!(r1.method_hash, r2.method_hash);
    }
}
