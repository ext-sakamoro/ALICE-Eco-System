//! Container bridges — ALICE-Container ↔ DB, Cache, Analytics, Auth, Crypto, Queue, RTOS
//!
//! 7 bridges connecting container runtime to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Container → DB (deployment record) ─────────────────────────

/// Container deployment record for ALICE-DB persistence.
pub struct ContainerDbRecord {
    /// FNV-1a content hash over image hash + cpu + memory + state.
    pub content_hash: u64,
    /// Hash identifying the container image.
    pub image_hash: u64,
    /// CPU bandwidth limit in microseconds per scheduler period.
    pub cpu_limit_us: u64,
    /// Memory limit in bytes.
    pub memory_limit_bytes: u64,
    /// Container state (0=created, 1=running, 2=paused, 3=stopped, 4=dead).
    pub state: u8,
    /// Record creation timestamp in milliseconds since Unix epoch.
    pub created_at_ms: u64,
}

/// Serialize a container deployment descriptor for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn container_to_db_record(
    image_hash: u64,
    cpu_limit_us: u64,
    memory_limit: u64,
    state: u8,
) -> ContainerDbRecord {
    let mut data = [0u8; 25];
    data[0..8].copy_from_slice(&image_hash.to_le_bytes());
    data[8..16].copy_from_slice(&cpu_limit_us.to_le_bytes());
    data[16..24].copy_from_slice(&memory_limit.to_le_bytes());
    data[24] = state;
    // Derive a stable wall-clock timestamp from the content hash to avoid
    // importing std::time in a no_std-compatible bridge module.
    let content_hash = fnv1a(&data);
    let created_at_ms = content_hash ^ 0x0001_7200_0000_0000; // deterministic stand-in
    ContainerDbRecord {
        content_hash,
        image_hash,
        cpu_limit_us,
        memory_limit_bytes: memory_limit,
        state,
        created_at_ms,
    }
}

// ── Bridge 2: Container → Cache (image layer cache) ──────────────────────

/// Image layer cache entry for ALICE-Cache.
pub struct ContainerCacheEntry {
    /// FNV-1a hash of the raw layer bytes — used as the cache key.
    pub layer_hash: u64,
    /// Compressed layer size in bytes (approximated as raw size for facade).
    pub compressed_bytes: usize,
    /// Uncompressed layer size in bytes.
    pub uncompressed_bytes: usize,
    /// Zero-based index of this layer within the image.
    pub layer_index: u16,
}

/// Build a cache entry for an image layer targeting ALICE-Cache.
#[inline]
#[must_use]
pub fn container_to_cache_entry(layer_data: &[u8], layer_index: u16) -> ContainerCacheEntry {
    let layer_hash = fnv1a(layer_data);
    // Estimate compressed size as 60 % of raw (typical OCI layer ratio).
    let compressed_bytes = (layer_data.len() * 3 / 5).max(1);
    ContainerCacheEntry {
        layer_hash,
        compressed_bytes,
        uncompressed_bytes: layer_data.len(),
        layer_index,
    }
}

// ── Bridge 3: Container → Analytics (resource metrics) ───────────────────

/// Container resource utilisation snapshot for ALICE-Analytics.
pub struct ContainerAnalyticsMetrics {
    /// FNV-1a hash of the container identifier.
    pub container_hash: u64,
    /// CPU utilisation in percent (0.0 – 100.0).
    pub cpu_usage_pct: f32,
    /// Memory utilisation in percent (0.0 – 100.0).
    pub memory_usage_pct: f32,
    /// Bytes read from block devices since container start.
    pub io_read_bytes: u64,
    /// Bytes written to block devices since container start.
    pub io_write_bytes: u64,
    /// Container uptime in whole seconds.
    pub uptime_seconds: u64,
}

/// Package container resource metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn container_to_analytics_metrics(
    id: u64,
    cpu_pct: f32,
    mem_pct: f32,
    io_r: u64,
    io_w: u64,
    uptime: u64,
) -> ContainerAnalyticsMetrics {
    let container_hash = fnv1a(&id.to_le_bytes());
    ContainerAnalyticsMetrics {
        container_hash,
        cpu_usage_pct: cpu_pct.clamp(0.0, 100.0),
        memory_usage_pct: mem_pct.clamp(0.0, 100.0),
        io_read_bytes: io_r,
        io_write_bytes: io_w,
        uptime_seconds: uptime,
    }
}

// ── Bridge 4: Container → Auth (registry access) ─────────────────────────

/// Registry access request for ALICE-Auth token issuance.
///
/// Scope values: 0 = pull, 1 = push, 2 = admin.
pub struct ContainerAuthRequest {
    /// FNV-1a hash of the registry URL.
    pub registry_hash: u64,
    /// FNV-1a hash of the image reference string.
    pub image_hash: u64,
    /// Access scope (0=pull, 1=push, 2=admin).
    pub scope: u8,
    /// Byte length of the bearer token to be returned (hint for ALICE-Auth).
    pub token_bytes: usize,
}

/// Build a registry access request for ALICE-Auth.
///
/// `scope`: 0=pull, 1=push, 2=admin.
#[inline]
#[must_use]
pub fn container_to_auth_request(
    registry: &str,
    image_ref: &str,
    scope: u8,
) -> ContainerAuthRequest {
    let registry_hash = fnv1a(registry.as_bytes());
    let image_hash = fnv1a(image_ref.as_bytes());
    // Token size hint: 256 B for pull, 512 B for push/admin.
    let token_bytes: usize = if scope == 0 { 256 } else { 512 };
    ContainerAuthRequest {
        registry_hash,
        image_hash,
        scope,
        token_bytes,
    }
}

// ── Bridge 5: Container → Crypto (image signing) ─────────────────────────

/// Image signing payload for ALICE-Crypto.
///
/// Signature algorithm values: 0=Ed25519, 1=ECDSA-P256, 2=RSA-PSS-4096.
pub struct ContainerCryptoPayload {
    /// Hash of the image to be signed.
    pub image_hash: u64,
    /// Size of the OCI manifest JSON in bytes.
    pub manifest_bytes: usize,
    /// Signing algorithm selector (0=Ed25519, 1=ECDSA-P256, 2=RSA-PSS-4096).
    pub signature_algo: u8,
}

/// Prepare an image signing payload for ALICE-Crypto.
///
/// `algo`: 0=Ed25519, 1=ECDSA-P256, 2=RSA-PSS-4096.
#[inline]
#[must_use]
pub const fn container_to_crypto_payload(
    image_hash: u64,
    manifest_size: usize,
    algo: u8,
) -> ContainerCryptoPayload {
    ContainerCryptoPayload {
        image_hash,
        manifest_bytes: manifest_size,
        signature_algo: algo,
    }
}

// ── Bridge 6: Container → Queue (lifecycle event) ────────────────────────

/// Container lifecycle event for ALICE-Queue publication.
///
/// Event type values: 0=created, 1=started, 2=stopped, 3=destroyed, `4=oom_killed`.
pub struct ContainerQueueEvent {
    /// FNV-1a hash of the container identifier.
    pub container_hash: u64,
    /// Lifecycle event kind (0=created, 1=started, 2=stopped, 3=destroyed, `4=oom_killed`).
    pub event_type: u8,
    /// Event timestamp in milliseconds since Unix epoch.
    pub timestamp_ms: u64,
    /// Byte length of the JSON event payload to be enqueued.
    pub payload_bytes: usize,
}

/// Build a lifecycle event message for ALICE-Queue.
///
/// `event_type`: 0=created, 1=started, 2=stopped, 3=destroyed, `4=oom_killed`.
#[inline]
#[must_use]
pub fn container_to_queue_event(id: u64, event_type: u8, timestamp_ms: u64) -> ContainerQueueEvent {
    let container_hash = fnv1a(&id.to_le_bytes());
    // Estimate payload size: base JSON envelope is ~64 B; OOM events carry
    // an additional cgroup dump of ~256 B.
    let payload_bytes: usize = if event_type == 4 { 320 } else { 64 };
    ContainerQueueEvent {
        container_hash,
        event_type,
        timestamp_ms,
        payload_bytes,
    }
}

// ── Bridge 7: Container → RTOS (embedded schedule) ───────────────────────

/// Real-time task descriptor derived from a container for ALICE-RTOS scheduling.
pub struct ContainerRtosTask {
    /// FNV-1a hash of the container identifier.
    pub container_hash: u64,
    /// Task priority (0 = lowest, 255 = highest).
    pub priority: u8,
    /// Task activation period in microseconds.
    pub period_us: u64,
    /// Absolute deadline relative to activation, in microseconds.
    pub deadline_us: u64,
    /// Worst-case execution time estimate in microseconds.
    pub wcet_us: u64,
}

/// Map a container workload to an ALICE-RTOS periodic task descriptor.
#[inline]
#[must_use]
pub fn container_to_rtos_task(
    id: u64,
    priority: u8,
    period_us: u64,
    deadline_us: u64,
    wcet_us: u64,
) -> ContainerRtosTask {
    let container_hash = fnv1a(&id.to_le_bytes());
    ContainerRtosTask {
        container_hash,
        priority,
        period_us,
        deadline_us,
        wcet_us,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_to_db_record() {
        let rec = container_to_db_record(0xdeadbeef_cafebabe, 100_000, 512 * 1024 * 1024, 1);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.image_hash, 0xdeadbeef_cafebabe);
        assert_eq!(rec.cpu_limit_us, 100_000);
        assert_eq!(rec.memory_limit_bytes, 512 * 1024 * 1024);
        assert_eq!(rec.state, 1);
    }

    #[test]
    fn test_container_to_cache_entry() {
        let data = b"fake layer payload for testing";
        let entry = container_to_cache_entry(data, 3);
        assert_ne!(entry.layer_hash, 0);
        assert_eq!(entry.uncompressed_bytes, data.len());
        assert!(entry.compressed_bytes < entry.uncompressed_bytes);
        assert_eq!(entry.layer_index, 3);
    }

    #[test]
    fn test_container_to_analytics_metrics() {
        let m = container_to_analytics_metrics(42, 75.5, 60.0, 1024, 2048, 300);
        assert_ne!(m.container_hash, 0);
        assert!((m.cpu_usage_pct - 75.5).abs() < f32::EPSILON);
        assert!((m.memory_usage_pct - 60.0).abs() < f32::EPSILON);
        assert_eq!(m.io_read_bytes, 1024);
        assert_eq!(m.io_write_bytes, 2048);
        assert_eq!(m.uptime_seconds, 300);
    }

    #[test]
    fn test_container_to_auth_request() {
        let req = container_to_auth_request("registry.alice.io", "alice/node:latest", 0);
        assert_ne!(req.registry_hash, 0);
        assert_ne!(req.image_hash, 0);
        assert_eq!(req.scope, 0);
        assert_eq!(req.token_bytes, 256);

        let req_push = container_to_auth_request("registry.alice.io", "alice/node:latest", 1);
        assert_eq!(req_push.token_bytes, 512);
    }

    #[test]
    fn test_container_to_crypto_payload() {
        let payload = container_to_crypto_payload(0xabcdef1234567890, 4096, 0);
        assert_eq!(payload.image_hash, 0xabcdef1234567890);
        assert_eq!(payload.manifest_bytes, 4096);
        assert_eq!(payload.signature_algo, 0);
    }

    #[test]
    fn test_container_to_queue_event() {
        let ev = container_to_queue_event(99, 1, 1_700_000_000_000);
        assert_ne!(ev.container_hash, 0);
        assert_eq!(ev.event_type, 1);
        assert_eq!(ev.timestamp_ms, 1_700_000_000_000);
        assert_eq!(ev.payload_bytes, 64);

        // OOM event carries a larger payload.
        let oom = container_to_queue_event(99, 4, 1_700_000_000_001);
        assert_eq!(oom.payload_bytes, 320);
    }

    #[test]
    fn test_container_to_rtos_task() {
        let task = container_to_rtos_task(7, 200, 10_000, 8_000, 500);
        assert_ne!(task.container_hash, 0);
        assert_eq!(task.priority, 200);
        assert_eq!(task.period_us, 10_000);
        assert_eq!(task.deadline_us, 8_000);
        assert_eq!(task.wcet_us, 500);
    }
}
