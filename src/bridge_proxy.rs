//! Proxy bridges — Proxy ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting the reverse proxy to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Proxy → DB (access log) ────────────────────────────────────

/// Access log record for ALICE-DB.
pub struct ProxyDbRecord {
    /// Content hash (FNV-1a of upstream_count + bytes_forwarded + latency_us).
    pub content_hash: u64,
    /// Number of upstream targets configured.
    pub upstream_count: u16,
    /// Number of currently active connections.
    pub active_connections: u32,
    /// Total bytes forwarded in this session.
    pub bytes_forwarded: u64,
    /// Round-trip latency to upstream in microseconds.
    pub latency_us: u64,
    /// Circuit breaker state hash (FNV-1a of state label bytes).
    pub circuit_state_hash: u64,
}

/// Serialize proxy access data for ALICE-DB logging.
#[inline]
#[must_use]
pub fn proxy_to_db_record(
    upstream_count: u16,
    active_connections: u32,
    bytes_forwarded: u64,
    latency_us: u64,
    circuit_state: &[u8],
) -> ProxyDbRecord {
    let mut buf = [0u8; 2 + 4 + 8 + 8];
    buf[..2].copy_from_slice(&upstream_count.to_le_bytes());
    buf[2..6].copy_from_slice(&active_connections.to_le_bytes());
    buf[6..14].copy_from_slice(&bytes_forwarded.to_le_bytes());
    buf[14..22].copy_from_slice(&latency_us.to_le_bytes());
    ProxyDbRecord {
        content_hash: fnv1a(&buf),
        upstream_count,
        active_connections,
        bytes_forwarded,
        latency_us,
        circuit_state_hash: fnv1a(circuit_state),
    }
}

// ── Bridge 2: Proxy → Cache (response cache) ─────────────────────────────

/// Response cache entry for ALICE-Cache.
pub struct ProxyCacheEntry {
    /// Content hash (FNV-1a of bytes_forwarded + latency_us).
    pub content_hash: u64,
    /// Bytes forwarded (used as cache key component).
    pub bytes_forwarded: u64,
    /// Upstream latency in microseconds.
    pub latency_us: u64,
    /// Cache TTL in seconds (branchless: 30 if latency_us < 50_000, else 5).
    pub ttl_secs: u32,
    /// Number of active connections.
    pub active_connections: u32,
}

/// Build a response cache entry for ALICE-Cache.
///
/// `ttl_secs` is computed branchlessly: 30 when `latency_us < 50_000`, else 5.
#[inline]
#[must_use]
pub fn proxy_to_cache_entry(
    bytes_forwarded: u64,
    latency_us: u64,
    active_connections: u32,
) -> ProxyCacheEntry {
    let mut buf = [0u8; 8 + 8 + 4];
    buf[..8].copy_from_slice(&bytes_forwarded.to_le_bytes());
    buf[8..16].copy_from_slice(&latency_us.to_le_bytes());
    buf[16..20].copy_from_slice(&active_connections.to_le_bytes());
    // ブランチレスTTL: レイテンシが低い場合は30秒、高い場合は5秒
    let is_low_latency = (latency_us < 50_000) as u32;
    let ttl_secs = 5 + is_low_latency * 25;
    ProxyCacheEntry {
        content_hash: fnv1a(&buf),
        bytes_forwarded,
        latency_us,
        ttl_secs,
        active_connections,
    }
}

// ── Bridge 3: Proxy → Analytics (traffic metrics) ────────────────────────

/// Traffic metrics for ALICE-Analytics.
pub struct ProxyAnalyticsMetrics {
    /// Content hash (FNV-1a of all traffic fields).
    pub content_hash: u64,
    /// Number of upstream targets.
    pub upstream_count: u16,
    /// Number of active connections.
    pub active_connections: u32,
    /// Total bytes forwarded.
    pub bytes_forwarded: u64,
    /// Round-trip latency in microseconds.
    pub latency_us: u64,
    /// Circuit breaker state hash.
    pub circuit_state_hash: u64,
    /// Number of failed upstream requests in this window.
    pub error_count: u32,
}

/// Extract traffic metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn proxy_to_analytics_metrics(
    upstream_count: u16,
    active_connections: u32,
    bytes_forwarded: u64,
    latency_us: u64,
    circuit_state: &[u8],
    error_count: u32,
) -> ProxyAnalyticsMetrics {
    let mut buf = [0u8; 2 + 4 + 8 + 8 + 4];
    buf[..2].copy_from_slice(&upstream_count.to_le_bytes());
    buf[2..6].copy_from_slice(&active_connections.to_le_bytes());
    buf[6..14].copy_from_slice(&bytes_forwarded.to_le_bytes());
    buf[14..22].copy_from_slice(&latency_us.to_le_bytes());
    buf[22..26].copy_from_slice(&error_count.to_le_bytes());
    ProxyAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        upstream_count,
        active_connections,
        bytes_forwarded,
        latency_us,
        circuit_state_hash: fnv1a(circuit_state),
        error_count,
    }
}

// ── Bridge 4: Proxy → Monitor (health) ───────────────────────────────────

/// Proxy health snapshot for ALICE-Monitor.
pub struct ProxyMonitorHealth {
    /// Content hash (FNV-1a of active_connections + latency_us + error_count).
    pub content_hash: u64,
    /// Number of active connections.
    pub active_connections: u32,
    /// Current upstream latency in microseconds.
    pub latency_us: u64,
    /// Number of upstream errors in the current window.
    pub error_count: u32,
    /// Number of configured upstreams.
    pub upstream_count: u16,
    /// Circuit breaker state hash.
    pub circuit_state_hash: u64,
}

/// Build a proxy health snapshot for ALICE-Monitor.
#[inline]
#[must_use]
pub fn proxy_to_monitor_health(
    active_connections: u32,
    latency_us: u64,
    error_count: u32,
    upstream_count: u16,
    circuit_state: &[u8],
) -> ProxyMonitorHealth {
    let mut buf = [0u8; 4 + 8 + 4 + 2];
    buf[..4].copy_from_slice(&active_connections.to_le_bytes());
    buf[4..12].copy_from_slice(&latency_us.to_le_bytes());
    buf[12..16].copy_from_slice(&error_count.to_le_bytes());
    buf[16..18].copy_from_slice(&upstream_count.to_le_bytes());
    ProxyMonitorHealth {
        content_hash: fnv1a(&buf),
        active_connections,
        latency_us,
        error_count,
        upstream_count,
        circuit_state_hash: fnv1a(circuit_state),
    }
}

// ── Bridge 5: Proxy → API (config) ───────────────────────────────────────

/// Proxy configuration descriptor for ALICE-API.
pub struct ProxyApiConfig {
    /// Content hash (FNV-1a of upstream_count + active_connections + bytes_forwarded).
    pub content_hash: u64,
    /// Number of upstream targets.
    pub upstream_count: u16,
    /// Number of active connections at config export time.
    pub active_connections: u32,
    /// Total bytes forwarded since last config reload.
    pub bytes_forwarded: u64,
    /// Circuit breaker state hash.
    pub circuit_state_hash: u64,
    /// Upstream latency in microseconds.
    pub latency_us: u64,
}

/// Build a proxy configuration descriptor for ALICE-API.
#[inline]
#[must_use]
pub fn proxy_to_api_config(
    upstream_count: u16,
    active_connections: u32,
    bytes_forwarded: u64,
    circuit_state: &[u8],
    latency_us: u64,
) -> ProxyApiConfig {
    let mut buf = [0u8; 2 + 4 + 8 + 8];
    buf[..2].copy_from_slice(&upstream_count.to_le_bytes());
    buf[2..6].copy_from_slice(&active_connections.to_le_bytes());
    buf[6..14].copy_from_slice(&bytes_forwarded.to_le_bytes());
    buf[14..22].copy_from_slice(&latency_us.to_le_bytes());
    ProxyApiConfig {
        content_hash: fnv1a(&buf),
        upstream_count,
        active_connections,
        bytes_forwarded,
        circuit_state_hash: fnv1a(circuit_state),
        latency_us,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CIRCUIT_CLOSED: &[u8] = b"closed";
    const CIRCUIT_OPEN: &[u8] = b"open";

    #[test]
    fn test_proxy_to_db_record_basic() {
        let rec = proxy_to_db_record(4, 128, 1_048_576, 12_000, CIRCUIT_CLOSED);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.upstream_count, 4);
        assert_eq!(rec.active_connections, 128);
        assert_eq!(rec.bytes_forwarded, 1_048_576);
        assert_eq!(rec.latency_us, 12_000);
        assert_ne!(rec.circuit_state_hash, 0);
    }

    #[test]
    fn test_proxy_to_db_record_determinism() {
        let r1 = proxy_to_db_record(2, 64, 512_000, 8_000, CIRCUIT_CLOSED);
        let r2 = proxy_to_db_record(2, 64, 512_000, 8_000, CIRCUIT_CLOSED);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_proxy_to_cache_entry_low_latency_ttl() {
        // latency_us < 50_000 → ttl_secs = 30
        let e = proxy_to_cache_entry(256_000, 10_000, 50);
        assert_eq!(e.ttl_secs, 30);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_proxy_to_cache_entry_high_latency_ttl() {
        // latency_us >= 50_000 → ttl_secs = 5
        let e = proxy_to_cache_entry(256_000, 100_000, 50);
        assert_eq!(e.ttl_secs, 5);
    }

    #[test]
    fn test_proxy_to_analytics_metrics_basic() {
        let m = proxy_to_analytics_metrics(4, 100, 2_097_152, 15_000, CIRCUIT_CLOSED, 2);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.upstream_count, 4);
        assert_eq!(m.error_count, 2);
    }

    #[test]
    fn test_proxy_to_monitor_health_basic() {
        let h = proxy_to_monitor_health(80, 9_000, 0, 3, CIRCUIT_CLOSED);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.active_connections, 80);
        assert_eq!(h.error_count, 0);
    }

    #[test]
    fn test_proxy_circuit_state_hash_differs() {
        // 異なるサーキット状態は異なるハッシュを生成する
        let h_closed = proxy_to_monitor_health(80, 9_000, 0, 3, CIRCUIT_CLOSED);
        let h_open = proxy_to_monitor_health(80, 9_000, 0, 3, CIRCUIT_OPEN);
        assert_ne!(h_closed.circuit_state_hash, h_open.circuit_state_hash);
    }

    #[test]
    fn test_proxy_to_api_config_basic() {
        let c = proxy_to_api_config(6, 200, 4_194_304, CIRCUIT_CLOSED, 8_000);
        assert_ne!(c.content_hash, 0);
        assert_eq!(c.upstream_count, 6);
        assert_eq!(c.active_connections, 200);
        assert_eq!(c.latency_us, 8_000);
    }
}
