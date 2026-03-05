//! FIX extension bridges — ALICE-FIX ↔ Analytics, Ledger, Semantic Telemetry
//!
//! 3 bridges connecting ALICE-FIX to the ALICE ecosystem:
//!
//! - Bridge 1: FIX Message → Analytics (protocol metrics)
//! - Bridge 2: Ledger Fill → FIX `ExecutionReport` (outbound notification)
//! - Bridge 3: FIX Session → Semantic Telemetry (session lifecycle events)

use alice_fix::{FixMessage, FixSession};
use alice_ledger::Fill;

// ---------------------------------------------------------------------------
// FNV-1a hash — deterministic, branch-free, no external dependency
// ---------------------------------------------------------------------------

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ---------------------------------------------------------------------------
// Bridge 1: FIX Message → Analytics (protocol metrics)
// ---------------------------------------------------------------------------

/// Protocol metrics derived from a parsed FIX message, ready for
/// ALICE-Analytics ingestion.
pub struct FixAnalyticsMessageEvent {
    /// Content hash over `msg_type` bytes and `field_count` bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the message type string (e.g. "D", "8", "A").
    pub msg_type_hash: u64,
    /// Number of tag/value fields present in the message.
    pub field_count: u32,
    /// Timestamp supplied by the caller in nanoseconds since the Unix epoch.
    pub timestamp_ns: u64,
}

/// Convert a parsed [`FixMessage`] into an analytics event.
///
/// `content_hash` is computed over the concatenation of the `msg_type` bytes
/// and the little-endian bytes of `field_count`, giving a compact fingerprint
/// that distinguishes message types and payload sizes.
#[inline]
#[must_use]
pub fn fix_message_to_analytics(msg: &FixMessage, timestamp_ns: u64) -> FixAnalyticsMessageEvent {
    let field_count = msg.fields.len() as u32;
    let msg_type_hash = fnv1a(msg.msg_type.as_bytes());

    // Hash input: msg_type bytes || field_count as 4-byte LE
    let field_count_bytes = field_count.to_le_bytes();
    let hash_data: Vec<u8> = msg
        .msg_type
        .as_bytes()
        .iter()
        .copied()
        .chain(field_count_bytes.iter().copied())
        .collect();

    FixAnalyticsMessageEvent {
        content_hash: fnv1a(&hash_data),
        msg_type_hash,
        field_count,
        timestamp_ns,
    }
}

// ---------------------------------------------------------------------------
// Bridge 2: Ledger Fill → FIX ExecutionReport (outbound notification)
// ---------------------------------------------------------------------------

/// FIX `ExecutionReport` (`MsgType` "8") data derived from a ledger fill event.
///
/// This struct carries the minimum fields required to construct an outbound
/// `ExecutionReport` notification without referencing the full FIX wire format.
pub struct LedgerFixExecReport {
    /// Content hash over `maker_id`, `taker_id`, price, and quantity bytes.
    pub content_hash: u64,
    /// Inner u64 of the maker [`OrderId`](alice_ledger::OrderId).
    pub maker_id: u64,
    /// Inner u64 of the taker [`OrderId`](alice_ledger::OrderId).
    pub taker_id: u64,
    /// Execution price in ticks (maker's limit price).
    pub fill_price: i64,
    /// Quantity matched in this fill event.
    pub fill_qty: u64,
    /// `ExecType` tag value: 1 = `PartialFill`, 2 = Fill.
    pub exec_type: u8,
}

/// Convert a ledger [`Fill`] into a FIX `ExecutionReport` event.
///
/// `exec_type` is computed branchlessly: `1 + fully_filled as u8` yields
/// `1` (`PartialFill`) when `fully_filled` is `false` and `2` (Fill) when
/// `fully_filled` is `true`.
#[inline]
#[must_use]
pub fn ledger_fill_to_fix_exec(fill: &Fill, fully_filled: bool) -> LedgerFixExecReport {
    let maker_id = fill.maker_id.0;
    let taker_id = fill.taker_id.0;

    // Branchless ExecType: false → 1 (PartialFill), true → 2 (Fill).
    let exec_type = 1 + fully_filled as u8;

    // Hash input: maker_id || taker_id || price || qty (all little-endian)
    let mut hash_data = [0u8; 32];
    hash_data[0..8].copy_from_slice(&maker_id.to_le_bytes());
    hash_data[8..16].copy_from_slice(&taker_id.to_le_bytes());
    hash_data[16..24].copy_from_slice(&fill.price.to_le_bytes());
    hash_data[24..32].copy_from_slice(&fill.quantity.to_le_bytes());

    LedgerFixExecReport {
        content_hash: fnv1a(&hash_data),
        maker_id,
        taker_id,
        fill_price: fill.price,
        fill_qty: fill.quantity,
        exec_type,
    }
}

// ---------------------------------------------------------------------------
// Bridge 3: FIX Session → Semantic Telemetry (session lifecycle events)
// ---------------------------------------------------------------------------

/// Semantic telemetry event derived from a FIX session snapshot.
///
/// Because [`FixSession`] does not expose its sender/target component IDs or
/// sequence numbers through public getters, `sender_hash` and `target_hash`
/// are set to `0` and `outgoing_seq`/`incoming_seq` are set to `0`. The
/// `content_hash` is computed over `timestamp_ns` and the session state
/// discriminant, providing a unique fingerprint per lifecycle transition.
pub struct FixSessionSemanticEvent {
    /// Content hash over timestamp and session state discriminant.
    pub content_hash: u64,
    /// FNV-1a hash of the sender component ID, or `0` if not accessible.
    pub sender_hash: u64,
    /// FNV-1a hash of the target component ID, or `0` if not accessible.
    pub target_hash: u64,
    /// Next outgoing sequence number, or `0` if not accessible.
    pub outgoing_seq: u64,
    /// Next incoming sequence number expected, or `0` if not accessible.
    pub incoming_seq: u64,
    /// Timestamp supplied by the caller in nanoseconds since the Unix epoch.
    pub timestamp_ns: u64,
}

/// Convert a [`FixSession`] snapshot into a semantic telemetry event.
///
/// Only the session state is accessible through the public API.  The state
/// discriminant (0 = Disconnected, 1 = `LogonSent`, 2 = Active, 3 = `LogoutSent`)
/// is packed with `timestamp_ns` to form the `content_hash`.
#[inline]
#[must_use]
pub fn fix_session_to_semantic(session: &FixSession, timestamp_ns: u64) -> FixSessionSemanticEvent {
    use alice_fix::SessionState;

    // Map SessionState to a u8 discriminant for hashing.
    let state_disc: u8 = match session.state() {
        SessionState::Disconnected => 0,
        SessionState::LogonSent => 1,
        SessionState::Active => 2,
        SessionState::LogoutSent => 3,
    };

    // Hash input: timestamp_ns (8 bytes) || state discriminant (1 byte)
    let mut hash_data = [0u8; 9];
    hash_data[0..8].copy_from_slice(&timestamp_ns.to_le_bytes());
    hash_data[8] = state_disc;

    FixSessionSemanticEvent {
        content_hash: fnv1a(&hash_data),
        sender_hash: 0,
        target_hash: 0,
        outgoing_seq: 0,
        incoming_seq: 0,
        timestamp_ns,
    }
}

// ---------------------------------------------------------------------------
// Bridge 4: FixMessage → Risk pre-trade check input
// ---------------------------------------------------------------------------

/// Pre-trade risk check input derived from a parsed FIX `NewOrderSingle` message.
///
/// Extracts the order quantity, price, side, and symbol hash from the FIX
/// message fields so the risk layer can perform checks without inspecting
/// the raw tag/value map on the hot path.
pub struct FixRiskInput {
    /// Content hash over `order_qty`, price, side, and `symbol_hash` bytes.
    pub content_hash: u64,
    /// Order quantity from FIX tag 38 (`OrderQty`), or 0 if absent.
    pub order_qty: u64,
    /// Limit price from FIX tag 44 (Price) as f64 ticks, or 0.0 if absent.
    pub price: f64,
    /// Side byte from FIX tag 54: b'1' = Buy, b'2' = Sell, 0 if absent.
    pub side: u8,
    /// FNV-1a hash of the symbol string from FIX tag 55, or 0 if absent.
    pub symbol_hash: u64,
    /// True when `order_qty` > 1000 or price > 10000.0, indicating that a
    /// margin check is required before the order may proceed.
    pub requires_margin_check: bool,
}

/// Convert a parsed [`FixMessage`] into a [`FixRiskInput`] for pre-trade risk evaluation.
///
/// FIX tag mapping:
/// - Tag 38 (`OrderQty`)  → `order_qty`
/// - Tag 44 (Price)     → `price`
/// - Tag 54 (Side)      → `side` (first byte of the value string)
/// - Tag 55 (Symbol)    → `symbol_hash` (FNV-1a of the symbol string)
///
/// `content_hash` is computed over `order_qty || price || side || symbol_hash`
/// in little-endian byte order.
#[inline]
#[must_use]
pub fn fix_message_to_risk(msg: &FixMessage) -> FixRiskInput {
    use alice_fix::tag;

    let order_qty: u64 = msg.get_u64(tag::ORDER_QTY).unwrap_or(0);
    let price: f64 = msg
        .get(tag::PRICE)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let side: u8 = msg
        .get(tag::SIDE)
        .and_then(|s| s.bytes().next())
        .unwrap_or(0);
    let symbol_hash: u64 = msg.get(tag::SYMBOL).map_or(0, |s| fnv1a(s.as_bytes()));

    let requires_margin_check = order_qty > 1000 || price > 10000.0;

    // Hash input: order_qty (8) || price bits (8) || side (1) || symbol_hash (8)
    let mut hash_data = [0u8; 25];
    hash_data[0..8].copy_from_slice(&order_qty.to_le_bytes());
    hash_data[8..16].copy_from_slice(&price.to_bits().to_le_bytes());
    hash_data[16] = side;
    hash_data[17..25].copy_from_slice(&symbol_hash.to_le_bytes());

    FixRiskInput {
        content_hash: fnv1a(&hash_data),
        order_qty,
        price,
        side,
        symbol_hash,
        requires_margin_check,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use alice_fix::{FixMessage, FixSession};
    use alice_ledger::{Fill, OrderId};

    // ── Helpers ────────────────────────────────────────────────────────────

    fn make_fix_message(msg_type: &str, fields: &[(u32, &str)]) -> FixMessage {
        let mut msg = FixMessage::new("FIX.4.4", msg_type);
        for &(tag, val) in fields {
            msg.set(tag, val);
        }
        msg
    }

    fn make_fill(maker: u64, taker: u64, price: i64, qty: u64) -> Fill {
        Fill {
            maker_id: OrderId(maker),
            taker_id: OrderId(taker),
            price,
            quantity: qty,
            timestamp_ns: 0,
        }
    }

    // ── Bridge 1: FIX Message → Analytics ──────────────────────────────

    #[test]
    fn test_message_to_analytics() {
        let msg = make_fix_message(
            "D",
            &[
                (49, "ALICE"),
                (56, "BROKER"),
                (34, "1"),
                (11, "42"),
                (55, "BTCUSD"),
            ],
        );
        let ts = 1_700_000_000_000_000_000u64;
        let ev = fix_message_to_analytics(&msg, ts);

        // content_hash must be non-zero and deterministic.
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.timestamp_ns, ts);
        assert_eq!(ev.field_count, 5);

        // msg_type_hash must equal fnv1a(b"D").
        let expected_type_hash = {
            let mut h: u64 = 0xcbf29ce484222325;
            h ^= b'D' as u64;
            h = h.wrapping_mul(0x100000001b3);
            h
        };
        assert_eq!(ev.msg_type_hash, expected_type_hash);

        // Different message types must produce different type hashes.
        let msg2 = make_fix_message("8", &[(49, "A"), (56, "B")]);
        let ev2 = fix_message_to_analytics(&msg2, ts);
        assert_ne!(ev.msg_type_hash, ev2.msg_type_hash);

        // Different field counts must produce different content hashes.
        assert_ne!(ev.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_message_to_analytics_empty_message() {
        let msg = make_fix_message("0", &[]);
        let ev = fix_message_to_analytics(&msg, 0);
        assert_eq!(ev.field_count, 0);
        // Hash over "0" bytes + 0u32 LE must be non-zero.
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_message_to_analytics_deterministic() {
        let msg = make_fix_message("A", &[(49, "ALICE"), (56, "BROKER"), (34, "1")]);
        let ev1 = fix_message_to_analytics(&msg, 12345);
        let ev2 = fix_message_to_analytics(&msg, 12345);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.msg_type_hash, ev2.msg_type_hash);
        assert_eq!(ev1.field_count, ev2.field_count);
    }

    // ── Bridge 2: Ledger Fill → FIX ExecutionReport ─────────────────────

    #[test]
    fn test_fill_to_fix_exec_partial() {
        let fill = make_fill(1, 2, 50_000, 5);
        let ev = ledger_fill_to_fix_exec(&fill, false);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.maker_id, 1);
        assert_eq!(ev.taker_id, 2);
        assert_eq!(ev.fill_price, 50_000);
        assert_eq!(ev.fill_qty, 5);
        // PartialFill = 1
        assert_eq!(ev.exec_type, 1);
    }

    #[test]
    fn test_fill_to_fix_exec_full() {
        let fill = make_fill(10, 20, 99_999, 100);
        let ev = ledger_fill_to_fix_exec(&fill, true);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.maker_id, 10);
        assert_eq!(ev.taker_id, 20);
        assert_eq!(ev.fill_price, 99_999);
        assert_eq!(ev.fill_qty, 100);
        // Fill = 2
        assert_eq!(ev.exec_type, 2);
    }

    #[test]
    fn test_fill_exec_type_branchless_invariant() {
        // Verify that exec_type = 1 + fully_filled as u8 holds for both cases.
        let fill = make_fill(1, 2, 100, 1);
        let partial = ledger_fill_to_fix_exec(&fill, false);
        let full = ledger_fill_to_fix_exec(&fill, true);
        assert_eq!(partial.exec_type, 1);
        assert_eq!(full.exec_type, 2);
    }

    #[test]
    fn test_fill_content_hash_changes_with_price() {
        let fill1 = make_fill(1, 2, 50_000, 10);
        let fill2 = make_fill(1, 2, 51_000, 10);
        let ev1 = ledger_fill_to_fix_exec(&fill1, false);
        let ev2 = ledger_fill_to_fix_exec(&fill2, false);
        assert_ne!(ev1.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_fill_content_hash_changes_with_qty() {
        let fill1 = make_fill(3, 4, 10_000, 5);
        let fill2 = make_fill(3, 4, 10_000, 6);
        let ev1 = ledger_fill_to_fix_exec(&fill1, false);
        let ev2 = ledger_fill_to_fix_exec(&fill2, false);
        assert_ne!(ev1.content_hash, ev2.content_hash);
    }

    // ── Bridge 3: FIX Session → Semantic Telemetry ──────────────────────

    #[test]
    fn test_session_to_semantic_disconnected() {
        let session = FixSession::new("ALICE", "BROKER", "FIX.4.4");
        let ts = 1_000_000u64;
        let ev = fix_session_to_semantic(&session, ts);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.timestamp_ns, ts);
        // Fields not accessible through public API default to 0.
        assert_eq!(ev.sender_hash, 0);
        assert_eq!(ev.target_hash, 0);
        assert_eq!(ev.outgoing_seq, 0);
        assert_eq!(ev.incoming_seq, 0);
    }

    #[test]
    fn test_session_to_semantic_logon_sent() {
        let mut session = FixSession::new("ALICE", "BROKER", "FIX.4.4");
        let _ = session.build_logon();
        let ts = 2_000_000u64;
        let ev = fix_session_to_semantic(&session, ts);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.timestamp_ns, ts);
    }

    #[test]
    fn test_session_to_semantic_different_timestamps_differ() {
        let session = FixSession::new("ALICE", "BROKER", "FIX.4.4");
        let ev1 = fix_session_to_semantic(&session, 1_000);
        let ev2 = fix_session_to_semantic(&session, 2_000);
        // Different timestamps must produce different content hashes.
        assert_ne!(ev1.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_session_to_semantic_state_change_changes_hash() {
        let mut session = FixSession::new("ALICE", "BROKER", "FIX.4.4");
        let ts = 5_000u64;

        let ev_disconnected = fix_session_to_semantic(&session, ts);
        let _ = session.build_logon();
        let ev_logon_sent = fix_session_to_semantic(&session, ts);

        // Same timestamp but different state must produce different hashes.
        assert_ne!(ev_disconnected.content_hash, ev_logon_sent.content_hash);
    }

    #[test]
    fn test_session_to_semantic_deterministic() {
        let session = FixSession::new("X", "Y", "FIX.4.4");
        let ts = 9_999u64;
        let ev1 = fix_session_to_semantic(&session, ts);
        let ev2 = fix_session_to_semantic(&session, ts);
        assert_eq!(ev1.content_hash, ev2.content_hash);
    }

    // ── Bridge 4: FIX Message → Risk pre-trade check input ──────────────

    #[test]
    fn test_fix_message_to_risk_standard_order() {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(38, "500"); // OrderQty
        msg.set(44, "9500.0"); // Price
        msg.set(54, "1"); // Side: Buy
        msg.set(55, "BTCUSD"); // Symbol

        let ri = fix_message_to_risk(&msg);

        assert_ne!(ri.content_hash, 0);
        assert_eq!(ri.order_qty, 500);
        assert_eq!(ri.price, 9500.0);
        assert_eq!(ri.side, b'1');
        // symbol_hash must be non-zero for a non-empty symbol.
        assert_ne!(ri.symbol_hash, 0);
        // qty=500 <= 1000 and price=9500 <= 10000 → no margin check.
        assert!(!ri.requires_margin_check);
    }

    #[test]
    fn test_fix_message_to_risk_large_qty_triggers_margin() {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(38, "1001"); // OrderQty > 1000
        msg.set(44, "100.0"); // Price
        msg.set(54, "2"); // Side: Sell
        msg.set(55, "ETHUSD");

        let ri = fix_message_to_risk(&msg);

        assert_eq!(ri.order_qty, 1001);
        assert_eq!(ri.side, b'2');
        // qty > 1000 → requires_margin_check = true.
        assert!(ri.requires_margin_check);
    }

    #[test]
    fn test_fix_message_to_risk_high_price_triggers_margin() {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(38, "10"); // OrderQty <= 1000
        msg.set(44, "10001.0"); // Price > 10000.0
        msg.set(54, "1");
        msg.set(55, "SOLUSD");

        let ri = fix_message_to_risk(&msg);

        assert_eq!(ri.order_qty, 10);
        assert_eq!(ri.price, 10001.0);
        // price > 10000 → requires_margin_check = true.
        assert!(ri.requires_margin_check);
    }

    #[test]
    fn test_fix_message_to_risk_missing_fields_default_to_zero() {
        let msg = FixMessage::new("FIX.4.4", "D");

        let ri = fix_message_to_risk(&msg);

        assert_eq!(ri.order_qty, 0);
        assert_eq!(ri.price, 0.0);
        assert_eq!(ri.side, 0);
        assert_eq!(ri.symbol_hash, 0);
        assert!(!ri.requires_margin_check);
    }

    #[test]
    fn test_fix_message_to_risk_deterministic() {
        let mut msg = FixMessage::new("FIX.4.4", "D");
        msg.set(38, "200")
            .set(44, "5000.0")
            .set(54, "1")
            .set(55, "XRPUSD");

        let ri1 = fix_message_to_risk(&msg);
        let ri2 = fix_message_to_risk(&msg);

        assert_eq!(ri1.content_hash, ri2.content_hash);
        assert_eq!(ri1.symbol_hash, ri2.symbol_hash);
    }

    #[test]
    fn test_fix_message_to_risk_different_symbols_differ() {
        let mut msg1 = FixMessage::new("FIX.4.4", "D");
        msg1.set(38, "10")
            .set(44, "100.0")
            .set(54, "1")
            .set(55, "BTCUSD");

        let mut msg2 = FixMessage::new("FIX.4.4", "D");
        msg2.set(38, "10")
            .set(44, "100.0")
            .set(54, "1")
            .set(55, "ETHUSD");

        let ri1 = fix_message_to_risk(&msg1);
        let ri2 = fix_message_to_risk(&msg2);

        assert_ne!(ri1.symbol_hash, ri2.symbol_hash);
        assert_ne!(ri1.content_hash, ri2.content_hash);
    }
}
