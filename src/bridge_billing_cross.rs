//! Cross-domain bridges — ALICE-Billing ↔ Ledger/Risk/Settlement
//!
//! 5 bridges connecting billing invoices to ledger journal entries,
//! risk exposure checks, settlement trades, and cache records.

use alice_billing::Invoice;
use alice_ledger::book::Fill;
use alice_settlement::trade::{SettlementStatus, Trade};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Invoice → Ledger journal entry ────────────────────────────

/// Ledger journal entry derived from a billing invoice.
///
/// Encodes invoice identity, line count, and total amount so the Ledger
/// layer can create a journal entry for double-entry bookkeeping without
/// parsing the full invoice structure.
pub struct BillingLedgerEntry {
    /// FNV-1a hash over `invoice_id_hash`, `line_count`, `subtotal`, `tax`, `total`.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID string.
    pub invoice_id_hash: u64,
    /// Number of line items in the invoice.
    pub line_count: usize,
    /// Subtotal in minor currency units.
    pub subtotal: i64,
    /// Tax amount in minor currency units.
    pub tax: i64,
    /// Grand total in minor currency units.
    pub total: i64,
}

/// Convert a billing invoice into a ledger journal entry.
#[inline]
#[must_use]
pub fn billing_invoice_to_ledger_entry(invoice: &Invoice) -> BillingLedgerEntry {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let line_count = invoice.lines.len();

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(line_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&invoice.subtotal.0.to_le_bytes());
    key[24..32].copy_from_slice(&invoice.tax.0.to_le_bytes());
    key[32..40].copy_from_slice(&invoice.total.0.to_le_bytes());

    BillingLedgerEntry {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        line_count,
        subtotal: invoice.subtotal.0,
        tax: invoice.tax.0,
        total: invoice.total.0,
    }
}

// ── Bridge 2: Invoice → Risk exposure check ─────────────────────────────

/// Risk exposure record derived from a billing invoice.
///
/// Encodes invoice total as notional exposure so the Risk layer can
/// include billing commitments in aggregate exposure calculations.
pub struct BillingRiskExposure {
    /// FNV-1a hash over `invoice_id_hash`, `total`, `line_count`, `max_line_total`.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID string.
    pub invoice_id_hash: u64,
    /// Total invoice amount as notional exposure (minor units).
    pub notional_exposure: i64,
    /// Number of line items.
    pub line_count: usize,
    /// Largest single line item total (concentration risk).
    pub max_line_total: i64,
    /// Whether concentration risk exists (max line > 50% of total).
    pub concentration_flag: bool,
}

/// Convert a billing invoice into a risk exposure record.
#[inline]
#[must_use]
pub fn billing_invoice_to_risk_exposure(invoice: &Invoice) -> BillingRiskExposure {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let line_count = invoice.lines.len();
    let max_line_total = invoice.lines.iter().map(|l| l.total.0).max().unwrap_or(0);
    let concentration_flag = invoice.total.0 > 0 && max_line_total * 2 > invoice.total.0;

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.total.0.to_le_bytes());
    key[16..24].copy_from_slice(&(line_count as u64).to_le_bytes());
    key[24..32].copy_from_slice(&max_line_total.to_le_bytes());

    BillingRiskExposure {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        notional_exposure: invoice.total.0,
        line_count,
        max_line_total,
        concentration_flag,
    }
}

// ── Bridge 3: Invoice → Settlement trade ────────────────────────────────

/// Settlement trade record derived from a billing invoice.
///
/// Maps a billing invoice into a settlement trade so the Settlement
/// layer can track payment settlement lifecycle alongside financial
/// trade settlement.
pub struct BillingSettlementTrade {
    /// FNV-1a hash over `invoice_id_hash`, `total`, `line_count`, `status`.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID string.
    pub invoice_id_hash: u64,
    /// Invoice total as settlement amount (minor units).
    pub settlement_amount: i64,
    /// Number of line items.
    pub line_count: usize,
    /// Settlement status: always `Pending` for new invoices.
    pub status: u8,
}

/// Convert a billing invoice into a settlement trade record.
#[inline]
#[must_use]
pub fn billing_invoice_to_settlement_trade(invoice: &Invoice) -> BillingSettlementTrade {
    // 新規請求書は常にPending状態
    let status: u8 = 0; // SettlementStatus::Pending

    let mut key = [0u8; 25];
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.total.0.to_le_bytes());
    key[16..24].copy_from_slice(&(invoice.lines.len() as u64).to_le_bytes());
    key[24] = status;

    BillingSettlementTrade {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        settlement_amount: invoice.total.0,
        line_count: invoice.lines.len(),
        status,
    }
}

// ── Bridge 4: Ledger Fill → billing reconciliation ──────────────────────

/// Billing reconciliation record derived from a Ledger Fill.
///
/// Maps a matching engine fill back into billing domain so revenue
/// from traded fills can be reconciled against invoiced amounts.
pub struct BillingFillReconciliation {
    /// FNV-1a hash over `maker_id`, `taker_id`, `price`, `quantity`, `notional`.
    pub content_hash: u64,
    /// Maker order ID from the fill.
    pub maker_id: u64,
    /// Taker order ID from the fill.
    pub taker_id: u64,
    /// Execution price in ticks.
    pub price: i64,
    /// Executed quantity.
    pub quantity: u64,
    /// Notional value (price * quantity) for billing purposes.
    pub notional: i64,
}

/// Convert a Ledger Fill into a billing reconciliation record.
#[inline]
#[must_use]
pub fn billing_ledger_fill_to_invoice_line(fill: &Fill) -> BillingFillReconciliation {
    let notional = fill.price * fill.quantity as i64;

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&fill.maker_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&fill.taker_id.0.to_le_bytes());
    key[16..24].copy_from_slice(&fill.price.to_le_bytes());
    key[24..32].copy_from_slice(&fill.quantity.to_le_bytes());
    key[32..40].copy_from_slice(&notional.to_le_bytes());

    BillingFillReconciliation {
        content_hash: fnv1a(&key),
        maker_id: fill.maker_id.0,
        taker_id: fill.taker_id.0,
        price: fill.price,
        quantity: fill.quantity,
        notional,
    }
}

// ── Bridge 5: SettlementStatus → billing cache ──────────────────────────

/// Billing cache record derived from a settlement trade status.
///
/// Encodes settlement lifecycle status and timing so the Cache layer
/// can serve billing status queries with branchless TTL selection:
/// settled invoices get long TTL, pending ones get short TTL.
pub struct BillingSettlementCache {
    /// FNV-1a hash over `trade_id`, `symbol_hash`, `status`, `amount`, `ttl_secs`.
    pub content_hash: u64,
    /// Trade identifier.
    pub trade_id: u64,
    /// Symbol hash from the trade.
    pub symbol_hash: u64,
    /// Settlement status as u8 discriminant.
    pub status: u8,
    /// Settlement amount (price * quantity).
    pub amount: i64,
    /// Branchless TTL: settled=3600s, pending=60s.
    pub ttl_secs: u32,
}

/// Convert a Settlement Trade into a billing cache record.
#[inline]
#[must_use]
pub fn billing_settlement_status_to_cache(trade: &Trade) -> BillingSettlementCache {
    let status_byte = match trade.status {
        SettlementStatus::Pending => 0,
        SettlementStatus::Netted => 1,
        SettlementStatus::Cleared => 2,
        SettlementStatus::Settled => 3,
        SettlementStatus::Failed => 4,
    };

    let amount = trade.price * trade.quantity as i64;

    // Branchless TTL: settled(3)→3600s, それ以外→60s
    let is_settled = (status_byte == 3) as u32;
    let ttl_secs = 60 + is_settled * 3540;

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&trade.trade_id.to_le_bytes());
    key[8..16].copy_from_slice(&trade.symbol_hash.to_le_bytes());
    key[16] = status_byte;
    key[17..25].copy_from_slice(&amount.to_le_bytes());
    key[25..33].copy_from_slice(&(ttl_secs as u64).to_le_bytes());

    BillingSettlementCache {
        content_hash: fnv1a(&key),
        trade_id: trade.trade_id,
        symbol_hash: trade.symbol_hash,
        status: status_byte,
        amount,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_billing::{generate_invoice, Amount, InvoiceLine};
    use alice_ledger::book::Fill;
    use alice_ledger::order::OrderId;
    use alice_settlement::trade::{SettlementStatus, Trade};

    fn sample_invoice() -> Invoice {
        let lines = vec![
            InvoiceLine {
                description: String::from("API利用料"),
                quantity: 10000,
                unit_price: Amount::new(1),
                total: Amount::new(10000),
            },
            InvoiceLine {
                description: String::from("ストレージ"),
                quantity: 500,
                unit_price: Amount::new(5),
                total: Amount::new(2500),
            },
        ];
        generate_invoice(String::from("INV-001"), lines, 1000)
    }

    fn sample_fill() -> Fill {
        Fill {
            maker_id: OrderId(100),
            taker_id: OrderId(200),
            price: 50_000,
            quantity: 10,
            timestamp_ns: 1_700_000_000_000_000_000,
        }
    }

    fn sample_trade(status: SettlementStatus) -> Trade {
        Trade {
            trade_id: 1,
            symbol_hash: 0xABCD,
            buyer_id: 100,
            seller_id: 200,
            price: 50_000,
            quantity: 10,
            timestamp_ns: 1_700_000_000_000_000_000,
            status,
        }
    }

    // ── Bridge 1: invoice → ledger entry ────────────────────────────────

    #[test]
    fn test_billing_invoice_to_ledger_entry() {
        let invoice = sample_invoice();
        let entry = billing_invoice_to_ledger_entry(&invoice);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.invoice_id_hash, 0);
        assert_eq!(entry.line_count, 2);
        assert_eq!(entry.subtotal, 12500);
        assert_eq!(entry.tax, 1250);
        assert_eq!(entry.total, 13750);
    }

    #[test]
    fn test_billing_invoice_to_ledger_entry_deterministic() {
        let invoice = sample_invoice();
        let e1 = billing_invoice_to_ledger_entry(&invoice);
        let e2 = billing_invoice_to_ledger_entry(&invoice);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 2: invoice → risk exposure ───────────────────────────────

    #[test]
    fn test_billing_invoice_to_risk_exposure() {
        let invoice = sample_invoice();
        let exp = billing_invoice_to_risk_exposure(&invoice);
        assert_ne!(exp.content_hash, 0);
        assert_eq!(exp.notional_exposure, 13750);
        assert_eq!(exp.line_count, 2);
        assert_eq!(exp.max_line_total, 10000);
        // 10000 * 2 = 20000 > 13750 → concentration
        assert!(exp.concentration_flag);
    }

    #[test]
    fn test_billing_invoice_to_risk_exposure_no_concentration() {
        let lines = vec![
            InvoiceLine {
                description: String::from("A"),
                quantity: 1,
                unit_price: Amount::new(100),
                total: Amount::new(100),
            },
            InvoiceLine {
                description: String::from("B"),
                quantity: 1,
                unit_price: Amount::new(100),
                total: Amount::new(100),
            },
            InvoiceLine {
                description: String::from("C"),
                quantity: 1,
                unit_price: Amount::new(100),
                total: Amount::new(100),
            },
        ];
        let invoice = generate_invoice(String::from("INV-002"), lines, 0);
        let exp = billing_invoice_to_risk_exposure(&invoice);
        // max=100, total=300, 100*2=200 < 300 → no concentration
        assert!(!exp.concentration_flag);
    }

    // ── Bridge 3: invoice → settlement trade ────────────────────────────

    #[test]
    fn test_billing_invoice_to_settlement_trade() {
        let invoice = sample_invoice();
        let st = billing_invoice_to_settlement_trade(&invoice);
        assert_ne!(st.content_hash, 0);
        assert_eq!(st.settlement_amount, 13750);
        assert_eq!(st.line_count, 2);
        assert_eq!(st.status, 0); // Pending
    }

    // ── Bridge 4: fill → billing reconciliation ─────────────────────────

    #[test]
    fn test_billing_ledger_fill_to_invoice_line() {
        let fill = sample_fill();
        let rec = billing_ledger_fill_to_invoice_line(&fill);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.maker_id, 100);
        assert_eq!(rec.taker_id, 200);
        assert_eq!(rec.price, 50_000);
        assert_eq!(rec.quantity, 10);
        assert_eq!(rec.notional, 500_000);
    }

    #[test]
    fn test_billing_ledger_fill_to_invoice_line_deterministic() {
        let fill = sample_fill();
        let r1 = billing_ledger_fill_to_invoice_line(&fill);
        let r2 = billing_ledger_fill_to_invoice_line(&fill);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 5: settlement status → cache ─────────────────────────────

    #[test]
    fn test_billing_settlement_cache_settled() {
        let trade = sample_trade(SettlementStatus::Settled);
        let cache = billing_settlement_status_to_cache(&trade);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.trade_id, 1);
        assert_eq!(cache.status, 3); // Settled
        assert_eq!(cache.amount, 500_000);
        assert_eq!(cache.ttl_secs, 3600); // 長いTTL
    }

    #[test]
    fn test_billing_settlement_cache_pending() {
        let trade = sample_trade(SettlementStatus::Pending);
        let cache = billing_settlement_status_to_cache(&trade);
        assert_eq!(cache.status, 0); // Pending
        assert_eq!(cache.ttl_secs, 60); // 短いTTL
    }
}
