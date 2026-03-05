//! FIX bridges — ALICE-FIX ↔ Analytics, DB, Cache, Edge
//!
//! 5 bridges connecting ALICE-FIX order management, execution reporting,
//! and market data to the ALICE ecosystem.
//!
//! These bridges focus on order-level and market-data structures, complementing
//! `bridge_fix_ext` which covers message-level analytics, fill-to-exec conversion,
//! session telemetry, and message-to-risk input.

use alice_fix::tag;
use alice_fix::FixMessage;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: NewOrderSingle → Analytics (order submission event) ───────

/// Order submission analytics event derived from a FIX `NewOrderSingle` (35=D).
///
/// Extracts the core order fields for analytics dashboards tracking order flow,
/// symbol distribution, and side bias.
pub struct FixOrderAnalyticsEvent {
    /// FNV-1a content hash over `ClOrdID`, symbol hash, side, order type, qty, and price.
    pub content_hash: u64,
    /// FNV-1a hash of the `ClOrdID` (tag 11) string, or 0 if absent.
    pub cl_ord_id_hash: u64,
    /// FNV-1a hash of the Symbol (tag 55) string, or 0 if absent.
    pub symbol_hash: u64,
    /// Side byte: b'1' = Buy, b'2' = Sell, 0 if absent.
    pub side: u8,
    /// `OrdType` byte: b'1' = Market, b'2' = Limit, 0 if absent.
    pub ord_type: u8,
    /// Order quantity from tag 38, or 0 if absent.
    pub quantity: u64,
    /// Limit price from tag 44 parsed as f64, or 0.0 if absent.
    pub price: f64,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a parsed FIX `NewOrderSingle` into an order analytics event.
///
/// Fields are extracted from the standard FIX tags (11, 55, 54, 40, 38, 44).
/// Missing fields default to zero/empty.
#[inline]
#[must_use]
pub fn fix_order_to_analytics(msg: &FixMessage, timestamp_ns: u64) -> FixOrderAnalyticsEvent {
    let cl_ord_id_hash = msg.get(tag::CL_ORD_ID).map_or(0, |s| fnv1a(s.as_bytes()));
    let symbol_hash = msg.get(tag::SYMBOL).map_or(0, |s| fnv1a(s.as_bytes()));
    let side: u8 = msg
        .get(tag::SIDE)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let ord_type: u8 = msg
        .get(tag::ORD_TYPE)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let quantity: u64 = msg.get_u64(tag::ORDER_QTY).unwrap_or(0);
    let price: f64 = msg
        .get(tag::PRICE)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Hash input: cl_ord_id_hash(8) || symbol_hash(8) || side(1) || ord_type(1)
    //           || quantity(8) || price bits(8) = 34 bytes
    let mut buf = [0u8; 34];
    buf[0..8].copy_from_slice(&cl_ord_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[16] = side;
    buf[17] = ord_type;
    buf[18..26].copy_from_slice(&quantity.to_le_bytes());
    buf[26..34].copy_from_slice(&price.to_bits().to_le_bytes());

    FixOrderAnalyticsEvent {
        content_hash: fnv1a(&buf),
        cl_ord_id_hash,
        symbol_hash,
        side,
        ord_type,
        quantity,
        price,
        timestamp_ns,
    }
}

// ── Bridge 2: ExecutionReport → DB (execution record) ───────────────────

/// Execution report DB record derived from a FIX `ExecutionReport` (35=8).
///
/// Captures fill price, fill quantity, cumulative quantity, and order status
/// for the DB audit trail.
pub struct FixExecReportDbRecord {
    /// FNV-1a content hash over `exec_id_hash`, `order_id_hash`, `last_px`, `last_qty`.
    pub content_hash: u64,
    /// FNV-1a hash of `ExecID` (tag 17), or 0 if absent.
    pub exec_id_hash: u64,
    /// FNV-1a hash of `OrderID` (tag 37), or 0 if absent.
    pub order_id_hash: u64,
    /// `ExecType` (tag 150) first byte, or 0 if absent.
    pub exec_type: u8,
    /// `OrdStatus` (tag 39) first byte, or 0 if absent.
    pub ord_status: u8,
    /// Last fill price (tag 31) as f64, or 0.0.
    pub last_px: f64,
    /// Last fill quantity (tag 32), or 0.
    pub last_qty: u64,
    /// Cumulative filled quantity (tag 14), or 0.
    pub cum_qty: u64,
    /// Leaves quantity (tag 151), or 0.
    pub leaves_qty: u64,
}

/// Convert a parsed FIX `ExecutionReport` into a DB record.
///
/// Fields are extracted from tags 17, 37, 150, 39, 31, 32, 14, 151.
#[inline]
#[must_use]
pub fn fix_exec_report_to_db(msg: &FixMessage) -> FixExecReportDbRecord {
    let exec_id_hash = msg.get(tag::EXEC_ID).map_or(0, |s| fnv1a(s.as_bytes()));
    let order_id_hash = msg.get(tag::ORDER_ID).map_or(0, |s| fnv1a(s.as_bytes()));
    let exec_type: u8 = msg
        .get(tag::EXEC_TYPE)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let ord_status: u8 = msg
        .get(tag::ORD_STATUS)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let last_px: f64 = msg
        .get(tag::LAST_PX)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let last_qty: u64 = msg.get_u64(tag::LAST_QTY).unwrap_or(0);
    let cum_qty: u64 = msg.get_u64(tag::CUM_QTY).unwrap_or(0);
    let leaves_qty: u64 = msg.get_u64(tag::LEAVES_QTY).unwrap_or(0);

    // Hash input: exec_id_hash(8) || order_id_hash(8) || last_px bits(8) || last_qty(8) = 32 bytes
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&exec_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&order_id_hash.to_le_bytes());
    buf[16..24].copy_from_slice(&last_px.to_bits().to_le_bytes());
    buf[24..32].copy_from_slice(&last_qty.to_le_bytes());

    FixExecReportDbRecord {
        content_hash: fnv1a(&buf),
        exec_id_hash,
        order_id_hash,
        exec_type,
        ord_status,
        last_px,
        last_qty,
        cum_qty,
        leaves_qty,
    }
}

// ── Bridge 3: Order → Cache (order status cache) ────────────────────────

/// Order status cache entry for ALICE-Cache.
///
/// Caches the current status of an order derived from a FIX message so the
/// order-entry UI can display status without re-querying the exchange.
/// TTL is branchless: terminal statuses (filled, cancelled) get long TTL;
/// active statuses get short TTL for frequent refresh.
pub struct FixOrderCacheEntry {
    /// FNV-1a content hash over `cl_ord_id_hash` and `ord_status`.
    pub content_hash: u64,
    /// FNV-1a hash of `ClOrdID` (tag 11).
    pub cl_ord_id_hash: u64,
    /// `OrdStatus` byte (tag 39): '0'=New, '1'=`PartialFill`, '2'=Fill, '4'=Cancelled, etc.
    pub ord_status: u8,
    /// Leaves quantity (tag 151), or 0.
    pub leaves_qty: u64,
    /// Cumulative filled quantity (tag 14), or 0.
    pub cum_qty: u64,
    /// Cache TTL in seconds. Branchless: terminal=3600s, active=5s.
    pub ttl_secs: u32,
}

/// Convert a FIX message carrying order status into a cache entry.
///
/// Branchless TTL: `3600 - is_active * 3595`.
/// Terminal statuses ('2'=Fill, '4'=Cancelled, '8'=Rejected, 'C'=Expired)
/// are cached for 3600s; active statuses are cached for 5s.
#[inline]
#[must_use]
pub fn fix_order_to_cache(msg: &FixMessage) -> FixOrderCacheEntry {
    let cl_ord_id_hash = msg.get(tag::CL_ORD_ID).map_or(0, |s| fnv1a(s.as_bytes()));
    let ord_status: u8 = msg
        .get(tag::ORD_STATUS)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let leaves_qty: u64 = msg.get_u64(tag::LEAVES_QTY).unwrap_or(0);
    let cum_qty: u64 = msg.get_u64(tag::CUM_QTY).unwrap_or(0);

    // Terminal statuses: '2' (Fill), '4' (Cancelled), '8' (Rejected), 'C' (Expired)
    let is_terminal =
        (ord_status == b'2') | (ord_status == b'4') | (ord_status == b'8') | (ord_status == b'C');
    // Branchless TTL: terminal → 3600, active → 5
    let is_active = (!is_terminal) as u32;
    let ttl_secs = 3600 - is_active * 3595;

    // Hash input: cl_ord_id_hash(8) || ord_status(1) = 9 bytes
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&cl_ord_id_hash.to_le_bytes());
    buf[8] = ord_status;

    FixOrderCacheEntry {
        content_hash: fnv1a(&buf),
        cl_ord_id_hash,
        ord_status,
        leaves_qty,
        cum_qty,
        ttl_secs,
    }
}

// ── Bridge 4: MarketData → Edge (market data snapshot) ──────────────────

/// Market data edge event for ALICE-Edge.
///
/// Captures top-of-book data from a FIX `MarketDataSnapshotFullRefresh` (35=W)
/// or similar market data message. Fields are extracted from standard tags.
pub struct FixMarketDataEdgeEvent {
    /// FNV-1a content hash over `symbol_hash`, `last_px` bits, and timestamp.
    pub content_hash: u64,
    /// FNV-1a hash of the symbol (tag 55).
    pub symbol_hash: u64,
    /// Last traded price (tag 31) as f64, or 0.0.
    pub last_px: f64,
    /// Last traded quantity (tag 32), or 0.
    pub last_qty: u64,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a FIX market data message into an edge event.
///
/// Extracts Symbol (55), `LastPx` (31), `LastQty` (32).
#[inline]
#[must_use]
pub fn fix_market_data_to_edge(msg: &FixMessage, timestamp_ns: u64) -> FixMarketDataEdgeEvent {
    let symbol_hash = msg.get(tag::SYMBOL).map_or(0, |s| fnv1a(s.as_bytes()));
    let last_px: f64 = msg
        .get(tag::LAST_PX)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let last_qty: u64 = msg.get_u64(tag::LAST_QTY).unwrap_or(0);

    // Hash input: symbol_hash(8) || last_px bits(8) || timestamp(8) = 24 bytes
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&symbol_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&last_px.to_bits().to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());

    FixMarketDataEdgeEvent {
        content_hash: fnv1a(&buf),
        symbol_hash,
        last_px,
        last_qty,
        timestamp_ns,
    }
}

// ── Bridge 5: Session state → Analytics (session health metric) ─────────

/// Session health metric for ALICE-Analytics.
///
/// Tracks the session state transitions, sequence number deltas, and
/// message throughput for monitoring session health.
pub struct FixSessionAnalyticsMetric {
    /// FNV-1a content hash over `sender_hash`, `target_hash`, `state_disc`, and timestamp.
    pub content_hash: u64,
    /// FNV-1a hash of sender comp ID from the message (tag 49), or 0.
    pub sender_hash: u64,
    /// FNV-1a hash of target comp ID from the message (tag 56), or 0.
    pub target_hash: u64,
    /// Session state discriminant derived from the message type:
    /// 0=unknown, 1=Logon(A), 2=Logout(5), 3=Heartbeat(0), 4=NewOrder(D), 5=ExecReport(8).
    pub msg_type_disc: u8,
    /// Message sequence number (tag 34), or 0 if absent.
    pub msg_seq_num: u64,
    /// Timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a FIX message into a session health analytics metric.
///
/// Extracts `SenderCompID` (49), `TargetCompID` (56), `MsgSeqNum` (34), and
/// classifies the `MsgType` into a compact discriminant.
#[inline]
#[must_use]
pub fn fix_session_to_analytics(msg: &FixMessage, timestamp_ns: u64) -> FixSessionAnalyticsMetric {
    let sender_hash = msg
        .get(tag::SENDER_COMP_ID)
        .map_or(0, |s| fnv1a(s.as_bytes()));
    let target_hash = msg
        .get(tag::TARGET_COMP_ID)
        .map_or(0, |s| fnv1a(s.as_bytes()));
    let msg_seq_num = msg.get_u64(tag::MSG_SEQ_NUM).unwrap_or(0);

    let msg_type_disc: u8 = match msg.msg_type.as_str() {
        "A" => 1, // Logon
        "5" => 2, // Logout
        "0" => 3, // Heartbeat
        "D" => 4, // NewOrderSingle
        "8" => 5, // ExecutionReport
        _ => 0,   // Unknown
    };

    // Hash input: sender_hash(8) || target_hash(8) || msg_type_disc(1) || timestamp(8) = 25 bytes
    let mut buf = [0u8; 25];
    buf[0..8].copy_from_slice(&sender_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&target_hash.to_le_bytes());
    buf[16] = msg_type_disc;
    buf[17..25].copy_from_slice(&timestamp_ns.to_le_bytes());

    FixSessionAnalyticsMetric {
        content_hash: fnv1a(&buf),
        sender_hash,
        target_hash,
        msg_type_disc,
        msg_seq_num,
        timestamp_ns,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use alice_fix::FixMessage;

    fn make_new_order(symbol: &str, side: &str, qty: &str, price: &str) -> FixMessage {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(tag::CL_ORD_ID, "ORD-001");
        msg.set(tag::SYMBOL, symbol);
        msg.set(tag::SIDE, side);
        msg.set(tag::ORD_TYPE, "2"); // Limit
        msg.set(tag::ORDER_QTY, qty);
        msg.set(tag::PRICE, price);
        msg
    }

    fn make_exec_report() -> FixMessage {
        let mut msg = FixMessage::new("FIX.4.4", "8");
        msg.set(tag::EXEC_ID, "EXEC-001");
        msg.set(tag::ORDER_ID, "ORD-001");
        msg.set(tag::EXEC_TYPE, "F"); // Trade
        msg.set(tag::ORD_STATUS, "2"); // Filled
        msg.set(tag::LAST_PX, "50000.0");
        msg.set(tag::LAST_QTY, "10");
        msg.set(tag::CUM_QTY, "10");
        msg.set(tag::LEAVES_QTY, "0");
        msg
    }

    // ── Bridge 1: Order → Analytics ────────────────────────────────────

    #[test]
    fn test_order_to_analytics_basic() {
        let msg = make_new_order("BTCUSD", "1", "100", "50000.0");
        let ts = 1_700_000_000_000_000_000u64;
        let ev = fix_order_to_analytics(&msg, ts);

        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.cl_ord_id_hash, 0);
        assert_ne!(ev.symbol_hash, 0);
        assert_eq!(ev.side, b'1');
        assert_eq!(ev.ord_type, b'2');
        assert_eq!(ev.quantity, 100);
        assert_eq!(ev.price, 50000.0);
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_order_to_analytics_deterministic() {
        let msg = make_new_order("ETHUSD", "2", "50", "3000.0");
        let ev1 = fix_order_to_analytics(&msg, 100);
        let ev2 = fix_order_to_analytics(&msg, 100);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.symbol_hash, ev2.symbol_hash);
    }

    #[test]
    fn test_order_to_analytics_different_symbols_differ() {
        let m1 = make_new_order("BTCUSD", "1", "10", "50000.0");
        let m2 = make_new_order("ETHUSD", "1", "10", "50000.0");
        let e1 = fix_order_to_analytics(&m1, 0);
        let e2 = fix_order_to_analytics(&m2, 0);
        assert_ne!(e1.symbol_hash, e2.symbol_hash);
        assert_ne!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 2: ExecReport → DB ──────────────────────────────────────

    #[test]
    fn test_exec_report_to_db_basic() {
        let msg = make_exec_report();
        let rec = fix_exec_report_to_db(&msg);

        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.exec_id_hash, 0);
        assert_ne!(rec.order_id_hash, 0);
        assert_eq!(rec.exec_type, b'F');
        assert_eq!(rec.ord_status, b'2');
        assert_eq!(rec.last_px, 50000.0);
        assert_eq!(rec.last_qty, 10);
        assert_eq!(rec.cum_qty, 10);
        assert_eq!(rec.leaves_qty, 0);
    }

    #[test]
    fn test_exec_report_to_db_empty_message() {
        let msg = FixMessage::new("FIX.4.4", "8");
        let rec = fix_exec_report_to_db(&msg);

        assert_eq!(rec.exec_id_hash, 0);
        assert_eq!(rec.order_id_hash, 0);
        assert_eq!(rec.exec_type, 0);
        assert_eq!(rec.last_px, 0.0);
    }

    // ── Bridge 3: Order → Cache ────────────────────────────────────────

    #[test]
    fn test_order_to_cache_terminal_status() {
        let mut msg = FixMessage::new("FIX.4.4", "8");
        msg.set(tag::CL_ORD_ID, "ORD-100");
        msg.set(tag::ORD_STATUS, "2"); // Filled = terminal
        msg.set(tag::LEAVES_QTY, "0");
        msg.set(tag::CUM_QTY, "50");

        let entry = fix_order_to_cache(&msg);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ord_status, b'2');
        assert_eq!(entry.leaves_qty, 0);
        assert_eq!(entry.cum_qty, 50);
        assert_eq!(entry.ttl_secs, 3600); // Terminal = long TTL
    }

    #[test]
    fn test_order_to_cache_active_status() {
        let mut msg = FixMessage::new("FIX.4.4", "8");
        msg.set(tag::CL_ORD_ID, "ORD-200");
        msg.set(tag::ORD_STATUS, "0"); // New = active
        msg.set(tag::LEAVES_QTY, "100");
        msg.set(tag::CUM_QTY, "0");

        let entry = fix_order_to_cache(&msg);

        assert_eq!(entry.ord_status, b'0');
        assert_eq!(entry.ttl_secs, 5); // Active = short TTL
    }

    #[test]
    fn test_order_to_cache_cancelled_is_terminal() {
        let mut msg = FixMessage::new("FIX.4.4", "8");
        msg.set(tag::CL_ORD_ID, "ORD-300");
        msg.set(tag::ORD_STATUS, "4"); // Cancelled = terminal

        let entry = fix_order_to_cache(&msg);
        assert_eq!(entry.ttl_secs, 3600);
    }

    // ── Bridge 4: MarketData → Edge ────────────────────────────────────

    #[test]
    fn test_market_data_to_edge_basic() {
        let mut msg = FixMessage::new("FIX.4.4", "W");
        msg.set(tag::SYMBOL, "BTCUSD");
        msg.set(tag::LAST_PX, "51000.5");
        msg.set(tag::LAST_QTY, "25");

        let ts = 2_000_000_000u64;
        let ev = fix_market_data_to_edge(&msg, ts);

        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.symbol_hash, 0);
        assert_eq!(ev.last_px, 51000.5);
        assert_eq!(ev.last_qty, 25);
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_market_data_to_edge_deterministic() {
        let mut msg = FixMessage::new("FIX.4.4", "W");
        msg.set(tag::SYMBOL, "SOLUSD");
        msg.set(tag::LAST_PX, "150.0");
        msg.set(tag::LAST_QTY, "1000");

        let e1 = fix_market_data_to_edge(&msg, 99);
        let e2 = fix_market_data_to_edge(&msg, 99);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_market_data_to_edge_different_timestamps_differ() {
        let mut msg = FixMessage::new("FIX.4.4", "W");
        msg.set(tag::SYMBOL, "XRPUSD");
        msg.set(tag::LAST_PX, "0.5");

        let e1 = fix_market_data_to_edge(&msg, 1000);
        let e2 = fix_market_data_to_edge(&msg, 2000);
        assert_ne!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 5: Session → Analytics ──────────────────────────────────

    #[test]
    fn test_session_to_analytics_logon() {
        let mut msg = FixMessage::new("FIX.4.4", "A");
        msg.set(tag::SENDER_COMP_ID, "ALICE");
        msg.set(tag::TARGET_COMP_ID, "BROKER");
        msg.set(tag::MSG_SEQ_NUM, "1");

        let ts = 5_000_000u64;
        let metric = fix_session_to_analytics(&msg, ts);

        assert_ne!(metric.content_hash, 0);
        assert_ne!(metric.sender_hash, 0);
        assert_ne!(metric.target_hash, 0);
        assert_eq!(metric.msg_type_disc, 1); // Logon
        assert_eq!(metric.msg_seq_num, 1);
        assert_eq!(metric.timestamp_ns, ts);
    }

    #[test]
    fn test_session_to_analytics_msg_type_classification() {
        let types = [("A", 1), ("5", 2), ("0", 3), ("D", 4), ("8", 5), ("X", 0)];
        for (mtype, expected_disc) in &types {
            let msg = FixMessage::new("FIX.4.4", mtype);
            let metric = fix_session_to_analytics(&msg, 0);
            assert_eq!(
                metric.msg_type_disc, *expected_disc,
                "msg_type '{mtype}' should map to disc {expected_disc}"
            );
        }
    }

    #[test]
    fn test_session_to_analytics_deterministic() {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(tag::SENDER_COMP_ID, "A");
        msg.set(tag::TARGET_COMP_ID, "B");

        let m1 = fix_session_to_analytics(&msg, 999);
        let m2 = fix_session_to_analytics(&msg, 999);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    #[test]
    fn test_session_to_analytics_different_senders_differ() {
        let mut msg1 = FixMessage::new("FIX.4.4", "A");
        msg1.set(tag::SENDER_COMP_ID, "ALICE");
        let mut msg2 = FixMessage::new("FIX.4.4", "A");
        msg2.set(tag::SENDER_COMP_ID, "BOB");

        let m1 = fix_session_to_analytics(&msg1, 0);
        let m2 = fix_session_to_analytics(&msg2, 0);
        assert_ne!(m1.sender_hash, m2.sender_hash);
        assert_ne!(m1.content_hash, m2.content_hash);
    }
}
