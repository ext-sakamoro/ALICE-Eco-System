//! Firewall bridges — ALICE-Edge-Firewall ↔ ML, Analytics, Edge, Cache, DB, Queue
//!
//! 6 bridges connecting ML-based flow classification to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Firewall → ML (flow features for anomaly detection) ─────────

/// Flow feature vector for ALICE-ML anomaly detection.
///
/// Packs 7 derived float features alongside raw flow metadata so that the
/// ML layer can run inference without any extra allocation or conversion.
pub struct FirewallMlFeatures {
    /// FNV-1a hash of the 5-tuple identifying this flow.
    pub flow_hash: u64,
    /// Raw packet count observed for this flow.
    pub packet_count: u32,
    /// Raw byte count observed for this flow.
    pub byte_count: u64,
    /// Average packet size in bytes (pre-computed by caller).
    pub avg_packet_size: f32,
    /// Burst score in [0.0, 1.0]: fraction of packets arriving in bursts.
    pub burst_score: f32,
    /// Timing variance (inter-arrival jitter) in milliseconds.
    pub timing_variance: f32,
    /// Packed feature vector for ML inference:
    /// [0] packets_f32, [1] bytes_f32 (saturating), [2] avg_packet_size,
    /// [3] burst_score, [4] timing_variance,
    /// [5] bytes_per_packet_ratio (bytes / packets, reciprocal-multiply),
    /// [6] log2_packets (f32 bit-cast trick for fast log2 approximation).
    pub features: [f32; 7],
}

/// Build a `FirewallMlFeatures` from raw flow counters.
///
/// # Optimization notes
/// - `bytes_per_packet_ratio` uses reciprocal multiply (`* rcp_packets`) to
///   avoid a runtime division in the feature extraction hot path.
/// - `log2_packets` uses the IEEE-754 exponent extraction trick:
///   `((bits >> 23) & 0xFF) - 127` gives `floor(log2(x))` as an integer,
///   cast to f32 — branchless, single shift + mask + sub.
/// - No branches; all fields computed with arithmetic only.
#[inline(always)]
pub fn firewall_to_ml_features(
    flow_hash: u64,
    packets: u32,
    bytes: u64,
    avg_size: f32,
    burst: f32,
    timing_var: f32,
) -> FirewallMlFeatures {
    let packets_f32 = packets as f32;
    let bytes_f32 = bytes.min(u32::MAX as u64) as f32; // saturate to f32 range

    // bytes_per_packet_ratio: bytes / packets — reciprocal multiply, no division.
    // Guard packets = 0 by clamping denominator to 1 (branchless max).
    let packets_safe = packets_f32.max(1.0);
    let rcp_packets = packets_safe.recip(); // single RCPSS instruction
    let bytes_per_packet_ratio = bytes_f32 * rcp_packets;

    // log2_packets: fast floor(log2(x)) via IEEE-754 exponent extraction.
    // bits[30:23] = biased exponent; subtract 127 to get true exponent.
    let log2_packets = {
        let bits = packets_safe.to_bits();
        let exp = ((bits >> 23) & 0xFF) as i32 - 127;
        exp as f32
    };

    FirewallMlFeatures {
        flow_hash,
        packet_count: packets,
        byte_count: bytes,
        avg_packet_size: avg_size,
        burst_score: burst,
        timing_variance: timing_var,
        features: [
            packets_f32,
            bytes_f32,
            avg_size,
            burst,
            timing_var,
            bytes_per_packet_ratio,
            log2_packets,
        ],
    }
}

// ── Bridge 2: Firewall → Analytics (security telemetry) ──────────────────

/// Security telemetry event for ALICE-Analytics ingestion.
///
/// `verdict`: 0 = normal, 1 = ad, 2 = tracker, 3 = malicious.
pub struct FirewallAnalyticsEvent {
    /// FNV-1a hash of the 5-tuple identifying this flow.
    pub flow_hash: u64,
    /// Classification verdict (0=normal, 1=ad, 2=tracker, 3=malicious).
    pub verdict: u8,
    /// ML confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// Unix timestamp in milliseconds when the verdict was issued.
    pub timestamp_ms: u64,
    /// Source port of the classified flow.
    pub src_port: u16,
    /// Destination port of the classified flow.
    pub dst_port: u16,
    /// IP protocol number (6=TCP, 17=UDP, etc.).
    pub protocol: u8,
}

/// Build a security telemetry event for ALICE-Analytics.
#[inline(always)]
pub fn firewall_to_analytics_event(
    flow_hash: u64,
    verdict: u8,
    confidence: f32,
    ts: u64,
    src_port: u16,
    dst_port: u16,
    proto: u8,
) -> FirewallAnalyticsEvent {
    FirewallAnalyticsEvent {
        flow_hash,
        verdict,
        confidence,
        timestamp_ms: ts,
        src_port,
        dst_port,
        protocol: proto,
    }
}

// ── Bridge 3: Firewall → Edge (forwarding decision) ───────────────────────

/// Forwarding decision for ALICE-Edge packet scheduling.
///
/// `action`: 0 = pass, 1 = drop, 2 = rate_limit.
pub struct FirewallEdgeDecision {
    /// FNV-1a hash of the 5-tuple identifying this flow.
    pub flow_hash: u64,
    /// Forwarding action (0=pass, 1=drop, 2=rate_limit).
    pub action: u8,
    /// Rate limit in bits per second (0 when action != rate_limit).
    pub rate_limit_bps: u64,
    /// Scheduling priority (0 = lowest, 255 = highest).
    pub priority: u8,
}

/// Derive an ALICE-Edge forwarding decision from a firewall verdict.
///
/// Verdict → action mapping (branchless match, compiler emits jump table):
/// - 0 (normal)    → pass (0),    rate=0,           priority=128
/// - 1 (ad)        → rate_limit (2), rate=512_000,  priority=32
/// - 2 (tracker)   → rate_limit (2), rate=128_000,  priority=16
/// - 3 (malicious) → drop (1),    rate=0,           priority=0
/// - _             → drop (1),    rate=0,           priority=0
#[inline(always)]
pub fn firewall_to_edge_decision(flow_hash: u64, verdict: u8) -> FirewallEdgeDecision {
    // Branchless table: (action, rate_limit_bps, priority)
    // Index clamped to [0, 3] via min to avoid out-of-bounds.
    const TABLE: [(u8, u64, u8); 5] = [
        (0, 0, 128),      // 0: normal   → pass
        (2, 512_000, 32), // 1: ad       → rate_limit 512 kbps
        (2, 128_000, 16), // 2: tracker  → rate_limit 128 kbps
        (1, 0, 0),        // 3: malicious → drop
        (1, 0, 0),        // 4: unknown  → drop (fallback)
    ];
    let idx = (verdict as usize).min(4);
    let (action, rate_limit_bps, priority) = TABLE[idx];

    FirewallEdgeDecision {
        flow_hash,
        action,
        rate_limit_bps,
        priority,
    }
}

// ── Bridge 4: Firewall → Cache (flow verdict cache) ───────────────────────

/// Flow verdict cache entry for ALICE-Cache.
///
/// TTL is derived from ML confidence: high confidence yields a longer TTL so
/// that certain verdicts are served from cache for longer, reducing re-scoring.
pub struct FirewallCacheEntry {
    /// FNV-1a hash used as the cache key for this flow.
    pub flow_hash: u64,
    /// Classification verdict (0=normal, 1=ad, 2=tracker, 3=malicious).
    pub verdict: u8,
    /// ML confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// Time-to-live in seconds for this cache entry.
    pub ttl_seconds: u32,
    /// Number of cache lookups that have hit this entry since insertion.
    pub hit_count: u32,
}

/// Build a flow verdict cache entry with confidence-derived TTL.
///
/// TTL formula (branchless reciprocal multiply):
/// ```text
/// ttl = BASE_TTL_SECS * confidence    (multiply, no division)
/// ```
/// clamped to `[MIN_TTL, MAX_TTL]`.
///
/// - confidence = 1.0 → ttl = 300 s (5 min)
/// - confidence = 0.5 → ttl = 150 s
/// - confidence < 0.1 → ttl clamped to 10 s minimum
#[inline(always)]
pub fn firewall_to_cache_entry(flow_hash: u64, verdict: u8, confidence: f32) -> FirewallCacheEntry {
    const BASE_TTL_SECS: f32 = 300.0;
    const MIN_TTL: u32 = 10;
    const MAX_TTL: u32 = 300;

    // Branchless clamp: confidence in [0,1] guarded by min/max.
    let conf_clamped = confidence.max(0.0).min(1.0);
    // Reciprocal multiply: ttl = BASE * confidence — no division.
    let raw_ttl = (BASE_TTL_SECS * conf_clamped) as u32;
    let ttl_seconds = raw_ttl.max(MIN_TTL).min(MAX_TTL);

    FirewallCacheEntry {
        flow_hash,
        verdict,
        confidence,
        ttl_seconds,
        hit_count: 0,
    }
}

// ── Bridge 5: Firewall → DB (audit log) ───────────────────────────────────

/// Audit log record for ALICE-DB persistence.
///
/// Written for every flow verdict so that security analysts can replay
/// decisions and tune the ML model offline.
pub struct FirewallDbAuditLog {
    /// FNV-1a hash of the 5-tuple identifying this flow.
    pub flow_hash: u64,
    /// Classification verdict (0=normal, 1=ad, 2=tracker, 3=malicious).
    pub verdict: u8,
    /// ML confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// Unix timestamp in milliseconds when the verdict was issued.
    pub timestamp_ms: u64,
    /// Packet count at the time of classification.
    pub packet_count: u32,
    /// Byte count at the time of classification.
    pub byte_count: u64,
}

/// Build an audit log record for ALICE-DB.
#[inline(always)]
pub fn firewall_to_db_audit_log(
    flow_hash: u64,
    verdict: u8,
    confidence: f32,
    ts: u64,
    packets: u32,
    bytes: u64,
) -> FirewallDbAuditLog {
    FirewallDbAuditLog {
        flow_hash,
        verdict,
        confidence,
        timestamp_ms: ts,
        packet_count: packets,
        byte_count: bytes,
    }
}

// ── Bridge 6: Firewall → Queue (security alert) ───────────────────────────

/// Security alert for ALICE-Queue publishing.
///
/// Only emitted for non-normal verdicts (severity > 0), so normal flows
/// never produce queue traffic.  Severity mirrors the verdict value:
/// 0 = normal (not emitted), 1 = ad, 2 = tracker, 3 = malicious.
pub struct FirewallQueueAlert {
    /// FNV-1a hash of the 5-tuple identifying this flow.
    pub flow_hash: u64,
    /// Alert severity (1=ad, 2=tracker, 3=malicious).
    pub severity: u8,
    /// Classification verdict that triggered the alert.
    pub verdict: u8,
    /// ML confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// Estimated serialised payload size in bytes for queue budgeting.
    pub payload_bytes: usize,
}

/// Build a security alert for ALICE-Queue.
///
/// Returns `None` for normal flows (verdict == 0 → severity == 0) so the
/// caller never enqueues benign traffic.
///
/// Severity is derived from verdict via a branchless min clamp:
/// `severity = verdict.min(3)` — unknown high verdicts are capped at 3.
///
/// `payload_bytes` is estimated as `32 + (severity as usize * 8)`:
/// - severity 1 (ad):        40 bytes
/// - severity 2 (tracker):   48 bytes
/// - severity 3 (malicious): 56 bytes
/// Computed with integer multiply — no branches, no division.
#[inline(always)]
pub fn firewall_to_queue_alert(
    flow_hash: u64,
    verdict: u8,
    confidence: f32,
) -> Option<FirewallQueueAlert> {
    let severity = verdict.min(3);

    // Only emit alerts for non-normal verdicts.
    if severity == 0 {
        return None;
    }

    // payload_bytes: base 32 + severity * 8 — integer multiply, no division.
    let payload_bytes = 32 + (severity as usize) * 8;

    Some(FirewallQueueAlert {
        flow_hash,
        severity,
        verdict,
        confidence,
        payload_bytes,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bridge 1: Firewall → ML ──────────────────────────────────────────

    #[test]
    fn test_firewall_to_ml_features_basic() {
        let feat = firewall_to_ml_features(0xDEAD_BEEF, 100, 150_000, 1500.0, 0.3, 12.5);

        assert_eq!(feat.flow_hash, 0xDEAD_BEEF);
        assert_eq!(feat.packet_count, 100);
        assert_eq!(feat.byte_count, 150_000);
        assert!((feat.avg_packet_size - 1500.0).abs() < f32::EPSILON);
        assert!((feat.burst_score - 0.3).abs() < f32::EPSILON);
        assert!((feat.timing_variance - 12.5).abs() < f32::EPSILON);

        // features[0] = packets_f32
        assert!((feat.features[0] - 100.0).abs() < f32::EPSILON);
        // features[2] = avg_packet_size
        assert!((feat.features[2] - 1500.0).abs() < f32::EPSILON);
        // features[5] = bytes_per_packet_ratio = 150_000 / 100 = 1500.0
        // (saturated bytes_f32 = 150_000 as f32)
        assert!(
            (feat.features[5] - 1500.0).abs() < 1.0,
            "bytes_per_packet_ratio = {}",
            feat.features[5]
        );
        // features[6] = log2(100) ≈ 6.0 (floor)
        assert!(
            (feat.features[6] - 6.0).abs() < f32::EPSILON,
            "log2_packets = {}",
            feat.features[6]
        );
    }

    #[test]
    fn test_firewall_to_ml_features_zero_packets() {
        // packets = 0 must not panic or produce NaN/Inf
        let feat = firewall_to_ml_features(0, 0, 0, 0.0, 0.0, 0.0);
        assert!(
            feat.features[5].is_finite(),
            "bytes_per_packet_ratio must be finite"
        );
        assert!(feat.features[6].is_finite(), "log2_packets must be finite");
    }

    #[test]
    fn test_firewall_to_ml_features_large_bytes() {
        // byte_count > u32::MAX should saturate gracefully
        let feat = firewall_to_ml_features(1, 1, u64::MAX, 0.0, 0.0, 0.0);
        // bytes_f32 is clamped to u32::MAX as f32 — finite, no panic
        assert!(feat.features[1].is_finite());
    }

    // ── Bridge 2: Firewall → Analytics ──────────────────────────────────

    #[test]
    fn test_firewall_to_analytics_event() {
        let ev =
            firewall_to_analytics_event(0xCAFE_BABE, 3, 0.97, 1_700_000_000_000, 54321, 443, 6);
        assert_eq!(ev.flow_hash, 0xCAFE_BABE);
        assert_eq!(ev.verdict, 3);
        assert!((ev.confidence - 0.97).abs() < 1e-5);
        assert_eq!(ev.timestamp_ms, 1_700_000_000_000);
        assert_eq!(ev.src_port, 54321);
        assert_eq!(ev.dst_port, 443);
        assert_eq!(ev.protocol, 6);
    }

    // ── Bridge 3: Firewall → Edge ────────────────────────────────────────

    #[test]
    fn test_firewall_to_edge_decision_normal() {
        let d = firewall_to_edge_decision(1, 0);
        assert_eq!(d.action, 0, "normal → pass");
        assert_eq!(d.rate_limit_bps, 0);
        assert_eq!(d.priority, 128);
    }

    #[test]
    fn test_firewall_to_edge_decision_ad() {
        let d = firewall_to_edge_decision(2, 1);
        assert_eq!(d.action, 2, "ad → rate_limit");
        assert_eq!(d.rate_limit_bps, 512_000);
        assert_eq!(d.priority, 32);
    }

    #[test]
    fn test_firewall_to_edge_decision_tracker() {
        let d = firewall_to_edge_decision(3, 2);
        assert_eq!(d.action, 2, "tracker → rate_limit");
        assert_eq!(d.rate_limit_bps, 128_000);
        assert_eq!(d.priority, 16);
    }

    #[test]
    fn test_firewall_to_edge_decision_malicious() {
        let d = firewall_to_edge_decision(4, 3);
        assert_eq!(d.action, 1, "malicious → drop");
        assert_eq!(d.rate_limit_bps, 0);
        assert_eq!(d.priority, 0);
    }

    #[test]
    fn test_firewall_to_edge_decision_unknown_verdict() {
        // verdict > 3 must not panic; should fall back to drop.
        let d = firewall_to_edge_decision(99, 255);
        assert_eq!(d.action, 1, "unknown → drop");
        assert_eq!(d.priority, 0);
    }

    // ── Bridge 4: Firewall → Cache ───────────────────────────────────────

    #[test]
    fn test_firewall_to_cache_entry_high_confidence() {
        let e = firewall_to_cache_entry(0xABCD, 3, 1.0);
        assert_eq!(e.ttl_seconds, 300, "confidence=1.0 → max TTL 300s");
        assert_eq!(e.hit_count, 0);
        assert_eq!(e.verdict, 3);
    }

    #[test]
    fn test_firewall_to_cache_entry_half_confidence() {
        let e = firewall_to_cache_entry(0x1234, 1, 0.5);
        // ttl = 300 * 0.5 = 150, within [10, 300]
        assert_eq!(e.ttl_seconds, 150);
    }

    #[test]
    fn test_firewall_to_cache_entry_low_confidence() {
        let e = firewall_to_cache_entry(0xFFFF, 2, 0.01);
        // raw_ttl = 300 * 0.01 = 3, clamped to MIN_TTL=10
        assert_eq!(e.ttl_seconds, 10, "low confidence → clamped to minimum TTL");
    }

    #[test]
    fn test_firewall_to_cache_entry_zero_confidence() {
        let e = firewall_to_cache_entry(0, 0, 0.0);
        assert_eq!(e.ttl_seconds, 10);
    }

    // ── Bridge 5: Firewall → DB ──────────────────────────────────────────

    #[test]
    fn test_firewall_to_db_audit_log() {
        let log = firewall_to_db_audit_log(0xFEED_C0DE, 3, 0.99, 1_700_000_000_000, 42, 100_000);
        assert_eq!(log.flow_hash, 0xFEED_C0DE);
        assert_eq!(log.verdict, 3);
        assert!((log.confidence - 0.99).abs() < 1e-5);
        assert_eq!(log.timestamp_ms, 1_700_000_000_000);
        assert_eq!(log.packet_count, 42);
        assert_eq!(log.byte_count, 100_000);
    }

    // ── Bridge 6: Firewall → Queue ───────────────────────────────────────

    #[test]
    fn test_firewall_to_queue_alert_normal_returns_none() {
        let result = firewall_to_queue_alert(0xABCD, 0, 0.95);
        assert!(result.is_none(), "normal verdict must not produce an alert");
    }

    #[test]
    fn test_firewall_to_queue_alert_ad() {
        let alert =
            firewall_to_queue_alert(0x1111, 1, 0.8).expect("ad verdict must produce an alert");
        assert_eq!(alert.severity, 1);
        assert_eq!(alert.verdict, 1);
        assert_eq!(alert.payload_bytes, 40); // 32 + 1 * 8
    }

    #[test]
    fn test_firewall_to_queue_alert_tracker() {
        let alert = firewall_to_queue_alert(0x2222, 2, 0.85)
            .expect("tracker verdict must produce an alert");
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.payload_bytes, 48); // 32 + 2 * 8
    }

    #[test]
    fn test_firewall_to_queue_alert_malicious() {
        let alert = firewall_to_queue_alert(0x3333, 3, 0.99)
            .expect("malicious verdict must produce an alert");
        assert_eq!(alert.severity, 3);
        assert_eq!(alert.payload_bytes, 56); // 32 + 3 * 8
        assert!((alert.confidence - 0.99).abs() < 1e-5);
        assert_eq!(alert.flow_hash, 0x3333);
    }

    #[test]
    fn test_firewall_to_queue_alert_unknown_verdict_capped() {
        // verdict > 3 is capped at severity 3 — must produce an alert, not None
        let alert = firewall_to_queue_alert(0xFFFF, 255, 0.5)
            .expect("unknown verdict > 0 must produce an alert");
        assert_eq!(alert.severity, 3);
        assert_eq!(alert.payload_bytes, 56);
    }

    // ── fnv1a sanity check ────────────────────────────────────────────────

    #[test]
    fn test_fnv1a_deterministic() {
        let h1 = fnv1a(b"alice-firewall");
        let h2 = fnv1a(b"alice-firewall");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
        // Different inputs must not collide on this short test vector.
        let h3 = fnv1a(b"alice-edge");
        assert_ne!(h1, h3);
    }
}
