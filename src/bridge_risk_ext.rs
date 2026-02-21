//! Risk extended bridges — ALICE-Risk ↔ Analytics, Cache, Semantic Telemetry
//!
//! 3 bridges connecting pre-trade risk management to the ALICE ecosystem.

use alice_risk::{RiskReject, RiskLimits};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Risk Reject → Analytics (rejection metrics) ────────────────

/// Rejection event for ALICE-Analytics ingestion.
///
/// Encodes a `RiskReject` variant as a compact numeric code so that the
/// analytics layer can build rejection-rate histograms without deserialising
/// the full enum on every hot path.
pub struct RiskAnalyticsRejectEvent {
    /// FNV-1a hash of `reject_code` byte concatenated with `timestamp_ns`
    /// little-endian bytes — used as the analytics stream key.
    pub content_hash: u64,
    /// Numeric rejection code (1–6).
    pub reject_code: u8,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a `RiskReject` to a compact `RiskAnalyticsRejectEvent`.
///
/// Reject code mapping:
/// - 1 = `PositionLimitBreached`
/// - 2 = `OrderSizeTooLarge`
/// - 3 = `NotionalExceeded`
/// - 4 = `MaxOpenOrdersReached`
/// - 5 = `DailyLossLimitHit`
/// - 6 = `CircuitBreakerTripped`
///
/// The content hash is `fnv1a([reject_code] ++ timestamp_ns.to_le_bytes())`.
#[inline]
pub fn risk_reject_to_analytics(
    reject: &RiskReject,
    timestamp_ns: u64,
) -> RiskAnalyticsRejectEvent {
    let reject_code: u8 = match reject {
        RiskReject::PositionLimitBreached { .. } => 1,
        RiskReject::OrderSizeTooLarge { .. }     => 2,
        RiskReject::NotionalExceeded { .. }      => 3,
        RiskReject::MaxOpenOrdersReached { .. }  => 4,
        RiskReject::DailyLossLimitHit { .. }     => 5,
        RiskReject::CircuitBreakerTripped        => 6,
    };

    // Hash: reject_code byte followed by timestamp bytes.
    let mut buf = [0u8; 9];
    buf[0] = reject_code;
    buf[1..9].copy_from_slice(&timestamp_ns.to_le_bytes());
    let content_hash = fnv1a(&buf);

    RiskAnalyticsRejectEvent {
        content_hash,
        reject_code,
        timestamp_ns,
    }
}

// ── Bridge 2: Risk Limits → Cache (per-account limit lookup) ─────────────

/// Per-account risk-limits cache entry for ALICE-Cache.
///
/// Allows the order-entry path to look up limits in O(1) without hitting
/// the risk-config store on every order.  TTL is fixed at 3600 s (1 hour).
pub struct RiskLimitsCacheEntry {
    /// FNV-1a hash of `account_id` LE bytes concatenated with
    /// `max_position` LE bytes — used as the cache key.
    pub content_hash: u64,
    /// Maximum absolute position size (contracts/shares).
    pub max_position: u64,
    /// Maximum single-order size.
    pub max_order_size: u64,
    /// Maximum notional exposure (signed, in base currency minor units).
    pub max_notional: i64,
    /// Maximum number of open orders simultaneously.
    pub max_open_orders: u32,
    /// Maximum daily loss allowed (signed, in base currency minor units).
    pub max_daily_loss: i64,
    /// Cache entry time-to-live in seconds.
    pub ttl_secs: u32,
}

/// Snapshot `RiskLimits` into a `RiskLimitsCacheEntry` for `account_id`.
///
/// TTL is always 3600 s so that stale limits are evicted within one hour.
/// The content hash is `fnv1a(account_id.to_le_bytes() ++ max_position.to_le_bytes())`.
#[inline]
pub fn risk_limits_to_cache(
    limits: &RiskLimits,
    account_id: u64,
) -> RiskLimitsCacheEntry {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&account_id.to_le_bytes());
    buf[8..16].copy_from_slice(&limits.max_position.to_le_bytes());
    let content_hash = fnv1a(&buf);

    RiskLimitsCacheEntry {
        content_hash,
        max_position:    limits.max_position,
        max_order_size:  limits.max_order_size,
        max_notional:    limits.max_notional,
        max_open_orders: limits.max_open_orders,
        max_daily_loss:  limits.max_daily_loss,
        ttl_secs: 3600,
    }
}

// ── Bridge 3: Risk Reject → Semantic Telemetry (event tracing) ───────────

/// Risk-rejection event for ALICE-Semantic-Telemetry tracing.
///
/// Pairs the rejection code with a severity level so that the telemetry
/// layer can route high-severity events to alerting pipelines without
/// re-inspecting the original enum.
pub struct RiskSemanticEvent {
    /// FNV-1a hash of `reject_code` byte concatenated with `timestamp_ns`
    /// little-endian bytes — event trace key.
    pub content_hash: u64,
    /// Numeric rejection code (1–6), same mapping as `risk_reject_to_analytics`.
    pub reject_code: u8,
    /// Severity level: 1 = low, 2 = medium, 3 = high.
    pub severity: u8,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a `RiskReject` to a `RiskSemanticEvent` for telemetry tracing.
///
/// Severity mapping:
/// - 3 (high)   = `CircuitBreakerTripped`, `DailyLossLimitHit`
/// - 2 (medium) = `PositionLimitBreached`, `NotionalExceeded`
/// - 1 (low)    = `OrderSizeTooLarge`, `MaxOpenOrdersReached`
///
/// The content hash uses the same scheme as `risk_reject_to_analytics`.
#[inline]
pub fn risk_reject_to_semantic(
    reject: &RiskReject,
    timestamp_ns: u64,
) -> RiskSemanticEvent {
    let (reject_code, severity): (u8, u8) = match reject {
        RiskReject::PositionLimitBreached { .. } => (1, 2),
        RiskReject::OrderSizeTooLarge { .. }     => (2, 1),
        RiskReject::NotionalExceeded { .. }      => (3, 2),
        RiskReject::MaxOpenOrdersReached { .. }  => (4, 1),
        RiskReject::DailyLossLimitHit { .. }     => (5, 3),
        RiskReject::CircuitBreakerTripped        => (6, 3),
    };

    let mut buf = [0u8; 9];
    buf[0] = reject_code;
    buf[1..9].copy_from_slice(&timestamp_ns.to_le_bytes());
    let content_hash = fnv1a(&buf);

    RiskSemanticEvent {
        content_hash,
        reject_code,
        severity,
        timestamp_ns,
    }
}

// ── Bridge 4: Risk check result → Ledger order entry ─────────────────────

/// Ledger order entry derived from a risk check result on a `RiskLimits` snapshot.
///
/// Encodes approval status, position ceiling, and utilization so the ledger
/// layer can gate order entry without re-querying the risk-config store.
pub struct RiskLedgerEntry {
    /// FNV-1a hash over max_position bytes and limit_utilization bits bytes.
    pub content_hash: u64,
    /// True when `limit_utilization < 1.0` (the order is within limits).
    pub approved: bool,
    /// Maximum absolute position size from the originating `RiskLimits`.
    pub max_position: u64,
    /// Fraction of the position limit already consumed (0.0–1.0+).
    pub limit_utilization: f64,
    /// Routing priority: 1 when `limit_utilization > 0.8` (near-limit), else 2.
    pub priority: u8,
}

/// Convert a `RiskLimits` snapshot and the current `used_position` into a
/// `RiskLedgerEntry` for ledger order gating.
///
/// `limit_utilization` is `used_position as f64 / max_position as f64`, clamped
/// to a well-defined range only by the input values (no artificial clamping).
/// `approved` is `limit_utilization < 1.0`.
/// `priority` is 1 when `limit_utilization > 0.8`, otherwise 2.
///
/// `content_hash` is FNV-1a over `max_position.to_le_bytes()` concatenated
/// with the little-endian bytes of the f64 bit-pattern of `limit_utilization`.
#[inline]
pub fn risk_limits_to_ledger_entry(
    limits: &RiskLimits,
    used_position: u64,
) -> RiskLedgerEntry {
    let limit_utilization = used_position as f64 / limits.max_position as f64;
    let approved  = limit_utilization < 1.0;
    let priority  = if limit_utilization > 0.8 { 1 } else { 2 };

    // Hash input: max_position (8 bytes) || limit_utilization bits (8 bytes).
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&limits.max_position.to_le_bytes());
    buf[8..16].copy_from_slice(&limit_utilization.to_bits().to_le_bytes());
    let content_hash = fnv1a(&buf);

    RiskLedgerEntry {
        content_hash,
        approved,
        max_position: limits.max_position,
        limit_utilization,
        priority,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_risk::{RiskReject, RiskLimits};

    // ── Bridge 1: Risk Reject → Analytics ────────────────────────────────

    #[test]
    fn test_reject_to_analytics() {
        let ts: u64 = 1_700_000_000_000_000_000;

        let cases: &[(RiskReject, u8)] = &[
            (RiskReject::PositionLimitBreached { current: 100, after: 200, limit: 150 }, 1),
            (RiskReject::OrderSizeTooLarge { size: 500, limit: 200 }, 2),
            (RiskReject::NotionalExceeded { notional: 1_000_000, limit: 500_000 }, 3),
            (RiskReject::MaxOpenOrdersReached { count: 10, limit: 5 }, 4),
            (RiskReject::DailyLossLimitHit { loss: -50_000, limit: -10_000 }, 5),
            (RiskReject::CircuitBreakerTripped, 6),
        ];

        for (reject, expected_code) in cases {
            let event = risk_reject_to_analytics(reject, ts);

            assert_eq!(event.reject_code, *expected_code,
                "reject_code mismatch for variant {}", expected_code);
            assert_eq!(event.timestamp_ns, ts);
            assert_ne!(event.content_hash, 0,
                "content_hash must be non-zero for code {}", expected_code);

            // Hash must be deterministic across two calls with same inputs.
            let event2 = risk_reject_to_analytics(reject, ts);
            assert_eq!(event.content_hash, event2.content_hash,
                "content_hash must be deterministic for code {}", expected_code);
        }

        // Different timestamps must produce different hashes.
        let r = RiskReject::CircuitBreakerTripped;
        let h1 = risk_reject_to_analytics(&r, 1_000).content_hash;
        let h2 = risk_reject_to_analytics(&r, 2_000).content_hash;
        assert_ne!(h1, h2, "distinct timestamps must yield distinct hashes");
    }

    // ── Bridge 2: Risk Limits → Cache ────────────────────────────────────

    #[test]
    fn test_limits_to_cache() {
        let limits = RiskLimits {
            max_position:    10_000,
            max_order_size:  1_000,
            max_notional:    5_000_000,
            max_open_orders: 20,
            max_daily_loss:  -100_000,
        };

        let account_id: u64 = 0xABCD_EF01_2345_6789;
        let entry = risk_limits_to_cache(&limits, account_id);

        // All limit fields must be faithfully copied.
        assert_eq!(entry.max_position,    10_000);
        assert_eq!(entry.max_order_size,  1_000);
        assert_eq!(entry.max_notional,    5_000_000);
        assert_eq!(entry.max_open_orders, 20);
        assert_eq!(entry.max_daily_loss,  -100_000);

        // TTL must be exactly 3600 s.
        assert_eq!(entry.ttl_secs, 3600);

        // Hash must be non-zero and deterministic.
        assert_ne!(entry.content_hash, 0);
        let entry2 = risk_limits_to_cache(&limits, account_id);
        assert_eq!(entry.content_hash, entry2.content_hash);

        // Different account IDs must produce different hashes.
        let entry_other = risk_limits_to_cache(&limits, account_id + 1);
        assert_ne!(entry.content_hash, entry_other.content_hash,
            "distinct account_ids must yield distinct hashes");
    }

    // ── Bridge 3: Risk Reject → Semantic Telemetry ───────────────────────

    #[test]
    fn test_reject_to_semantic_high_severity() {
        let ts: u64 = 9_999_999_999_999;

        // CircuitBreakerTripped → severity 3.
        let cb = risk_reject_to_semantic(&RiskReject::CircuitBreakerTripped, ts);
        assert_eq!(cb.reject_code, 6);
        assert_eq!(cb.severity, 3);
        assert_eq!(cb.timestamp_ns, ts);
        assert_ne!(cb.content_hash, 0);

        // DailyLossLimitHit → severity 3.
        let dl = risk_reject_to_semantic(
            &RiskReject::DailyLossLimitHit { loss: -200_000, limit: -50_000 },
            ts,
        );
        assert_eq!(dl.reject_code, 5);
        assert_eq!(dl.severity, 3);
        assert_ne!(dl.content_hash, 0);

        // The two high-severity events must have different hashes (different codes).
        assert_ne!(cb.content_hash, dl.content_hash);
    }

    #[test]
    fn test_reject_to_semantic_low_severity() {
        let ts: u64 = 1_234_567_890;

        // OrderSizeTooLarge → severity 1.
        let os = risk_reject_to_semantic(
            &RiskReject::OrderSizeTooLarge { size: 999, limit: 500 },
            ts,
        );
        assert_eq!(os.reject_code, 2);
        assert_eq!(os.severity, 1);
        assert_eq!(os.timestamp_ns, ts);
        assert_ne!(os.content_hash, 0);

        // MaxOpenOrdersReached → severity 1.
        let mo = risk_reject_to_semantic(
            &RiskReject::MaxOpenOrdersReached { count: 50, limit: 10 },
            ts,
        );
        assert_eq!(mo.reject_code, 4);
        assert_eq!(mo.severity, 1);
        assert_ne!(mo.content_hash, 0);

        // PositionLimitBreached → severity 2 (medium, not low).
        let pl = risk_reject_to_semantic(
            &RiskReject::PositionLimitBreached { current: 200, after: 300, limit: 250 },
            ts,
        );
        assert_eq!(pl.reject_code, 1);
        assert_eq!(pl.severity, 2);

        // NotionalExceeded → severity 2 (medium, not low).
        let ne = risk_reject_to_semantic(
            &RiskReject::NotionalExceeded { notional: 2_000_000, limit: 1_000_000 },
            ts,
        );
        assert_eq!(ne.reject_code, 3);
        assert_eq!(ne.severity, 2);

        // All four events must have distinct hashes (distinct codes).
        assert_ne!(os.content_hash, mo.content_hash);
        assert_ne!(os.content_hash, pl.content_hash);
        assert_ne!(mo.content_hash, ne.content_hash);
    }

    // ── Bridge 4: Risk Limits → Ledger order entry ────────────────────────

    fn make_limits(max_position: u64) -> RiskLimits {
        RiskLimits {
            max_position,
            max_order_size:  100,
            max_notional:    10_000_000,
            max_open_orders: 50,
            max_daily_loss:  -500_000,
        }
    }

    #[test]
    fn test_limits_to_ledger_entry_approved_low_utilization() {
        // used=500, max=1000 → utilization=0.5, approved=true, priority=2.
        let limits = make_limits(1000);
        let entry = risk_limits_to_ledger_entry(&limits, 500);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.max_position, 1000);
        assert!((entry.limit_utilization - 0.5).abs() < f64::EPSILON);
        assert!(entry.approved, "utilization 0.5 < 1.0 must be approved");
        assert_eq!(entry.priority, 2, "utilization 0.5 <= 0.8 must have priority 2");
    }

    #[test]
    fn test_limits_to_ledger_entry_near_limit_high_priority() {
        // used=900, max=1000 → utilization=0.9 > 0.8, priority=1, still approved.
        let limits = make_limits(1000);
        let entry = risk_limits_to_ledger_entry(&limits, 900);

        assert_ne!(entry.content_hash, 0);
        assert!(entry.approved, "utilization 0.9 < 1.0 must be approved");
        assert_eq!(entry.priority, 1, "utilization 0.9 > 0.8 must have priority 1");
        assert!((entry.limit_utilization - 0.9).abs() < 1e-12);
    }

    #[test]
    fn test_limits_to_ledger_entry_at_limit_rejected() {
        // used=1000, max=1000 → utilization=1.0, approved=false, priority=1.
        let limits = make_limits(1000);
        let entry = risk_limits_to_ledger_entry(&limits, 1000);

        assert!(!entry.approved, "utilization 1.0 must not be approved");
        assert_eq!(entry.priority, 1, "utilization 1.0 > 0.8 must have priority 1");
    }

    #[test]
    fn test_limits_to_ledger_entry_over_limit_rejected() {
        // used=1500, max=1000 → utilization=1.5, approved=false, priority=1.
        let limits = make_limits(1000);
        let entry = risk_limits_to_ledger_entry(&limits, 1500);

        assert!(!entry.approved, "utilization 1.5 must not be approved");
        assert_eq!(entry.priority, 1);
    }

    #[test]
    fn test_limits_to_ledger_entry_deterministic() {
        let limits = make_limits(2000);
        let e1 = risk_limits_to_ledger_entry(&limits, 1200);
        let e2 = risk_limits_to_ledger_entry(&limits, 1200);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.limit_utilization, e2.limit_utilization);
    }

    #[test]
    fn test_limits_to_ledger_entry_different_positions_differ() {
        let limits = make_limits(1000);
        let e1 = risk_limits_to_ledger_entry(&limits, 100);
        let e2 = risk_limits_to_ledger_entry(&limits, 200);
        assert_ne!(e1.content_hash, e2.content_hash);
    }
}
