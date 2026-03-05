//! Risk bridges — ALICE-Risk ↔ Analytics, DB, Cache, Edge
//!
//! 5 bridges connecting ALICE-Risk pre-trade risk management, margin calculation,
//! circuit breaker state, and position tracking to the ALICE ecosystem.
//!
//! These bridges focus on limit snapshots, position snapshots, margin computation,
//! circuit breaker state, and pre-trade checker health metrics, complementing
//! `bridge_risk_ext` which covers rejection events, limit caching, semantic telemetry,
//! and ledger gating.

use alice_ledger::Position;
use alice_risk::{CircuitBreaker, MarginCalculator, PreTradeChecker, RiskLimits};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: RiskLimits → Analytics (limit configuration snapshot) ─────

/// Risk limit configuration snapshot for ALICE-Analytics.
///
/// Captures the full risk limit configuration for a given account at a point
/// in time, enabling analytics dashboards to track limit changes over time.
pub struct RiskLimitsAnalyticsEvent {
    /// FNV-1a content hash over `account_id`, `max_position`, `max_order_size`, and `max_notional`.
    pub content_hash: u64,
    /// Account identifier this snapshot belongs to.
    pub account_id: u64,
    /// Maximum net position size (absolute) in lots.
    pub max_position: u64,
    /// Maximum single order quantity in lots.
    pub max_order_size: u64,
    /// Maximum notional value in ticks.
    pub max_notional: i64,
    /// Maximum number of open orders.
    pub max_open_orders: u32,
    /// Maximum daily loss (negative value) before kill switch.
    pub max_daily_loss: i64,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`RiskLimits`] configuration into an analytics snapshot event.
///
/// `account_id` identifies the account or instrument this limit applies to.
#[inline]
#[must_use]
pub fn risk_limits_to_analytics(
    limits: &RiskLimits,
    account_id: u64,
    timestamp_ns: u64,
) -> RiskLimitsAnalyticsEvent {
    // Hash input: account_id(8) || max_position(8) || max_order_size(8) || max_notional(8) = 32 bytes
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&account_id.to_le_bytes());
    buf[8..16].copy_from_slice(&limits.max_position.to_le_bytes());
    buf[16..24].copy_from_slice(&limits.max_order_size.to_le_bytes());
    buf[24..32].copy_from_slice(&limits.max_notional.to_le_bytes());

    RiskLimitsAnalyticsEvent {
        content_hash: fnv1a(&buf),
        account_id,
        max_position: limits.max_position,
        max_order_size: limits.max_order_size,
        max_notional: limits.max_notional,
        max_open_orders: limits.max_open_orders,
        max_daily_loss: limits.max_daily_loss,
        timestamp_ns,
    }
}

// ── Bridge 2: Position → DB (position snapshot record) ──────────────────

/// Position snapshot DB record for ALICE-DB.
///
/// Captures the full position state at a point in time for the audit trail
/// and mark-to-market reconciliation.
pub struct RiskPositionDbRecord {
    /// FNV-1a content hash over `symbol_hash`, `net_quantity`, `avg_entry_price`, and `trade_count`.
    pub content_hash: u64,
    /// FNV-derived hash of the instrument symbol.
    pub symbol_hash: u64,
    /// Signed net position size (positive=long, negative=short, zero=flat).
    pub net_quantity: i64,
    /// Weighted-average entry price in ticks.
    pub avg_entry_price: i64,
    /// P&L from closed positions in ticks.
    pub realized_pnl: i64,
    /// P&L from open positions at last mark price.
    pub unrealized_pnl: i64,
    /// Total fills applied to this position.
    pub trade_count: u64,
    /// Snapshot timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`Position`] into a DB snapshot record.
///
/// `timestamp_ns` is the snapshot time supplied by the caller.
#[inline]
#[must_use]
pub fn risk_position_to_db(pos: &Position, timestamp_ns: u64) -> RiskPositionDbRecord {
    // Hash input: symbol_hash(8) || net_quantity(8) || avg_entry_price(8) || trade_count(8) = 32 bytes
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&pos.symbol_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&pos.net_quantity.to_le_bytes());
    buf[16..24].copy_from_slice(&pos.avg_entry_price.to_le_bytes());
    buf[24..32].copy_from_slice(&pos.trade_count.to_le_bytes());

    RiskPositionDbRecord {
        content_hash: fnv1a(&buf),
        symbol_hash: pos.symbol_hash,
        net_quantity: pos.net_quantity,
        avg_entry_price: pos.avg_entry_price,
        realized_pnl: pos.realized_pnl,
        unrealized_pnl: pos.unrealized_pnl,
        trade_count: pos.trade_count,
        timestamp_ns,
    }
}

// ── Bridge 3: Margin → Cache (margin requirement cache) ─────────────────

/// Margin requirement cache entry for ALICE-Cache.
///
/// Caches initial and maintenance margin requirements for a given price and
/// quantity so the order-entry path can check margin without recomputation.
/// TTL is branchless: margin calls (equity below maintenance) get short TTL
/// for rapid refresh; normal states get longer TTL.
pub struct RiskMarginCacheEntry {
    /// FNV-1a content hash over price, quantity, `initial_margin`, and `maintenance_margin`.
    pub content_hash: u64,
    /// Mark price used for margin computation.
    pub price: i64,
    /// Position quantity.
    pub quantity: u64,
    /// Initial margin requirement in ticks.
    pub initial_margin: i64,
    /// Maintenance margin requirement in ticks.
    pub maintenance_margin: i64,
    /// True when account equity is below maintenance margin (margin call).
    pub is_margin_call: bool,
    /// Cache TTL in seconds. Branchless: `margin_call=5s`, normal=60s.
    pub ttl_secs: u32,
}

/// Compute margin requirements and cache them.
///
/// `account_equity` is used to determine whether a margin call is active.
///
/// Branchless TTL: `60 - is_margin_call_u32 * 55`.
/// - `margin_call=true`  → 60 - 55 = 5s  (rapid refresh)
/// - `margin_call=false` → 60 - 0  = 60s (normal)
#[inline]
#[must_use]
pub fn risk_margin_to_cache(
    calc: &MarginCalculator,
    price: i64,
    quantity: u64,
    account_equity: i64,
) -> RiskMarginCacheEntry {
    let initial_margin = calc.initial_margin(price, quantity);
    let maintenance_margin = calc.maintenance_margin(price, quantity);
    let is_margin_call = calc.is_margin_call(price, quantity, account_equity);

    // Branchless TTL
    let mc_u32 = is_margin_call as u32;
    let ttl_secs = 60 - mc_u32 * 55;

    // Hash input: price(8) || quantity(8) || initial_margin(8) || maintenance_margin(8) = 32 bytes
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&price.to_le_bytes());
    buf[8..16].copy_from_slice(&quantity.to_le_bytes());
    buf[16..24].copy_from_slice(&initial_margin.to_le_bytes());
    buf[24..32].copy_from_slice(&maintenance_margin.to_le_bytes());

    RiskMarginCacheEntry {
        content_hash: fnv1a(&buf),
        price,
        quantity,
        initial_margin,
        maintenance_margin,
        is_margin_call,
        ttl_secs,
    }
}

// ── Bridge 4: CircuitBreaker → Edge (breaker state telemetry) ───────────

/// Circuit breaker state telemetry for ALICE-Edge.
///
/// Captures the circuit breaker configuration and trip state so the edge
/// layer can alert on breaker trips and monitor fill-rate patterns.
pub struct RiskCircuitBreakerEdgeEvent {
    /// FNV-1a content hash over `max_move`, `max_fills`, `window_ns`, and tripped flag.
    pub content_hash: u64,
    /// Maximum allowed price deviation before trip (ticks).
    pub max_move: i64,
    /// Maximum fills per rolling window.
    pub max_fills_per_window: u32,
    /// Rolling window duration in nanoseconds.
    pub window_ns: u64,
    /// True if the circuit breaker is currently tripped.
    pub is_tripped: bool,
    /// Severity: 3 if tripped, 1 if not tripped.
    pub severity: u8,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`CircuitBreaker`] snapshot into an edge telemetry event.
///
/// Severity is branchless: `1 + tripped_u8 * 2` → tripped=3, normal=1.
#[inline]
#[must_use]
pub fn risk_circuit_breaker_to_edge(
    cb: &CircuitBreaker,
    timestamp_ns: u64,
) -> RiskCircuitBreakerEdgeEvent {
    let is_tripped = cb.is_tripped();
    let tripped_u8 = is_tripped as u8;
    let severity = 1 + tripped_u8 * 2;

    // Hash input: max_move(8) || max_fills(4) || window_ns(8) || tripped(1) = 21 bytes
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&cb.max_move.to_le_bytes());
    buf[8..12].copy_from_slice(&cb.max_fills_per_window.to_le_bytes());
    buf[12..20].copy_from_slice(&cb.window_ns.to_le_bytes());
    buf[20] = tripped_u8;

    RiskCircuitBreakerEdgeEvent {
        content_hash: fnv1a(&buf),
        max_move: cb.max_move,
        max_fills_per_window: cb.max_fills_per_window,
        window_ns: cb.window_ns,
        is_tripped,
        severity,
        timestamp_ns,
    }
}

// ── Bridge 5: PreTradeChecker → Analytics (checker health metric) ───────

/// Pre-trade checker health metric for ALICE-Analytics.
///
/// Captures the internal state of the pre-trade risk engine so that
/// analytics dashboards can track daily P&L trends, open order counts,
/// and circuit breaker status in real time.
pub struct RiskCheckerAnalyticsMetric {
    /// FNV-1a content hash over `daily_pnl`, `open_order_count`, and `cb_tripped`.
    pub content_hash: u64,
    /// Current accumulated daily P&L (may be negative).
    pub daily_pnl: i64,
    /// Current number of open orders.
    pub open_order_count: u32,
    /// True if the internal circuit breaker is tripped.
    pub circuit_breaker_tripped: bool,
    /// Utilization ratio: `open_order_count` / `max_open_orders` (0.0 if max is 0).
    pub order_utilization: f64,
    /// Event timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`PreTradeChecker`] snapshot and its [`RiskLimits`] into a health metric.
///
/// `order_utilization` is `open_order_count / max_open_orders`, or 0.0 if
/// `max_open_orders` is zero.
#[inline]
#[must_use]
pub fn risk_checker_to_analytics(
    checker: &PreTradeChecker,
    limits: &RiskLimits,
    timestamp_ns: u64,
) -> RiskCheckerAnalyticsMetric {
    let daily_pnl = checker.daily_pnl();
    let open_order_count = checker.open_order_count();
    let cb_tripped = checker.is_circuit_breaker_tripped();

    let order_utilization = if limits.max_open_orders == 0 {
        0.0
    } else {
        open_order_count as f64 / limits.max_open_orders as f64
    };

    // Hash input: daily_pnl(8) || open_order_count(4) || cb_tripped(1) = 13 bytes
    let mut buf = [0u8; 13];
    buf[0..8].copy_from_slice(&daily_pnl.to_le_bytes());
    buf[8..12].copy_from_slice(&open_order_count.to_le_bytes());
    buf[12] = cb_tripped as u8;

    RiskCheckerAnalyticsMetric {
        content_hash: fnv1a(&buf),
        daily_pnl,
        open_order_count,
        circuit_breaker_tripped: cb_tripped,
        order_utilization,
        timestamp_ns,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use alice_ledger::Position;
    use alice_risk::{CircuitBreaker, MarginCalculator, MarginParams, PreTradeChecker, RiskLimits};

    fn make_position(symbol: u64, net_qty: i64, avg_price: i64, trades: u64) -> Position {
        Position {
            symbol_hash: symbol,
            net_quantity: net_qty,
            avg_entry_price: avg_price,
            realized_pnl: 1000,
            unrealized_pnl: -500,
            trade_count: trades,
        }
    }

    // ── Bridge 1: RiskLimits → Analytics ───────────────────────────────

    #[test]
    fn test_limits_to_analytics_basic() {
        let limits = RiskLimits {
            max_position: 10_000,
            max_order_size: 1_000,
            max_notional: 5_000_000,
            max_open_orders: 20,
            max_daily_loss: -100_000,
        };
        let ts = 1_700_000_000_000_000_000u64;
        let ev = risk_limits_to_analytics(&limits, 0xABCD, ts);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.account_id, 0xABCD);
        assert_eq!(ev.max_position, 10_000);
        assert_eq!(ev.max_order_size, 1_000);
        assert_eq!(ev.max_notional, 5_000_000);
        assert_eq!(ev.max_open_orders, 20);
        assert_eq!(ev.max_daily_loss, -100_000);
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_limits_to_analytics_deterministic() {
        let limits = RiskLimits::default();
        let ev1 = risk_limits_to_analytics(&limits, 1, 100);
        let ev2 = risk_limits_to_analytics(&limits, 1, 100);
        assert_eq!(ev1.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_limits_to_analytics_different_accounts_differ() {
        let limits = RiskLimits::default();
        let ev1 = risk_limits_to_analytics(&limits, 1, 0);
        let ev2 = risk_limits_to_analytics(&limits, 2, 0);
        assert_ne!(ev1.content_hash, ev2.content_hash);
    }

    // ── Bridge 2: Position → DB ────────────────────────────────────────

    #[test]
    fn test_position_to_db_basic() {
        let pos = make_position(0xDEAD_BEEF, 500, 50_000, 42);
        let ts = 9_999u64;
        let rec = risk_position_to_db(&pos, ts);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.symbol_hash, 0xDEAD_BEEF);
        assert_eq!(rec.net_quantity, 500);
        assert_eq!(rec.avg_entry_price, 50_000);
        assert_eq!(rec.realized_pnl, 1000);
        assert_eq!(rec.unrealized_pnl, -500);
        assert_eq!(rec.trade_count, 42);
        assert_eq!(rec.timestamp_ns, ts);
    }

    #[test]
    fn test_position_to_db_deterministic() {
        let pos = make_position(0x1234, 100, 1000, 5);
        let r1 = risk_position_to_db(&pos, 0);
        let r2 = risk_position_to_db(&pos, 0);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_position_to_db_different_positions_differ() {
        let p1 = make_position(0x1234, 100, 1000, 5);
        let p2 = make_position(0x1234, 200, 1000, 5);
        let r1 = risk_position_to_db(&p1, 0);
        let r2 = risk_position_to_db(&p2, 0);
        assert_ne!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 3: Margin → Cache ───────────────────────────────────────

    #[test]
    fn test_margin_to_cache_normal() {
        let calc = MarginCalculator::new(MarginParams::default());
        // price=10_000, qty=10, equity=50_000 → initial=10_000, maint=5_000
        // equity=50_000 > maint=5_000 → no margin call
        let entry = risk_margin_to_cache(&calc, 10_000, 10, 50_000);

        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.price, 10_000);
        assert_eq!(entry.quantity, 10);
        assert_eq!(entry.initial_margin, 10_000);
        assert_eq!(entry.maintenance_margin, 5_000);
        assert!(!entry.is_margin_call);
        assert_eq!(entry.ttl_secs, 60); // Normal = 60s
    }

    #[test]
    fn test_margin_to_cache_margin_call() {
        let calc = MarginCalculator::new(MarginParams::default());
        // price=10_000, qty=10, equity=4_999 < maint=5_000 → margin call
        let entry = risk_margin_to_cache(&calc, 10_000, 10, 4_999);

        assert!(entry.is_margin_call);
        assert_eq!(entry.ttl_secs, 5); // Margin call = 5s
    }

    #[test]
    fn test_margin_to_cache_deterministic() {
        let calc = MarginCalculator::new(MarginParams::default());
        let e1 = risk_margin_to_cache(&calc, 5_000, 20, 100_000);
        let e2 = risk_margin_to_cache(&calc, 5_000, 20, 100_000);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 4: CircuitBreaker → Edge ────────────────────────────────

    #[test]
    fn test_circuit_breaker_to_edge_not_tripped() {
        let cb = CircuitBreaker::new(500, 5, 1_000_000_000);
        let ts = 3_000_000u64;
        let ev = risk_circuit_breaker_to_edge(&cb, ts);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.max_move, 500);
        assert_eq!(ev.max_fills_per_window, 5);
        assert_eq!(ev.window_ns, 1_000_000_000);
        assert!(!ev.is_tripped);
        assert_eq!(ev.severity, 1); // Not tripped = low
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_circuit_breaker_to_edge_tripped() {
        let mut cb = CircuitBreaker::new(500, 5, 1_000_000_000);
        cb.reset(10_000, 0);
        let _ = cb.on_fill(10_600, 100_000_000); // Price move > 500 trips
        assert!(cb.is_tripped());

        let ev = risk_circuit_breaker_to_edge(&cb, 200_000_000);

        assert!(ev.is_tripped);
        assert_eq!(ev.severity, 3); // Tripped = high
    }

    #[test]
    fn test_circuit_breaker_to_edge_deterministic() {
        let cb = CircuitBreaker::new(100, 10, 2_000_000_000);
        let e1 = risk_circuit_breaker_to_edge(&cb, 0);
        let e2 = risk_circuit_breaker_to_edge(&cb, 0);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_circuit_breaker_to_edge_tripped_vs_not_differ() {
        let cb1 = CircuitBreaker::new(500, 5, 1_000_000_000);
        let mut cb2 = CircuitBreaker::new(500, 5, 1_000_000_000);
        cb2.reset(10_000, 0);
        let _ = cb2.on_fill(10_600, 100_000_000); // Trip cb2

        let e1 = risk_circuit_breaker_to_edge(&cb1, 0);
        let e2 = risk_circuit_breaker_to_edge(&cb2, 0);
        assert_ne!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 5: PreTradeChecker → Analytics ──────────────────────────

    #[test]
    fn test_checker_to_analytics_initial_state() {
        let limits = RiskLimits::default();
        let checker = PreTradeChecker::new(limits.clone());
        let ts = 7_777u64;
        let metric = risk_checker_to_analytics(&checker, &limits, ts);

        assert_ne!(metric.content_hash, 0);
        assert_eq!(metric.daily_pnl, 0);
        assert_eq!(metric.open_order_count, 0);
        assert!(!metric.circuit_breaker_tripped);
        assert_eq!(metric.order_utilization, 0.0);
        assert_eq!(metric.timestamp_ns, ts);
    }

    #[test]
    fn test_checker_to_analytics_with_state() {
        let limits = RiskLimits {
            max_open_orders: 100,
            ..RiskLimits::default()
        };
        let mut checker = PreTradeChecker::new(limits.clone());
        checker.update_daily_pnl(-5000);
        checker.increment_open_orders();
        checker.increment_open_orders();
        checker.increment_open_orders();

        let metric = risk_checker_to_analytics(&checker, &limits, 0);

        assert_eq!(metric.daily_pnl, -5000);
        assert_eq!(metric.open_order_count, 3);
        assert!(!metric.circuit_breaker_tripped);
        assert!((metric.order_utilization - 0.03).abs() < 1e-12);
    }

    #[test]
    fn test_checker_to_analytics_circuit_breaker_tripped() {
        let limits = RiskLimits::default();
        let mut checker = PreTradeChecker::new(limits.clone());
        checker.trip_circuit_breaker();

        let metric = risk_checker_to_analytics(&checker, &limits, 0);
        assert!(metric.circuit_breaker_tripped);
    }

    #[test]
    fn test_checker_to_analytics_deterministic() {
        let limits = RiskLimits::default();
        let checker = PreTradeChecker::new(limits.clone());
        let m1 = risk_checker_to_analytics(&checker, &limits, 500);
        let m2 = risk_checker_to_analytics(&checker, &limits, 500);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    #[test]
    fn test_checker_to_analytics_zero_max_orders() {
        let limits = RiskLimits {
            max_open_orders: 0,
            ..RiskLimits::default()
        };
        let checker = PreTradeChecker::new(limits.clone());
        let metric = risk_checker_to_analytics(&checker, &limits, 0);
        assert_eq!(metric.order_utilization, 0.0);
    }
}
