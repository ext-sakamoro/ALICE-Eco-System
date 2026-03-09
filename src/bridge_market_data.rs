//! MarketData bridges — ALICE-MarketData ↔ DB, Cache, Analytics, FIX, Monitor
//!
//! 5 bridges connecting market data feed processing to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: MarketData → DB (tick storage) ─────────────────────────────

/// Tick storage record for ALICE-DB persistence.
pub struct MarketDataDbRecord {
    /// Content hash over the record metadata.
    pub content_hash: u64,
    /// Hash of the trading symbol.
    pub symbol_hash: u64,
    /// Total tick count stored.
    pub tick_count: u64,
    /// Number of OHLC bars derived from ticks.
    pub ohlc_count: u32,
    /// Hash of the data source identifier.
    pub source_hash: u64,
    /// Record timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Serialize market data tick metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn market_data_to_db_record(
    symbol_hash: u64,
    tick_count: u64,
    ohlc_count: u32,
    source_hash: u64,
    timestamp_ms: u64,
) -> MarketDataDbRecord {
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tick_count.to_le_bytes());
    buf[16..20].copy_from_slice(&ohlc_count.to_le_bytes());
    buf[20..28].copy_from_slice(&source_hash.to_le_bytes());
    buf[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    MarketDataDbRecord {
        content_hash: fnv1a(&buf),
        symbol_hash,
        tick_count,
        ohlc_count,
        source_hash,
        timestamp_ms,
    }
}

// ── Bridge 2: MarketData → Cache (quote cache) ───────────────────────────

/// Latest quote cache entry for ALICE-Cache.
pub struct MarketDataCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Hash of the trading symbol.
    pub symbol_hash: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Number of ticks in this cache window.
    pub tick_count: u64,
    /// Last price scaled by 100 (cents).
    pub last_price_x100: u64,
}

/// Build a market data quote cache entry for ALICE-Cache.
///
/// Symbols with recent ticks (tick_count > 0) get a short TTL (5 s) to stay
/// fresh; symbols with no ticks get 60 s to avoid hammering the feed.
#[inline]
#[must_use]
pub fn market_data_to_cache_entry(
    symbol_hash: u64,
    tick_count: u64,
    last_price_x100: u64,
) -> MarketDataCacheEntry {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tick_count.to_le_bytes());
    buf[16..24].copy_from_slice(&last_price_x100.to_le_bytes());
    let has_ticks = (tick_count > 0) as u32;
    let ttl_secs = 60 - has_ticks * 55;
    MarketDataCacheEntry {
        content_hash: fnv1a(&buf),
        symbol_hash,
        ttl_secs,
        tick_count,
        last_price_x100,
    }
}

// ── Bridge 3: MarketData → Analytics (feed event) ────────────────────────

/// Market data feed analytics event for ALICE-Analytics.
pub struct MarketDataAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Hash of the trading symbol.
    pub symbol_hash: u64,
    /// Cumulative traded volume.
    pub volume: u64,
    /// Bid-ask spread in basis points.
    pub spread_bps: u16,
    /// Feed latency in microseconds.
    pub latency_us: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a market data feed analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn market_data_to_analytics_event(
    symbol_hash: u64,
    volume: u64,
    spread_bps: u16,
    latency_us: u64,
    timestamp_ms: u64,
) -> MarketDataAnalyticsEvent {
    let mut buf = [0u8; 34];
    buf[0..8].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&volume.to_le_bytes());
    buf[16..18].copy_from_slice(&spread_bps.to_le_bytes());
    buf[18..26].copy_from_slice(&latency_us.to_le_bytes());
    buf[26..34].copy_from_slice(&timestamp_ms.to_le_bytes());
    MarketDataAnalyticsEvent {
        content_hash: fnv1a(&buf),
        symbol_hash,
        volume,
        spread_bps,
        latency_us,
        timestamp_ms,
    }
}

// ── Bridge 4: MarketData → FIX (message link) ────────────────────────────

/// FIX protocol message link for ALICE-FIX integration.
pub struct MarketDataFixLink {
    /// Content hash over the FIX message envelope.
    pub content_hash: u64,
    /// Hash of the trading symbol.
    pub symbol_hash: u64,
    /// FIX message type byte (e.g. b'W' = MarketDataSnapshot).
    pub msg_type: u8,
    /// FIX sequence number.
    pub seq_num: u64,
    /// Hash of the sender comp ID.
    pub sender_hash: u64,
}

/// Build a market data FIX message link for ALICE-FIX.
#[inline]
#[must_use]
pub fn market_data_to_fix_link(
    symbol_hash: u64,
    msg_type: u8,
    seq_num: u64,
    sender_hash: u64,
) -> MarketDataFixLink {
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[8] = msg_type;
    buf[9..17].copy_from_slice(&seq_num.to_le_bytes());
    buf[17..25].copy_from_slice(&sender_hash.to_le_bytes());
    MarketDataFixLink {
        content_hash: fnv1a(&buf),
        symbol_hash,
        msg_type,
        seq_num,
        sender_hash,
    }
}

// ── Bridge 5: MarketData → Monitor (feed health) ─────────────────────────

/// Feed health monitor status for ALICE-Monitor dashboards.
pub struct MarketDataMonitorStatus {
    /// Content hash over the status tuple.
    pub content_hash: u64,
    /// Total number of active feeds.
    pub feed_count: u32,
    /// Number of feeds with stale data.
    pub stale_count: u32,
    /// Number of sequence gaps detected.
    pub gap_count: u32,
    /// Whether all feeds are within healthy thresholds.
    pub is_healthy: bool,
    /// Status timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a market data feed health monitor status for ALICE-Monitor.
#[inline]
#[must_use]
pub fn market_data_to_monitor_status(
    feed_count: u32,
    stale_count: u32,
    gap_count: u32,
    is_healthy: bool,
    timestamp_ms: u64,
) -> MarketDataMonitorStatus {
    let mut buf = [0u8; 21];
    buf[0..4].copy_from_slice(&feed_count.to_le_bytes());
    buf[4..8].copy_from_slice(&stale_count.to_le_bytes());
    buf[8..12].copy_from_slice(&gap_count.to_le_bytes());
    buf[12] = is_healthy as u8;
    buf[13..21].copy_from_slice(&timestamp_ms.to_le_bytes());
    MarketDataMonitorStatus {
        content_hash: fnv1a(&buf),
        feed_count,
        stale_count,
        gap_count,
        is_healthy,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_to_db_record_hash_nonzero() {
        let rec = market_data_to_db_record(0x1111, 10_000, 240, 0x2222, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_market_data_to_db_record_fields() {
        let rec = market_data_to_db_record(0xabcd, 500, 10, 0xef01, 9_999_999);
        assert_eq!(rec.symbol_hash, 0xabcd);
        assert_eq!(rec.tick_count, 500);
        assert_eq!(rec.ohlc_count, 10);
        assert_eq!(rec.source_hash, 0xef01);
        assert_eq!(rec.timestamp_ms, 9_999_999);
    }

    #[test]
    fn test_market_data_to_db_record_deterministic() {
        let a = market_data_to_db_record(1, 2, 3, 4, 5);
        let b = market_data_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_market_data_to_cache_entry_no_ticks_ttl() {
        let entry = market_data_to_cache_entry(0x1234, 0, 10_000);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_market_data_to_cache_entry_with_ticks_ttl() {
        let entry = market_data_to_cache_entry(0x5678, 100, 20_000);
        assert_eq!(entry.ttl_secs, 5);
        assert_eq!(entry.tick_count, 100);
        assert_eq!(entry.last_price_x100, 20_000);
    }

    #[test]
    fn test_market_data_to_analytics_event() {
        let ev = market_data_to_analytics_event(0xbeef, 1_000_000, 25, 120, 1_700_000_000_001);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.volume, 1_000_000);
        assert_eq!(ev.spread_bps, 25);
        assert_eq!(ev.latency_us, 120);
    }

    #[test]
    fn test_market_data_to_fix_link() {
        let link = market_data_to_fix_link(0xc0de, b'W', 42, 0xface);
        assert_ne!(link.content_hash, 0);
        assert_eq!(link.msg_type, b'W');
        assert_eq!(link.seq_num, 42);
        assert_eq!(link.sender_hash, 0xface);
    }

    #[test]
    fn test_market_data_to_monitor_status_healthy() {
        let ms = market_data_to_monitor_status(10, 0, 0, true, 1_700_000_000_002);
        assert_ne!(ms.content_hash, 0);
        assert!(ms.is_healthy);
        assert_eq!(ms.feed_count, 10);
        assert_eq!(ms.stale_count, 0);
    }

    #[test]
    fn test_market_data_to_monitor_status_unhealthy() {
        let ms = market_data_to_monitor_status(10, 3, 5, false, 1_700_000_000_003);
        assert!(!ms.is_healthy);
        assert_eq!(ms.stale_count, 3);
        assert_eq!(ms.gap_count, 5);
    }
}
