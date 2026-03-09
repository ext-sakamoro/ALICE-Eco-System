//! Quant bridges — ALICE-Quant ↔ DB, Cache, Analytics, Risk, Monitor
//!
//! 5 bridges connecting quantitative strategy processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Quant → DB (strategy storage) ──────────────────────────────

/// Strategy storage record for ALICE-DB persistence.
pub struct QuantDbRecord {
    /// Content hash over the strategy metadata.
    pub content_hash: u64,
    /// Hash of the strategy identifier.
    pub strategy_hash: u64,
    /// Number of open positions.
    pub position_count: u32,
    /// Profit and loss scaled by 100 (cents).
    pub pnl_x100: i64,
    /// Sharpe ratio scaled by 1000.
    pub sharpe_x1000: i32,
    /// Maximum drawdown in basis points.
    pub drawdown_bps: u16,
}

/// Serialize quant strategy metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn quant_to_db_record(
    strategy_hash: u64,
    position_count: u32,
    pnl_x100: i64,
    sharpe_x1000: i32,
    drawdown_bps: u16,
) -> QuantDbRecord {
    let mut buf = [0u8; 30];
    buf[0..8].copy_from_slice(&strategy_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&position_count.to_le_bytes());
    buf[12..20].copy_from_slice(&pnl_x100.to_le_bytes());
    buf[20..24].copy_from_slice(&sharpe_x1000.to_le_bytes());
    buf[24..26].copy_from_slice(&drawdown_bps.to_le_bytes());
    QuantDbRecord {
        content_hash: fnv1a(&buf),
        strategy_hash,
        position_count,
        pnl_x100,
        sharpe_x1000,
        drawdown_bps,
    }
}

// ── Bridge 2: Quant → Cache (signal cache) ───────────────────────────────

/// Signal cache entry for ALICE-Cache.
pub struct QuantCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the strategy identifier.
    pub strategy_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Number of active signals cached.
    pub signal_count: u32,
    /// Model version that produced the signals.
    pub model_version: u32,
}

/// Build a quant signal cache entry for ALICE-Cache.
///
/// Active strategies (signal_count > 0) receive a shorter TTL (60 s) because
/// signals are time-sensitive; idle strategies receive 300 s.
#[inline]
#[must_use]
pub fn quant_to_cache_entry(
    strategy_hash: u64,
    signal_count: u32,
    model_version: u32,
) -> QuantCacheEntry {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&strategy_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&signal_count.to_le_bytes());
    buf[12..16].copy_from_slice(&model_version.to_le_bytes());
    let has_signals = (signal_count > 0) as u32;
    let ttl_secs = 300 - has_signals * 240;
    QuantCacheEntry {
        content_hash: fnv1a(&buf),
        strategy_hash,
        ttl_secs,
        signal_count,
        model_version,
    }
}

// ── Bridge 3: Quant → Analytics (performance event) ──────────────────────

/// Trading performance analytics event for ALICE-Analytics.
pub struct QuantAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Total number of trades executed.
    pub trade_count: u64,
    /// Profit and loss scaled by 100 (cents).
    pub pnl_x100: i64,
    /// Win rate in basis points.
    pub win_rate_bps: u16,
    /// Average order latency in microseconds.
    pub avg_latency_us: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a quant performance analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn quant_to_analytics_event(
    trade_count: u64,
    pnl_x100: i64,
    win_rate_bps: u16,
    avg_latency_us: u64,
    timestamp_ms: u64,
) -> QuantAnalyticsEvent {
    let mut buf = [0u8; 34];
    buf[0..8].copy_from_slice(&trade_count.to_le_bytes());
    buf[8..16].copy_from_slice(&pnl_x100.to_le_bytes());
    buf[16..18].copy_from_slice(&win_rate_bps.to_le_bytes());
    buf[18..26].copy_from_slice(&avg_latency_us.to_le_bytes());
    buf[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    QuantAnalyticsEvent {
        content_hash: fnv1a(&buf),
        trade_count,
        pnl_x100,
        win_rate_bps,
        avg_latency_us,
        timestamp_ms,
    }
}

// ── Bridge 4: Quant → Risk (risk metrics) ────────────────────────────────

/// Risk metrics snapshot for ALICE-Risk integration.
pub struct QuantRiskMetrics {
    /// Content hash over the risk tuple.
    pub content_hash: u64,
    /// Value-at-risk scaled by 100 (cents).
    pub var_x100: i64,
    /// Maximum drawdown in basis points.
    pub max_drawdown_bps: u16,
    /// Number of open positions contributing to exposure.
    pub position_count: u32,
    /// Total market exposure scaled by 100 (cents).
    pub exposure_x100: i64,
}

/// Build a quant risk metrics snapshot for ALICE-Risk.
#[inline]
#[must_use]
pub fn quant_to_risk_metrics(
    var_x100: i64,
    max_drawdown_bps: u16,
    position_count: u32,
    exposure_x100: i64,
) -> QuantRiskMetrics {
    let mut buf = [0u8; 22];
    buf[0..8].copy_from_slice(&var_x100.to_le_bytes());
    buf[8..10].copy_from_slice(&max_drawdown_bps.to_le_bytes());
    buf[10..14].copy_from_slice(&position_count.to_le_bytes());
    buf[14..22].copy_from_slice(&exposure_x100.to_le_bytes());
    QuantRiskMetrics {
        content_hash: fnv1a(&buf),
        var_x100,
        max_drawdown_bps,
        position_count,
        exposure_x100,
    }
}

// ── Bridge 5: Quant → Monitor (strategy status) ──────────────────────────

/// Strategy monitor status for ALICE-Monitor dashboards.
pub struct QuantMonitorStatus {
    /// Content hash over the status tuple.
    pub content_hash: u64,
    /// Hash of the strategy identifier.
    pub strategy_hash: u64,
    /// Whether the strategy is currently active.
    pub is_active: bool,
    /// Current profit and loss scaled by 100 (cents).
    pub pnl_x100: i64,
    /// Number of signals generated in this period.
    pub signal_count: u32,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a quant strategy monitor status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn quant_to_monitor_status(
    strategy_hash: u64,
    is_active: bool,
    pnl_x100: i64,
    signal_count: u32,
    timestamp_ms: u64,
) -> QuantMonitorStatus {
    let mut buf = [0u8; 29];
    buf[0..8].copy_from_slice(&strategy_hash.to_le_bytes());
    buf[8] = is_active as u8;
    buf[9..17].copy_from_slice(&pnl_x100.to_le_bytes());
    buf[17..21].copy_from_slice(&signal_count.to_le_bytes());
    buf[21..29].copy_from_slice(&timestamp_ms.to_le_bytes());
    QuantMonitorStatus {
        content_hash: fnv1a(&buf),
        strategy_hash,
        is_active,
        pnl_x100,
        signal_count,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quant_to_db_record_hash_nonzero() {
        let rec = quant_to_db_record(0xdead_beef_cafe_1234, 5, 100_00, 1_500, 200);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_quant_to_db_record_fields() {
        let rec = quant_to_db_record(0xaaaa_bbbb_cccc_dddd, 3, -50_00, -300, 150);
        assert_eq!(rec.strategy_hash, 0xaaaa_bbbb_cccc_dddd);
        assert_eq!(rec.position_count, 3);
        assert_eq!(rec.pnl_x100, -50_00);
        assert_eq!(rec.sharpe_x1000, -300);
        assert_eq!(rec.drawdown_bps, 150);
    }

    #[test]
    fn test_quant_to_db_record_deterministic() {
        let a = quant_to_db_record(1, 2, 3, 4, 5);
        let b = quant_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_quant_to_cache_entry_idle_ttl() {
        let entry = quant_to_cache_entry(0x1111, 0, 1);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_quant_to_cache_entry_active_ttl() {
        let entry = quant_to_cache_entry(0x2222, 10, 2);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.signal_count, 10);
    }

    #[test]
    fn test_quant_to_analytics_event() {
        let ev = quant_to_analytics_event(500, 25_000_00, 6_000, 350, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.trade_count, 500);
        assert_eq!(ev.win_rate_bps, 6_000);
        assert_eq!(ev.avg_latency_us, 350);
    }

    #[test]
    fn test_quant_to_risk_metrics() {
        let rm = quant_to_risk_metrics(-5_000_00, 800, 12, 1_000_000_00);
        assert_ne!(rm.content_hash, 0);
        assert_eq!(rm.var_x100, -5_000_00);
        assert_eq!(rm.max_drawdown_bps, 800);
        assert_eq!(rm.position_count, 12);
        assert_eq!(rm.exposure_x100, 1_000_000_00);
    }

    #[test]
    fn test_quant_to_monitor_status() {
        let ms = quant_to_monitor_status(0xface, true, 9_999_00, 7, 1_700_000_000_001);
        assert_ne!(ms.content_hash, 0);
        assert!(ms.is_active);
        assert_eq!(ms.pnl_x100, 9_999_00);
        assert_eq!(ms.signal_count, 7);
    }

    #[test]
    fn test_quant_to_monitor_status_inactive() {
        let ms = quant_to_monitor_status(0xface, false, 0, 0, 0);
        assert!(!ms.is_active);
        assert_eq!(ms.signal_count, 0);
    }
}
