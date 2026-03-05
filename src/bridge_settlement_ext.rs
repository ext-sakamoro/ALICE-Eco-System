//! Settlement extended bridges — ALICE-Settlement ↔ DB, Analytics, Queue, Semantic Telemetry
//!
//! 4 bridges connecting the settlement engine to the ALICE ecosystem.

use alice_settlement::{JournalEntry, JournalEvent, NetObligation, SettlementStatus, Trade};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Settlement Trade → DB (trade persistence) ─────────────────

/// Persistent trade record for ALICE-DB.
///
/// Captures the full trade lifecycle state so the database layer can store
/// and query confirmed trades by trade ID, symbol, or settlement status.
pub struct SettlementDbTradeRecord {
    /// FNV-1a hash over `trade_id`, `symbol_hash`, price, and quantity bytes.
    pub content_hash: u64,
    /// Unique trade identifier.
    pub trade_id: u64,
    /// Symbol hash (FNV-derived).
    pub symbol_hash: u64,
    /// Buyer account identifier.
    pub buyer_id: u64,
    /// Seller account identifier.
    pub seller_id: u64,
    /// Execution price in ticks.
    pub price_ticks: i64,
    /// Trade quantity in lots.
    pub quantity: u64,
    /// Settlement status code: Pending=0, Netted=1, Cleared=2, Settled=3, Failed=4.
    pub status: u8,
    /// Execution timestamp (nanoseconds since Unix epoch).
    pub timestamp_ns: u64,
}

/// Map a `SettlementStatus` variant to its numeric status code.
///
/// Pending=0, Netted=1, Cleared=2, Settled=3, Failed=4.
#[inline(always)]
const fn status_to_u8(status: SettlementStatus) -> u8 {
    match status {
        SettlementStatus::Pending => 0,
        SettlementStatus::Netted => 1,
        SettlementStatus::Cleared => 2,
        SettlementStatus::Settled => 3,
        SettlementStatus::Failed => 4,
    }
}

/// Convert a settlement `Trade` into a `SettlementDbTradeRecord` for persistence.
///
/// The `content_hash` is computed as FNV-1a over the concatenation of
/// `trade_id`, `symbol_hash`, `price`, and `quantity` in little-endian byte order.
#[inline]
#[must_use]
pub fn settlement_trade_to_db(trade: &Trade) -> SettlementDbTradeRecord {
    let data: Vec<u8> = [
        trade.trade_id.to_le_bytes().as_slice(),
        trade.symbol_hash.to_le_bytes().as_slice(),
        trade.price.to_le_bytes().as_slice(),
        trade.quantity.to_le_bytes().as_slice(),
    ]
    .concat();
    let content_hash = fnv1a(&data);

    SettlementDbTradeRecord {
        content_hash,
        trade_id: trade.trade_id,
        symbol_hash: trade.symbol_hash,
        buyer_id: trade.buyer_id,
        seller_id: trade.seller_id,
        price_ticks: trade.price,
        quantity: trade.quantity,
        status: status_to_u8(trade.status),
        timestamp_ns: trade.timestamp_ns,
    }
}

// ── Bridge 2: Settlement Journal → Analytics (settlement metrics) ────────

/// Analytics event derived from a settlement journal entry.
///
/// Provides a compact, typed representation of journal events for the
/// analytics layer to aggregate settlement throughput and failure rates.
pub struct SettlementAnalyticsEvent {
    /// FNV-1a hash over sequence bytes and `event_type` byte.
    pub content_hash: u64,
    /// Event type code: TradeReceived=1, NettingCompleted=2,
    /// ClearingAttempted=3, SettlementCompleted=4, SettlementFailed=5.
    pub event_type: u8,
    /// Journal sequence number.
    pub sequence: u64,
    /// Journal entry timestamp (nanoseconds since Unix epoch).
    pub timestamp_ns: u64,
}

/// Map a `JournalEvent` variant to its numeric event type code.
///
/// TradeReceived=1, NettingCompleted=2, ClearingAttempted=3,
/// SettlementCompleted=4, SettlementFailed=5.
#[inline(always)]
const fn journal_event_to_type(event: &JournalEvent) -> u8 {
    match event {
        JournalEvent::TradeReceived { .. } => 1,
        JournalEvent::NettingCompleted { .. } => 2,
        JournalEvent::ClearingAttempted { .. } => 3,
        JournalEvent::SettlementCompleted { .. } => 4,
        JournalEvent::SettlementFailed { .. } => 5,
    }
}

/// Convert a settlement `JournalEntry` into a `SettlementAnalyticsEvent`.
///
/// The `content_hash` is computed as FNV-1a over the concatenation of
/// the sequence bytes (little-endian u64) and the single `event_type` byte.
#[inline]
#[must_use]
pub fn settlement_journal_entry_to_analytics(entry: &JournalEntry) -> SettlementAnalyticsEvent {
    let event_type = journal_event_to_type(&entry.event);
    let data: Vec<u8> = [entry.sequence.to_le_bytes().as_slice(), &[event_type]].concat();
    let content_hash = fnv1a(&data);

    SettlementAnalyticsEvent {
        content_hash,
        event_type,
        sequence: entry.sequence,
        timestamp_ns: entry.timestamp_ns,
    }
}

// ── Bridge 3: Settlement Net Obligation → Queue (async clearing) ─────────

/// Clearing message enqueued for async processing by ALICE-Queue.
///
/// Carries the full bilateral net obligation so the clearing house can
/// process deliveries asynchronously without blocking the settlement engine.
pub struct SettlementQueueClearingMsg {
    /// FNV-1a hash over `deliverer_id`, `receiver_id`, `symbol_hash`, and `net_quantity` bytes.
    pub content_hash: u64,
    /// Account that owes delivery (net seller).
    pub deliverer_id: u64,
    /// Account that receives delivery (net buyer).
    pub receiver_id: u64,
    /// Symbol hash identifying the instrument.
    pub symbol_hash: u64,
    /// Net quantity to deliver in lots.
    pub net_quantity: u64,
    /// Net payment amount (positive = receiver pays deliverer).
    pub net_payment: i64,
    /// Queue priority: always 2 (high priority for settlement clearing).
    pub priority: u8,
}

/// Convert a `NetObligation` into a `SettlementQueueClearingMsg` for async clearing.
///
/// The `content_hash` is computed as FNV-1a over the concatenation of
/// `deliverer_id`, `receiver_id`, `symbol_hash`, and `net_quantity` in
/// little-endian byte order.  Priority is always 2 (high).
#[inline]
#[must_use]
pub fn settlement_obligation_to_queue(oblig: &NetObligation) -> SettlementQueueClearingMsg {
    let data: Vec<u8> = [
        oblig.deliverer_id.to_le_bytes().as_slice(),
        oblig.receiver_id.to_le_bytes().as_slice(),
        oblig.symbol_hash.to_le_bytes().as_slice(),
        oblig.net_quantity.to_le_bytes().as_slice(),
    ]
    .concat();
    let content_hash = fnv1a(&data);

    SettlementQueueClearingMsg {
        content_hash,
        deliverer_id: oblig.deliverer_id,
        receiver_id: oblig.receiver_id,
        symbol_hash: oblig.symbol_hash,
        net_quantity: oblig.net_quantity,
        net_payment: oblig.net_payment,
        priority: 2,
    }
}

// ── Bridge 4: Settlement Trade → Semantic Telemetry (trade lifecycle) ────

/// Semantic telemetry event derived from a settlement trade.
///
/// Emits a severity-tagged lifecycle event for each trade so the telemetry
/// layer can surface failures immediately while treating normal progression
/// as informational events.
pub struct SettlementSemanticEvent {
    /// FNV-1a hash over `trade_id`, status, and severity bytes.
    pub content_hash: u64,
    /// Unique trade identifier.
    pub trade_id: u64,
    /// Settlement status code (same mapping as Bridge 1).
    pub status: u8,
    /// Severity: Failed=3, all other statuses=1.
    pub severity: u8,
    /// Trade execution timestamp (nanoseconds since Unix epoch).
    pub timestamp_ns: u64,
}

/// Map a `SettlementStatus` to a severity level.
///
/// Failed=3, Pending/Netted/Cleared/Settled=1.
#[inline(always)]
const fn status_to_severity(status: SettlementStatus) -> u8 {
    match status {
        SettlementStatus::Failed => 3,
        _ => 1,
    }
}

/// Convert a settlement `Trade` into a `SettlementSemanticEvent` for telemetry.
///
/// The `content_hash` is computed as FNV-1a over the concatenation of
/// `trade_id` bytes (little-endian u64), the status byte, and the severity byte.
#[inline]
#[must_use]
pub fn settlement_trade_to_semantic(trade: &Trade) -> SettlementSemanticEvent {
    let status = status_to_u8(trade.status);
    let severity = status_to_severity(trade.status);
    let data: Vec<u8> = [
        trade.trade_id.to_le_bytes().as_slice(),
        &[status],
        &[severity],
    ]
    .concat();
    let content_hash = fnv1a(&data);

    SettlementSemanticEvent {
        content_hash,
        trade_id: trade.trade_id,
        status,
        severity,
        timestamp_ns: trade.timestamp_ns,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_settlement::{JournalEvent, NetObligation, SettlementStatus, Trade};

    fn make_trade(trade_id: u64, status: SettlementStatus) -> Trade {
        Trade {
            trade_id,
            symbol_hash: 0xDEAD_BEEF,
            buyer_id: 100,
            seller_id: 200,
            price: 50_000,
            quantity: 10,
            timestamp_ns: 1_700_000_000_000_000_000,
            status,
        }
    }

    fn make_journal_entry(sequence: u64, event: JournalEvent) -> JournalEntry {
        JournalEntry {
            sequence,
            timestamp_ns: 1_700_000_000_000_000_000,
            event,
        }
    }

    fn make_obligation() -> NetObligation {
        NetObligation {
            symbol_hash: 0xABCD_1234,
            deliverer_id: 200,
            receiver_id: 100,
            net_quantity: 50,
            net_payment: 2_500_000,
            trade_count: 5,
        }
    }

    // ── Bridge 1: Settlement Trade → DB ──────────────────────────────────

    #[test]
    fn test_trade_to_db_pending() {
        let trade = make_trade(1, SettlementStatus::Pending);
        let rec = settlement_trade_to_db(&trade);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.trade_id, 1);
        assert_eq!(rec.symbol_hash, 0xDEAD_BEEF);
        assert_eq!(rec.buyer_id, 100);
        assert_eq!(rec.seller_id, 200);
        assert_eq!(rec.price_ticks, 50_000);
        assert_eq!(rec.quantity, 10);
        assert_eq!(rec.status, 0, "Pending must map to status 0");
        assert_eq!(rec.timestamp_ns, 1_700_000_000_000_000_000);
    }

    #[test]
    fn test_trade_to_db_settled() {
        let trade = make_trade(2, SettlementStatus::Settled);
        let rec = settlement_trade_to_db(&trade);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.trade_id, 2);
        assert_eq!(rec.status, 3, "Settled must map to status 3");
    }

    #[test]
    fn test_trade_to_db_failed() {
        let trade = make_trade(3, SettlementStatus::Failed);
        let rec = settlement_trade_to_db(&trade);

        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.trade_id, 3);
        assert_eq!(rec.status, 4, "Failed must map to status 4");
    }

    #[test]
    fn test_trade_to_db_status_coverage() {
        // Verify all status codes are distinct and correct.
        let pending = settlement_trade_to_db(&make_trade(10, SettlementStatus::Pending));
        let netted = settlement_trade_to_db(&make_trade(11, SettlementStatus::Netted));
        let cleared = settlement_trade_to_db(&make_trade(12, SettlementStatus::Cleared));
        let settled = settlement_trade_to_db(&make_trade(13, SettlementStatus::Settled));
        let failed = settlement_trade_to_db(&make_trade(14, SettlementStatus::Failed));

        assert_eq!(pending.status, 0);
        assert_eq!(netted.status, 1);
        assert_eq!(cleared.status, 2);
        assert_eq!(settled.status, 3);
        assert_eq!(failed.status, 4);
    }

    #[test]
    fn test_trade_to_db_hash_deterministic() {
        let trade = make_trade(42, SettlementStatus::Cleared);
        let r1 = settlement_trade_to_db(&trade);
        let r2 = settlement_trade_to_db(&trade);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2: Settlement Journal → Analytics ─────────────────────────

    #[test]
    fn test_journal_entry_to_analytics_trade_received() {
        let entry = make_journal_entry(1, JournalEvent::TradeReceived { trade_id: 99 });
        let ev = settlement_journal_entry_to_analytics(&entry);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_type, 1, "TradeReceived must map to event_type 1");
        assert_eq!(ev.sequence, 1);
        assert_eq!(ev.timestamp_ns, 1_700_000_000_000_000_000);
    }

    #[test]
    fn test_journal_entry_to_analytics_netting_completed() {
        let entry = make_journal_entry(
            2,
            JournalEvent::NettingCompleted {
                obligation_count: 3,
            },
        );
        let ev = settlement_journal_entry_to_analytics(&entry);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(
            ev.event_type, 2,
            "NettingCompleted must map to event_type 2"
        );
        assert_eq!(ev.sequence, 2);
    }

    #[test]
    fn test_journal_entry_to_analytics_clearing_attempted() {
        let entry = make_journal_entry(
            3,
            JournalEvent::ClearingAttempted {
                obligation_count: 5,
                success_count: 4,
                fail_count: 1,
            },
        );
        let ev = settlement_journal_entry_to_analytics(&entry);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(
            ev.event_type, 3,
            "ClearingAttempted must map to event_type 3"
        );
        assert_eq!(ev.sequence, 3);
    }

    #[test]
    fn test_journal_entry_to_analytics_settlement_completed() {
        let entry = make_journal_entry(4, JournalEvent::SettlementCompleted { trade_count: 10 });
        let ev = settlement_journal_entry_to_analytics(&entry);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(
            ev.event_type, 4,
            "SettlementCompleted must map to event_type 4"
        );
        assert_eq!(ev.sequence, 4);
    }

    #[test]
    fn test_journal_entry_to_analytics_settlement_failed() {
        let entry = make_journal_entry(
            5,
            JournalEvent::SettlementFailed {
                trade_id: 77,
                reason: "insufficient funds".to_string(),
            },
        );
        let ev = settlement_journal_entry_to_analytics(&entry);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(
            ev.event_type, 5,
            "SettlementFailed must map to event_type 5"
        );
        assert_eq!(ev.sequence, 5);
    }

    #[test]
    fn test_journal_entry_to_analytics_hash_deterministic() {
        let entry = make_journal_entry(7, JournalEvent::TradeReceived { trade_id: 1 });
        let e1 = settlement_journal_entry_to_analytics(&entry);
        let e2 = settlement_journal_entry_to_analytics(&entry);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_journal_entry_to_analytics_different_types_differ() {
        // Different event types with the same sequence must produce different hashes.
        let e1 = settlement_journal_entry_to_analytics(&make_journal_entry(
            1,
            JournalEvent::TradeReceived { trade_id: 1 },
        ));
        let e2 = settlement_journal_entry_to_analytics(&make_journal_entry(
            1,
            JournalEvent::NettingCompleted {
                obligation_count: 0,
            },
        ));
        assert_ne!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 3: Settlement Net Obligation → Queue ───────────────────────

    #[test]
    fn test_obligation_to_queue() {
        let oblig = make_obligation();
        let msg = settlement_obligation_to_queue(&oblig);

        assert_ne!(msg.content_hash, 0);
        assert_eq!(msg.deliverer_id, 200);
        assert_eq!(msg.receiver_id, 100);
        assert_eq!(msg.symbol_hash, 0xABCD_1234);
        assert_eq!(msg.net_quantity, 50);
        assert_eq!(msg.net_payment, 2_500_000);
        assert_eq!(
            msg.priority, 2,
            "settlement clearing is always high priority (2)"
        );
    }

    #[test]
    fn test_obligation_to_queue_hash_deterministic() {
        let oblig = make_obligation();
        let m1 = settlement_obligation_to_queue(&oblig);
        let m2 = settlement_obligation_to_queue(&oblig);
        assert_eq!(m1.content_hash, m2.content_hash);
    }

    #[test]
    fn test_obligation_to_queue_different_obligors_differ() {
        let oblig1 = make_obligation();
        let mut oblig2 = make_obligation();
        oblig2.deliverer_id = 999;
        let m1 = settlement_obligation_to_queue(&oblig1);
        let m2 = settlement_obligation_to_queue(&oblig2);
        assert_ne!(m1.content_hash, m2.content_hash);
    }

    // ── Bridge 4: Settlement Trade → Semantic Telemetry ──────────────────

    #[test]
    fn test_trade_to_semantic_failed() {
        let trade = make_trade(5, SettlementStatus::Failed);
        let ev = settlement_trade_to_semantic(&trade);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.trade_id, 5);
        assert_eq!(ev.status, 4, "Failed must map to status 4");
        assert_eq!(ev.severity, 3, "Failed must have severity 3");
        assert_eq!(ev.timestamp_ns, 1_700_000_000_000_000_000);
    }

    #[test]
    fn test_trade_to_semantic_settled() {
        let trade = make_trade(6, SettlementStatus::Settled);
        let ev = settlement_trade_to_semantic(&trade);

        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.trade_id, 6);
        assert_eq!(ev.status, 3, "Settled must map to status 3");
        assert_eq!(ev.severity, 1, "Settled must have severity 1");
    }

    #[test]
    fn test_trade_to_semantic_severity_non_failed_statuses() {
        // All non-Failed statuses must produce severity 1.
        for (id, status) in [
            (20u64, SettlementStatus::Pending),
            (21, SettlementStatus::Netted),
            (22, SettlementStatus::Cleared),
            (23, SettlementStatus::Settled),
        ] {
            let ev = settlement_trade_to_semantic(&make_trade(id, status));
            assert_eq!(
                ev.severity, 1,
                "status {:?} (id={}) should have severity 1, got {}",
                status, id, ev.severity
            );
        }
    }

    #[test]
    fn test_trade_to_semantic_hash_deterministic() {
        let trade = make_trade(99, SettlementStatus::Pending);
        let e1 = settlement_trade_to_semantic(&trade);
        let e2 = settlement_trade_to_semantic(&trade);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_trade_to_semantic_failed_vs_settled_hash_differ() {
        // Same trade_id but different status must produce different content_hash.
        let failed = settlement_trade_to_semantic(&make_trade(7, SettlementStatus::Failed));
        let settled = settlement_trade_to_semantic(&make_trade(7, SettlementStatus::Settled));
        assert_ne!(failed.content_hash, settled.content_hash);
    }
}
