//! Ledger bridges — ALICE-Ledger ↔ Analytics, DB, Cache
//!
//! 5 bridges connecting order book and position data to the ALICE ecosystem.

use alice_ledger::{Order, OrderType, Side, Fill, Position};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Ledger Order → Analytics (order submission metrics) ─────────

/// Order submission event for ALICE-Analytics ingestion.
pub struct LedgerAnalyticsOrderEvent {
    /// Content hash over order ID, side, and price bytes.
    pub content_hash: u64,
    /// Side of the order: 0 = Bid, 1 = Ask.
    pub side: u8,
    /// Order type: 0 = Limit, 1 = Market, 2 = StopLimit.
    pub order_type: u8,
    /// Limit price in ticks.
    pub price_ticks: i64,
    /// Total quantity requested in base-asset lots.
    pub quantity: u64,
    /// Submission timestamp as nanoseconds since the Unix epoch.
    pub timestamp_ns: u64,
}

/// Convert a ledger order into an analytics order submission event.
#[inline]
pub fn ledger_order_to_analytics(order: &Order) -> LedgerAnalyticsOrderEvent {
    let side_byte: u8 = match order.side {
        Side::Bid => 0,
        Side::Ask => 1,
    };
    let order_type_byte: u8 = match order.order_type {
        OrderType::Limit         => 0,
        OrderType::Market        => 1,
        OrderType::StopLimit { .. } => 2,
    };

    // Hash: order_id bytes + side byte + price bytes.
    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&order.id.0.to_le_bytes());
    key[8] = side_byte;
    key[9..17].copy_from_slice(&order.price.to_le_bytes());

    LedgerAnalyticsOrderEvent {
        content_hash: fnv1a(&key),
        side: side_byte,
        order_type: order_type_byte,
        price_ticks: order.price,
        quantity: order.quantity,
        timestamp_ns: order.timestamp_ns,
    }
}

// ── Bridge 2: Ledger Fill → Analytics (execution metrics) ─────────────────

/// Fill execution event for ALICE-Analytics ingestion.
pub struct LedgerAnalyticsFillEvent {
    /// Content hash over maker ID, taker ID, price, and quantity bytes.
    pub content_hash: u64,
    /// Inner u64 of the passive maker order ID.
    pub maker_id: u64,
    /// Inner u64 of the aggressive taker order ID.
    pub taker_id: u64,
    /// Execution price in ticks.
    pub price_ticks: i64,
    /// Quantity matched in this fill event.
    pub quantity: u64,
    /// Fill timestamp as nanoseconds since the Unix epoch.
    pub timestamp_ns: u64,
}

/// Convert a ledger fill into an analytics execution event.
#[inline]
pub fn ledger_fill_to_analytics(fill: &Fill) -> LedgerAnalyticsFillEvent {
    // Hash: maker_id + taker_id + price + quantity bytes.
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&fill.maker_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&fill.taker_id.0.to_le_bytes());
    key[16..24].copy_from_slice(&fill.price.to_le_bytes());
    key[24..32].copy_from_slice(&fill.quantity.to_le_bytes());

    LedgerAnalyticsFillEvent {
        content_hash: fnv1a(&key),
        maker_id: fill.maker_id.0,
        taker_id: fill.taker_id.0,
        price_ticks: fill.price,
        quantity: fill.quantity,
        timestamp_ns: fill.timestamp_ns,
    }
}

// ── Bridge 3: Ledger Fill → DB (trade audit record) ───────────────────────

/// Trade audit record for ALICE-DB persistence.
pub struct LedgerDbFillRecord {
    /// Content hash over maker ID, taker ID, price, and quantity bytes.
    pub content_hash: u64,
    /// Inner u64 of the passive maker order ID.
    pub maker_id: u64,
    /// Inner u64 of the aggressive taker order ID.
    pub taker_id: u64,
    /// Execution price in ticks.
    pub price_ticks: i64,
    /// Quantity matched in this fill event.
    pub quantity: u64,
    /// Fill timestamp as nanoseconds since the Unix epoch.
    pub timestamp_ns: u64,
    /// FNV-derived hash of the instrument symbol.
    pub symbol_hash: u64,
}

/// Convert a ledger fill and symbol hash into a DB trade audit record.
#[inline]
pub fn ledger_fill_to_db_record(fill: &Fill, symbol_hash: u64) -> LedgerDbFillRecord {
    // Hash: maker_id + taker_id + price + quantity bytes (same as analytics).
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&fill.maker_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&fill.taker_id.0.to_le_bytes());
    key[16..24].copy_from_slice(&fill.price.to_le_bytes());
    key[24..32].copy_from_slice(&fill.quantity.to_le_bytes());

    LedgerDbFillRecord {
        content_hash: fnv1a(&key),
        maker_id: fill.maker_id.0,
        taker_id: fill.taker_id.0,
        price_ticks: fill.price,
        quantity: fill.quantity,
        timestamp_ns: fill.timestamp_ns,
        symbol_hash,
    }
}

// ── Bridge 4: Ledger Position → Analytics (P&L metrics) ───────────────────

/// P&L metrics event for ALICE-Analytics ingestion.
pub struct LedgerAnalyticsPnlEvent {
    /// Content hash over symbol hash, net quantity, and realized P&L bytes.
    pub content_hash: u64,
    /// FNV-derived hash of the instrument symbol.
    pub symbol_hash: u64,
    /// Signed position size: positive = long, negative = short, zero = flat.
    pub net_quantity: i64,
    /// Weighted-average entry price in ticks.
    pub avg_entry_price: i64,
    /// Realized P&L in ticks.
    pub realized_pnl: i64,
    /// Unrealized P&L at the last mark-to-market price, in ticks.
    pub unrealized_pnl: i64,
    /// Total number of fills applied to this position.
    pub trade_count: u64,
}

/// Convert a ledger position into an analytics P&L event.
#[inline]
pub fn ledger_position_to_analytics(pos: &Position) -> LedgerAnalyticsPnlEvent {
    // Hash: symbol_hash + net_quantity + realized_pnl bytes.
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&pos.symbol_hash.to_le_bytes());
    key[8..16].copy_from_slice(&pos.net_quantity.to_le_bytes());
    key[16..24].copy_from_slice(&pos.realized_pnl.to_le_bytes());

    LedgerAnalyticsPnlEvent {
        content_hash: fnv1a(&key),
        symbol_hash: pos.symbol_hash,
        net_quantity: pos.net_quantity,
        avg_entry_price: pos.avg_entry_price,
        realized_pnl: pos.realized_pnl,
        unrealized_pnl: pos.unrealized_pnl,
        trade_count: pos.trade_count,
    }
}

// ── Bridge 5: Ledger Position → Cache (real-time P&L lookup) ──────────────

/// Real-time position cache entry for ALICE-Cache.
pub struct LedgerCachePosition {
    /// Content hash over symbol hash and net quantity bytes.
    pub content_hash: u64,
    /// FNV-derived hash of the instrument symbol (used as cache key).
    pub symbol_hash: u64,
    /// Signed position size: positive = long, negative = short, zero = flat.
    pub net_quantity: i64,
    /// Unrealized P&L at the last mark-to-market price, in ticks.
    pub unrealized_pnl: i64,
    /// Cache TTL in seconds: 5 when |unrealized_pnl| > 100_000, else 30.
    pub ttl_secs: u32,
}

/// Convert a ledger position into a real-time cache entry.
///
/// TTL is computed branchlessly: volatile positions (|unrealized_pnl| > 100_000)
/// receive a 5-second TTL; stable positions receive a 30-second TTL.
#[inline]
pub fn ledger_position_to_cache(pos: &Position) -> LedgerCachePosition {
    // Branchless TTL: volatile=1 → 30-25=5, stable=0 → 30-0=30.
    let volatile = (pos.unrealized_pnl.unsigned_abs() > 100_000) as u32;
    let ttl_secs = 30 - volatile * 25;

    // Hash: symbol_hash + net_quantity bytes.
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&pos.symbol_hash.to_le_bytes());
    key[8..16].copy_from_slice(&pos.net_quantity.to_le_bytes());

    LedgerCachePosition {
        content_hash: fnv1a(&key),
        symbol_hash: pos.symbol_hash,
        net_quantity: pos.net_quantity,
        unrealized_pnl: pos.unrealized_pnl,
        ttl_secs,
    }
}

// ── Bridge 6: Ledger Fill → Settlement trade record ───────────────────────

/// Settlement trade record derived from a ledger fill event.
///
/// Carries the minimum fields required by the settlement engine to register
/// an executed trade for T+2 standard delivery settlement.
pub struct LedgerSettlementTrade {
    /// Content hash over fill_price, fill_qty, maker_id, and taker_id bytes.
    pub content_hash: u64,
    /// Execution price in ticks (from the fill's maker limit price).
    pub fill_price: f64,
    /// Quantity matched in this fill event.
    pub fill_qty: u64,
    /// Inner u64 of the passive maker order ID.
    pub maker_id: u64,
    /// Inner u64 of the aggressive taker order ID.
    pub taker_id: u64,
    /// Number of business days until settlement: always 2 (T+2 standard settlement).
    pub settlement_date_offset: u8,
}

/// Convert a ledger [`Fill`] into a [`LedgerSettlementTrade`] record.
///
/// `fill_price` is stored as `f64` ticks for compatibility with the settlement
/// engine's floating-point price representation.  `settlement_date_offset` is
/// always 2 (T+2), the standard equity and FX settlement convention.
///
/// `content_hash` is FNV-1a over `fill_price` bits (8 bytes) concatenated with
/// `fill_qty` (8 bytes), `maker_id` (8 bytes), and `taker_id` (8 bytes),
/// all in little-endian byte order.
#[inline]
pub fn ledger_fill_to_settlement_trade(fill: &Fill) -> LedgerSettlementTrade {
    let fill_price = fill.price as f64;
    let fill_qty   = fill.quantity;
    let maker_id   = fill.maker_id.0;
    let taker_id   = fill.taker_id.0;

    // Hash input: fill_price bits (8) || fill_qty (8) || maker_id (8) || taker_id (8)
    let mut hash_data = [0u8; 32];
    hash_data[0..8].copy_from_slice(&fill_price.to_bits().to_le_bytes());
    hash_data[8..16].copy_from_slice(&fill_qty.to_le_bytes());
    hash_data[16..24].copy_from_slice(&maker_id.to_le_bytes());
    hash_data[24..32].copy_from_slice(&taker_id.to_le_bytes());

    LedgerSettlementTrade {
        content_hash: fnv1a(&hash_data),
        fill_price,
        fill_qty,
        maker_id,
        taker_id,
        settlement_date_offset: 2,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_ledger::{OrderId, TimeInForce, Fill, Position};

    // ── helpers ──────────────────────────────────────────────────────────

    fn make_limit_order(id: u64, side: Side, price: i64, qty: u64) -> Order {
        Order {
            id: OrderId(id),
            side,
            order_type: OrderType::Limit,
            price,
            quantity: qty,
            filled_quantity: 0,
            timestamp_ns: 1_000_000_000,
            time_in_force: TimeInForce::GTC,
        }
    }

    fn make_market_order(id: u64, side: Side, qty: u64) -> Order {
        Order {
            id: OrderId(id),
            side,
            order_type: OrderType::Market,
            price: 0,
            quantity: qty,
            filled_quantity: 0,
            timestamp_ns: 2_000_000_000,
            time_in_force: TimeInForce::IOC,
        }
    }

    fn make_fill(maker: u64, taker: u64, price: i64, qty: u64, ts: u64) -> Fill {
        Fill {
            maker_id: OrderId(maker),
            taker_id: OrderId(taker),
            price,
            quantity: qty,
            timestamp_ns: ts,
        }
    }

    fn make_position(
        symbol_hash: u64,
        net_quantity: i64,
        avg_entry_price: i64,
        realized_pnl: i64,
        unrealized_pnl: i64,
        trade_count: u64,
    ) -> Position {
        Position {
            symbol_hash,
            net_quantity,
            avg_entry_price,
            realized_pnl,
            unrealized_pnl,
            trade_count,
        }
    }

    // ── Bridge 1 tests ───────────────────────────────────────────────────

    #[test]
    fn test_order_to_analytics_bid() {
        let order = make_limit_order(42, Side::Bid, 1000, 50);
        let ev = ledger_order_to_analytics(&order);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.side, 0);          // Bid → 0
        assert_eq!(ev.order_type, 0);    // Limit → 0
        assert_eq!(ev.price_ticks, 1000);
        assert_eq!(ev.quantity, 50);
        assert_eq!(ev.timestamp_ns, 1_000_000_000);
    }

    #[test]
    fn test_order_to_analytics_ask() {
        let order = make_limit_order(99, Side::Ask, 1005, 25);
        let ev = ledger_order_to_analytics(&order);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.side, 1);          // Ask → 1
        assert_eq!(ev.order_type, 0);    // Limit → 0
        assert_eq!(ev.price_ticks, 1005);
        assert_eq!(ev.quantity, 25);
    }

    #[test]
    fn test_order_to_analytics_market() {
        let order = make_market_order(7, Side::Bid, 100);
        let ev = ledger_order_to_analytics(&order);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.side, 0);          // Bid → 0
        assert_eq!(ev.order_type, 1);    // Market → 1
        assert_eq!(ev.quantity, 100);
    }

    #[test]
    fn test_order_to_analytics_stop_limit() {
        let order = Order {
            id: OrderId(5),
            side: Side::Ask,
            order_type: OrderType::StopLimit { stop_price: 990 },
            price: 985,
            quantity: 10,
            filled_quantity: 0,
            timestamp_ns: 500,
            time_in_force: TimeInForce::GTC,
        };
        let ev = ledger_order_to_analytics(&order);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.side, 1);          // Ask → 1
        assert_eq!(ev.order_type, 2);    // StopLimit → 2
        assert_eq!(ev.price_ticks, 985);
    }

    // ── Bridge 2 tests ───────────────────────────────────────────────────

    #[test]
    fn test_fill_to_analytics() {
        let fill = make_fill(10, 20, 1000, 5, 999_000);
        let ev = ledger_fill_to_analytics(&fill);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.maker_id, 10);
        assert_eq!(ev.taker_id, 20);
        assert_eq!(ev.price_ticks, 1000);
        assert_eq!(ev.quantity, 5);
        assert_eq!(ev.timestamp_ns, 999_000);
    }

    // ── Bridge 3 tests ───────────────────────────────────────────────────

    #[test]
    fn test_fill_to_db_record() {
        let fill = make_fill(3, 7, 2000, 10, 123_456);
        let symbol_hash: u64 = 0xDEAD_BEEF_CAFE_1234;
        let rec = ledger_fill_to_db_record(&fill, symbol_hash);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.maker_id, 3);
        assert_eq!(rec.taker_id, 7);
        assert_eq!(rec.price_ticks, 2000);
        assert_eq!(rec.quantity, 10);
        assert_eq!(rec.timestamp_ns, 123_456);
        assert_eq!(rec.symbol_hash, symbol_hash);
    }

    // ── Bridge 4 tests ───────────────────────────────────────────────────

    #[test]
    fn test_position_to_analytics_long() {
        let pos = make_position(0xABCD_EF01, 100, 1000, 500, 200, 10);
        let ev = ledger_position_to_analytics(&pos);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.symbol_hash, 0xABCD_EF01);
        assert_eq!(ev.net_quantity, 100);
        assert_eq!(ev.avg_entry_price, 1000);
        assert_eq!(ev.realized_pnl, 500);
        assert_eq!(ev.unrealized_pnl, 200);
        assert_eq!(ev.trade_count, 10);
    }

    #[test]
    fn test_position_to_analytics_short() {
        let pos = make_position(0x1234_5678, -50, 2000, -100, -300, 5);
        let ev = ledger_position_to_analytics(&pos);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.net_quantity, -50);
        assert_eq!(ev.avg_entry_price, 2000);
        assert_eq!(ev.realized_pnl, -100);
        assert_eq!(ev.unrealized_pnl, -300);
        assert_eq!(ev.trade_count, 5);
    }

    // ── Bridge 5 tests ───────────────────────────────────────────────────

    #[test]
    fn test_position_to_cache_volatile() {
        // |unrealized_pnl| = 200_000 > 100_000 → ttl = 5
        let pos = make_position(0xFEED_FACE, 200, 1500, 0, 200_000, 3);
        let entry = ledger_position_to_cache(&pos);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.symbol_hash, 0xFEED_FACE);
        assert_eq!(entry.net_quantity, 200);
        assert_eq!(entry.unrealized_pnl, 200_000);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_position_to_cache_stable() {
        // |unrealized_pnl| = 50_000 <= 100_000 → ttl = 30
        let pos = make_position(0xCAFE_BABE, 10, 800, 0, 50_000, 1);
        let entry = ledger_position_to_cache(&pos);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.symbol_hash, 0xCAFE_BABE);
        assert_eq!(entry.net_quantity, 10);
        assert_eq!(entry.unrealized_pnl, 50_000);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_position_to_cache_negative_volatile() {
        // |unrealized_pnl| = 150_001 > 100_000 (negative value) → ttl = 5
        let pos = make_position(0x1111_2222, -30, 1200, -5000, -150_001, 8);
        let entry = ledger_position_to_cache(&pos);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 5);
    }

    #[test]
    fn test_position_to_cache_boundary() {
        // |unrealized_pnl| = 100_000 exactly → not strictly greater → ttl = 30
        let pos = make_position(0x9999_AAAA, 5, 500, 0, 100_000, 2);
        let entry = ledger_position_to_cache(&pos);

        assert_eq!(entry.ttl_secs, 30);
    }

    // ── Bridge 6 tests ───────────────────────────────────────────────────

    #[test]
    fn test_fill_to_settlement_trade_basic() {
        let fill = make_fill(10, 20, 50_000, 5, 1_000_000);
        let st = ledger_fill_to_settlement_trade(&fill);

        assert_ne!(st.content_hash, 0);
        assert_eq!(st.fill_price, 50_000.0);
        assert_eq!(st.fill_qty, 5);
        assert_eq!(st.maker_id, 10);
        assert_eq!(st.taker_id, 20);
        // T+2 standard settlement.
        assert_eq!(st.settlement_date_offset, 2);
    }

    #[test]
    fn test_fill_to_settlement_trade_t2_invariant() {
        // settlement_date_offset must always be 2 regardless of fill values.
        let fill1 = make_fill(1, 2, 100, 1, 0);
        let fill2 = make_fill(99, 88, 999_999, 10_000, 5_000_000);
        let st1 = ledger_fill_to_settlement_trade(&fill1);
        let st2 = ledger_fill_to_settlement_trade(&fill2);
        assert_eq!(st1.settlement_date_offset, 2);
        assert_eq!(st2.settlement_date_offset, 2);
    }

    #[test]
    fn test_fill_to_settlement_trade_deterministic() {
        let fill = make_fill(3, 7, 12_500, 50, 42_000);
        let st1 = ledger_fill_to_settlement_trade(&fill);
        let st2 = ledger_fill_to_settlement_trade(&fill);
        assert_eq!(st1.content_hash, st2.content_hash);
        assert_eq!(st1.fill_price,   st2.fill_price);
        assert_eq!(st1.maker_id,     st2.maker_id);
    }

    #[test]
    fn test_fill_to_settlement_trade_hash_changes_with_price() {
        let fill1 = make_fill(1, 2, 10_000, 10, 0);
        let fill2 = make_fill(1, 2, 10_001, 10, 0);
        let st1 = ledger_fill_to_settlement_trade(&fill1);
        let st2 = ledger_fill_to_settlement_trade(&fill2);
        assert_ne!(st1.content_hash, st2.content_hash);
    }

    #[test]
    fn test_fill_to_settlement_trade_hash_changes_with_qty() {
        let fill1 = make_fill(5, 6, 500, 100, 0);
        let fill2 = make_fill(5, 6, 500, 101, 0);
        let st1 = ledger_fill_to_settlement_trade(&fill1);
        let st2 = ledger_fill_to_settlement_trade(&fill2);
        assert_ne!(st1.content_hash, st2.content_hash);
    }

    #[test]
    fn test_fill_to_settlement_trade_hash_changes_with_maker_id() {
        let fill1 = make_fill(1, 2, 1000, 5, 0);
        let fill2 = make_fill(9, 2, 1000, 5, 0);
        let st1 = ledger_fill_to_settlement_trade(&fill1);
        let st2 = ledger_fill_to_settlement_trade(&fill2);
        assert_ne!(st1.content_hash, st2.content_hash);
    }
}
