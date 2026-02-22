//! ASPCommercial bridges — ALICE-Streaming-Protocol-Enterprise ↔ DB, Analytics, Auth
//!
//! 3 bridges connecting the enterprise ASP crate to the ALICE ecosystem.
//! Gated behind the `streaming-protocol-commercial` feature flag.

#[cfg(feature = "streaming-protocol-commercial")]
use libasp_enterprise::audit::metrics::{MetricsSnapshot, PacketMetricsSnapshot};
#[cfg(feature = "streaming-protocol-commercial")]
use libasp_enterprise::audit::{AuditEvent, AuditLog, EventSeverity, EventType};
#[cfg(feature = "streaming-protocol-commercial")]
use libasp_enterprise::gpu::{GpuBackend, GpuConfig};

// ── Bridge 1: ASPCommercial → DB (audit log persistence) ─────────────────

/// Database-ready audit record for ALICE-DB persistence.
///
/// Converts an enterprise ASP audit event into a flat, heap-free record
/// for indexed storage. All string fields are dropped or hashed to avoid
/// heap allocation in the hot path.
#[cfg(feature = "streaming-protocol-commercial")]
pub struct AspCommercialDbAuditRecord {
    /// Monotonically-increasing event ID from the AuditLog.
    pub event_id: u64,
    /// EventType discriminant as u8 for compact DB column storage.
    pub event_type_discriminant: u8,
    /// EventSeverity level (0=Debug … 4=Critical).
    pub severity_level: u8,
    /// Is this a security-relevant event? (index hint for audit queries)
    pub is_security_event: bool,
    /// Device ID bytes (8 bytes, zeroed when absent).
    pub device_id: [u8; 8],
    /// Has a device ID (distinguishes absent from zeroed).
    pub has_device_id: bool,
    /// Optional packet size in bytes (0 when not applicable).
    pub packet_size: u32,
    /// FNV-1a content hash for deduplication across DB writes.
    pub content_hash: u64,
}

/// Convert an enterprise ASP audit event to a DB persistence record.
///
/// # Optimization notes
/// - EventType discriminant: direct `as u8` cast, no match.
/// - `is_security_event`: one method call, branchless bool result.
/// - Device ID: `unwrap_or([0u8; 8])` — branchless via Option pattern.
/// - Content hash: FNV-1a over event_id + type_byte + severity_byte + device_id.
#[cfg(feature = "streaming-protocol-commercial")]
#[inline]
pub fn asp_commercial_to_db_audit_record(event: &AuditEvent) -> AspCommercialDbAuditRecord {
    let event_type_discriminant = event.event_type as u8;
    let severity_level = event.severity.level();
    let is_security_event = event.event_type.is_security_event();

    // Device ID: unwrap or zero-fill.
    let (device_id, has_device_id) = match event.device_id {
        Some(id) => (id, true),
        None => ([0u8; 8], false),
    };

    let packet_size = event.packet_size.unwrap_or(0) as u32;

    // Content hash: event_id (8) + type (1) + severity (1) + device_id (8) = 18 bytes.
    let mut key = [0u8; 18];
    key[0..8].copy_from_slice(&event.id.to_le_bytes());
    key[8] = event_type_discriminant;
    key[9] = severity_level;
    key[10..18].copy_from_slice(&device_id);
    let content_hash = crate::hash::fnv1a(&key);

    AspCommercialDbAuditRecord {
        event_id: event.id,
        event_type_discriminant,
        severity_level,
        is_security_event,
        device_id,
        has_device_id,
        packet_size,
        content_hash,
    }
}

/// Flush a recent slice of audit events to DB records in a single batch.
///
/// Returns one record per event in the slice. Caller is responsible for
/// deduplication by `content_hash` before writing to the DB.
#[cfg(feature = "streaming-protocol-commercial")]
#[inline]
pub fn asp_commercial_audit_log_to_db_batch(
    log: &AuditLog,
    batch_size: usize,
) -> Vec<AspCommercialDbAuditRecord> {
    log.recent(batch_size)
        .iter()
        .map(|event| asp_commercial_to_db_audit_record(event))
        .collect()
}

// ── Bridge 2: ASPCommercial → Analytics (enterprise streaming metrics) ───

/// Enterprise streaming analytics record for ALICE-Analytics ingestion.
///
/// Combines `MetricsSnapshot` and GPU configuration into a single flat record
/// for time-series storage and dashboard display.
#[cfg(feature = "streaming-protocol-commercial")]
pub struct AspCommercialAnalyticsRecord {
    /// Uptime in seconds since the enterprise encoder started.
    pub uptime_seconds: u64,
    /// Active connections at snapshot time.
    pub active_connections: u64,
    /// Total lifetime connections.
    pub total_connections: u64,
    /// Authentication success count.
    pub auth_successes: u64,
    /// Authentication failure count.
    pub auth_failures: u64,
    /// Authentication success rate in [0.0, 1.0].
    pub auth_success_rate: f64,
    /// Total packets sent (all types).
    pub total_packets_sent: u64,
    /// Total packets received (all types).
    pub total_packets_recv: u64,
    /// Encrypted byte ratio in [0.0, 1.0].
    pub encryption_ratio: f64,
    /// Total protocol error count.
    pub protocol_errors: u64,
    /// GPU backend in use (0=CPU, 1=Cuda, 2=Metal, 3=Vulkan).
    pub gpu_backend_id: u8,
    /// GPU device index.
    pub gpu_device_index: u8,
    /// GPU async encoding enabled flag.
    pub gpu_async_encode: bool,
    /// Content hash for dedup / time-series keying.
    pub content_hash: u64,
}

/// Build an analytics record from a metrics snapshot and GPU configuration.
///
/// # Optimization notes
/// - Auth success rate: `MetricsSnapshot::auth_success_rate()` (reciprocal multiply).
/// - Encryption ratio: `PacketMetricsSnapshot::encryption_ratio()` (reciprocal multiply).
/// - GPU backend: direct `as u8` cast via a small const table for the enum.
/// - Content hash: FNV-1a over uptime + packets_sent + auth_failures + gpu_backend.
#[cfg(feature = "streaming-protocol-commercial")]
#[inline]
pub fn asp_commercial_to_analytics_record(
    snapshot: &MetricsSnapshot,
    gpu_config: &GpuConfig,
) -> AspCommercialAnalyticsRecord {
    let auth_success_rate = snapshot.auth_success_rate();
    let encryption_ratio = snapshot.packets.encryption_ratio();
    let total_packets_sent = snapshot.packets.total_packets_sent();
    let total_packets_recv = snapshot.packets.total_packets_recv();
    let protocol_errors = snapshot.packets.protocol_errors;

    // GPU backend: map enum to u8 via branchless const table.
    // GpuBackend has no guaranteed discriminant; use explicit mapping.
    let gpu_backend_id: u8 = match gpu_config.backend {
        GpuBackend::Cpu => 0,
        GpuBackend::Cuda => 1,
        GpuBackend::Metal => 2,
        GpuBackend::Vulkan => 3,
    };
    let gpu_device_index = gpu_config.device_index.min(255) as u8;
    let gpu_async_encode = gpu_config.async_encode;

    // Content hash over key counters + GPU backend.
    let auth_fail_pct = if (snapshot.auth_successes + snapshot.auth_failures) > 0 {
        ((snapshot.auth_failures * 100) / (snapshot.auth_successes + snapshot.auth_failures)) as u8
    } else {
        0u8
    };
    let mut key = [0u8; 18];
    key[0..8].copy_from_slice(&snapshot.uptime_seconds.to_le_bytes());
    key[8..16].copy_from_slice(&total_packets_sent.to_le_bytes());
    key[16] = gpu_backend_id;
    key[17] = auth_fail_pct;
    let content_hash = crate::hash::fnv1a(&key);

    AspCommercialAnalyticsRecord {
        uptime_seconds: snapshot.uptime_seconds,
        active_connections: snapshot.active_connections,
        total_connections: snapshot.total_connections,
        auth_successes: snapshot.auth_successes,
        auth_failures: snapshot.auth_failures,
        auth_success_rate,
        total_packets_sent,
        total_packets_recv,
        encryption_ratio,
        protocol_errors,
        gpu_backend_id,
        gpu_device_index,
        gpu_async_encode,
        content_hash,
    }
}

// ── Bridge 3: ASPCommercial → Auth (access control integration) ──────────

/// Auth access-control descriptor for an enterprise ASP streaming session.
///
/// Derives a compact, stateless session token from a packet metrics snapshot
/// and an ALICE-Auth identity hash, enabling the Auth subsystem to validate
/// enterprise stream access without a round-trip to the license server.
#[cfg(feature = "streaming-protocol-commercial")]
pub struct AspCommercialAuthDescriptor {
    /// Identity fingerprint (FNV-1a of the 32-byte Ed25519 public key).
    pub identity_hash: u64,
    /// Session fingerprint derived from enterprise packet metrics.
    pub session_hash: u64,
    /// Combined access token (identity XOR rotated session hash).
    pub access_token: u64,
    /// Is this session allowed to use GPU acceleration?
    pub gpu_authorized: bool,
    /// GPU backend the session is authorized for (0=CPU fallback).
    pub authorized_gpu_backend: u8,
    /// Encryption ratio at time of token issuance (session quality signal).
    pub encryption_ratio: f32,
}

/// Build an enterprise ASP auth descriptor for ALICE-Auth integration.
///
/// `identity_bytes` is the 32-byte Ed25519 public key from the authenticated
/// ALICE-Auth identity. `snapshot` is the current enterprise metrics snapshot.
/// `gpu_config` determines which GPU backend the session is authorized for.
///
/// # Optimization notes
/// - Identity hash: FNV-1a over the 32-byte key.
/// - Session hash: FNV-1a over auth counters + total packets.
/// - `access_token`: XOR + 19-bit rotate to reduce collision probability.
/// - `gpu_authorized`: `gpu_backend_id != 0` — branchless bool.
#[cfg(feature = "streaming-protocol-commercial")]
#[inline]
pub fn asp_commercial_auth_descriptor(
    identity_bytes: &[u8; 32],
    snapshot: &MetricsSnapshot,
    gpu_config: &GpuConfig,
) -> AspCommercialAuthDescriptor {
    let identity_hash = crate::hash::fnv1a(identity_bytes);

    // Session hash: auth counters + packet totals.
    let mut session_buf = [0u8; 24];
    session_buf[0..8].copy_from_slice(&snapshot.auth_successes.to_le_bytes());
    session_buf[8..16].copy_from_slice(&snapshot.auth_failures.to_le_bytes());
    session_buf[16..24].copy_from_slice(&snapshot.packets.total_packets_sent().to_le_bytes());
    let session_hash = crate::hash::fnv1a(&session_buf);

    // Access token: rotate session hash by 19 bits before XOR to reduce
    // hash collision probability when identity and session hashes are similar.
    let access_token = identity_hash ^ session_hash.rotate_left(19);

    let gpu_backend_id: u8 = match gpu_config.backend {
        GpuBackend::Cpu => 0,
        GpuBackend::Cuda => 1,
        GpuBackend::Metal => 2,
        GpuBackend::Vulkan => 3,
    };

    // GPU authorized: branchless — any non-CPU backend is considered licensed.
    let gpu_authorized = gpu_backend_id != 0;

    // Encryption ratio: f32 snapshot for access quality signalling.
    let encryption_ratio = snapshot.packets.encryption_ratio() as f32;

    AspCommercialAuthDescriptor {
        identity_hash,
        session_hash,
        access_token,
        gpu_authorized,
        authorized_gpu_backend: gpu_backend_id,
        encryption_ratio,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "streaming-protocol-commercial"))]
mod tests {
    use super::*;
    use libasp_enterprise::audit::metrics::{Metrics, MetricsSnapshot, PacketMetrics};
    use libasp_enterprise::audit::{AuditEvent, AuditLog, EventSeverity, EventType};
    use libasp_enterprise::gpu::{GpuBackend, GpuConfig};

    // Helper: build a minimal MetricsSnapshot for testing.
    fn make_snapshot(
        auth_ok: u64,
        auth_fail: u64,
        pkts_sent: u64,
        pkts_recv: u64,
    ) -> MetricsSnapshot {
        use chrono::Utc;
        use libasp_enterprise::audit::metrics::{ChannelMetrics, PacketMetricsSnapshot};

        MetricsSnapshot {
            timestamp: Utc::now(),
            uptime_seconds: 3600,
            packets: PacketMetricsSnapshot {
                i_packets_sent: pkts_sent / 10,
                d_packets_sent: pkts_sent - pkts_sent / 10,
                c_packets_sent: 0,
                s_packets_sent: 0,
                i_packets_recv: pkts_recv / 10,
                d_packets_recv: pkts_recv - pkts_recv / 10,
                c_packets_recv: 0,
                s_packets_recv: 0,
                bytes_sent: pkts_sent * 1024,
                bytes_recv: pkts_recv * 512,
                encrypted_bytes_sent: pkts_sent * 1024,
                encrypted_bytes_recv: pkts_recv * 512,
                encryption_errors: 0,
                decryption_errors: 0,
                protocol_errors: 2,
            },
            active_connections: 5,
            total_connections: 100,
            auth_successes: auth_ok,
            auth_failures: auth_fail,
            channels: vec![],
        }
    }

    // ── Bridge 1 tests ────────────────────────────────────────────────────

    #[test]
    fn test_asp_commercial_to_db_audit_record_connect() {
        let event = AuditEvent::new(EventType::Connect)
            .with_device_id([0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);

        let rec = asp_commercial_to_db_audit_record(&event);

        assert_eq!(rec.event_type_discriminant, EventType::Connect as u8);
        assert_eq!(rec.severity_level, EventSeverity::Info.level());
        assert!(!rec.is_security_event, "Connect is not a security event");
        assert!(rec.has_device_id);
        assert_eq!(
            rec.device_id,
            [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]
        );
        assert_eq!(rec.packet_size, 0);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.content_hash, 0xcbf29ce484222325);
    }

    #[test]
    fn test_asp_commercial_to_db_audit_record_auth_failure() {
        let event = AuditEvent::new(EventType::AuthFailure).with_packet_size(256);

        let rec = asp_commercial_to_db_audit_record(&event);

        assert_eq!(rec.severity_level, EventSeverity::Warning.level());
        assert!(rec.is_security_event, "AuthFailure is a security event");
        assert!(!rec.has_device_id);
        assert_eq!(rec.device_id, [0u8; 8]);
        assert_eq!(rec.packet_size, 256);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_asp_commercial_audit_log_to_db_batch() {
        let mut log = AuditLog::new(100);
        log.record(AuditEvent::new(EventType::Connect));
        log.record(AuditEvent::new(EventType::Subscribe));
        log.record(AuditEvent::new(EventType::AuthSuccess));

        let batch = asp_commercial_audit_log_to_db_batch(&log, 3);

        assert_eq!(batch.len(), 3);
        // All records should have non-zero content hashes.
        for rec in &batch {
            assert_ne!(rec.content_hash, 0);
        }
    }

    #[test]
    fn test_asp_commercial_db_different_events_different_hashes() {
        let e1 = AuditEvent::new(EventType::Connect);
        let e2 = AuditEvent::new(EventType::Disconnect);
        let r1 = asp_commercial_to_db_audit_record(&e1);
        let r2 = asp_commercial_to_db_audit_record(&e2);
        // Different event types → different content hashes.
        assert_ne!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2 tests ────────────────────────────────────────────────────

    #[test]
    fn test_asp_commercial_to_analytics_record_basic() {
        let snap = make_snapshot(900, 100, 2000, 1800);
        let gpu = GpuConfig {
            backend: GpuBackend::Metal,
            device_index: 0,
            max_streams: 4,
            async_encode: true,
        };

        let rec = asp_commercial_to_analytics_record(&snap, &gpu);

        assert_eq!(rec.uptime_seconds, 3600);
        assert_eq!(rec.active_connections, 5);
        assert_eq!(rec.total_connections, 100);
        assert_eq!(rec.auth_successes, 900);
        assert_eq!(rec.auth_failures, 100);
        // auth_success_rate = 900/1000 = 0.9
        assert!((rec.auth_success_rate - 0.9).abs() < 1e-9);
        assert_eq!(rec.total_packets_sent, 2000);
        assert_eq!(rec.total_packets_recv, 1800);
        // All bytes are encrypted in this fixture → ratio = 1.0
        assert!((rec.encryption_ratio - 1.0).abs() < 1e-9);
        assert_eq!(rec.protocol_errors, 2);
        assert_eq!(rec.gpu_backend_id, 2, "Metal = 2");
        assert_eq!(rec.gpu_device_index, 0);
        assert!(rec.gpu_async_encode);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_asp_commercial_to_analytics_record_cpu_backend() {
        let snap = make_snapshot(0, 0, 0, 0);
        let gpu = GpuConfig::default(); // Cpu backend

        let rec = asp_commercial_to_analytics_record(&snap, &gpu);

        assert_eq!(rec.gpu_backend_id, 0, "CPU = 0");
        // auth_success_rate defaults to 1.0 with zero attempts.
        assert!((rec.auth_success_rate - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_asp_commercial_to_analytics_record_cuda_backend() {
        let snap = make_snapshot(50, 0, 500, 500);
        let gpu = GpuConfig {
            backend: GpuBackend::Cuda,
            ..GpuConfig::default()
        };

        let rec = asp_commercial_to_analytics_record(&snap, &gpu);
        assert_eq!(rec.gpu_backend_id, 1, "CUDA = 1");
    }

    // ── Bridge 3 tests ────────────────────────────────────────────────────

    #[test]
    fn test_asp_commercial_auth_descriptor_gpu_authorized() {
        let identity: [u8; 32] = [0xAA; 32];
        let snap = make_snapshot(100, 0, 1000, 900);
        let gpu = GpuConfig {
            backend: GpuBackend::Metal,
            device_index: 0,
            max_streams: 4,
            async_encode: true,
        };

        let desc = asp_commercial_auth_descriptor(&identity, &snap, &gpu);

        assert_ne!(desc.identity_hash, 0);
        assert_ne!(desc.session_hash, 0);
        assert_ne!(desc.access_token, 0);
        assert!(desc.gpu_authorized, "Metal backend is GPU-authorized");
        assert_eq!(desc.authorized_gpu_backend, 2, "Metal = 2");
        assert!((desc.encryption_ratio - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_asp_commercial_auth_descriptor_cpu_not_authorized() {
        let identity: [u8; 32] = [0xBB; 32];
        let snap = make_snapshot(50, 5, 500, 450);
        let gpu = GpuConfig::default(); // CPU fallback

        let desc = asp_commercial_auth_descriptor(&identity, &snap, &gpu);
        assert!(!desc.gpu_authorized, "CPU backend is not GPU-authorized");
        assert_eq!(desc.authorized_gpu_backend, 0);
    }

    #[test]
    fn test_asp_commercial_auth_descriptor_different_identities() {
        let snap = make_snapshot(100, 0, 1000, 900);
        let gpu = GpuConfig {
            backend: GpuBackend::Vulkan,
            ..GpuConfig::default()
        };

        let identity_a: [u8; 32] = [0xAA; 32];
        let identity_b: [u8; 32] = [0xBB; 32];

        let desc_a = asp_commercial_auth_descriptor(&identity_a, &snap, &gpu);
        let desc_b = asp_commercial_auth_descriptor(&identity_b, &snap, &gpu);

        // Different identities must produce different tokens.
        assert_ne!(desc_a.identity_hash, desc_b.identity_hash);
        assert_ne!(desc_a.access_token, desc_b.access_token);
        // Session hash is independent of identity.
        assert_eq!(desc_a.session_hash, desc_b.session_hash);
    }

    #[test]
    fn test_asp_commercial_auth_descriptor_vulkan_authorized() {
        let identity: [u8; 32] = [0x01; 32];
        let snap = make_snapshot(1, 0, 10, 10);
        let gpu = GpuConfig {
            backend: GpuBackend::Vulkan,
            ..GpuConfig::default()
        };

        let desc = asp_commercial_auth_descriptor(&identity, &snap, &gpu);
        assert!(desc.gpu_authorized);
        assert_eq!(desc.authorized_gpu_backend, 3, "Vulkan = 3");
    }
}
