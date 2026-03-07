//! Billing bridges — ALICE-Billing ↔ DB, Analytics, Ledger, Cache, Settlement
//!
//! 5 bridges connecting the billing layer to the ALICE ecosystem.
//! Covers persistent billing records in DB, revenue metrics in Analytics,
//! financial journal entries in Ledger, pricing cache, and payment settlement.

use alice_billing::Invoice;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Billing → DB (billing record persistence) ──────────────────

/// Billing record for ALICE-DB persistence.
///
/// Written when an invoice is finalized so the database layer can store
/// and query billing history by tenant, period, or invoice ID.
pub struct BillingDbRecord {
    /// FNV-1a hash over invoice ID, subtotal, tax, and total bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID string.
    pub invoice_id_hash: u64,
    /// Number of line items on the invoice.
    pub line_item_count: u32,
    /// Subtotal in cents (before tax).
    pub subtotal_cents: i64,
    /// Tax amount in cents.
    pub tax_cents: i64,
    /// Total amount in cents (subtotal + tax).
    pub total_cents: i64,
}

/// Convert an invoice into a billing record for ALICE-DB.
#[inline]
#[must_use]
pub fn billing_invoice_to_db_record(invoice: &Invoice) -> BillingDbRecord {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.subtotal.0.to_le_bytes());
    key[16..24].copy_from_slice(&invoice.tax.0.to_le_bytes());
    key[24..32].copy_from_slice(&invoice.total.0.to_le_bytes());
    BillingDbRecord {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        line_item_count: invoice.lines.len() as u32,
        subtotal_cents: invoice.subtotal.0,
        tax_cents: invoice.tax.0,
        total_cents: invoice.total.0,
    }
}

// ── Bridge 2: Billing → Analytics (revenue metrics) ──────────────────────

/// Revenue metrics event for ALICE-Analytics.
///
/// Emitted on invoice finalization so the analytics layer can compute MRR,
/// ARR, per-tenant revenue trends, and tax liability aggregates.
pub struct BillingAnalyticsRevenueEvent {
    /// FNV-1a hash over invoice ID and total bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID — analytics stream key.
    pub invoice_id_hash: u64,
    /// Total revenue in cents from this invoice.
    pub revenue_cents: i64,
    /// Tax collected in cents.
    pub tax_cents: i64,
    /// Number of billable line items.
    pub line_item_count: u32,
}

/// Convert an invoice into a revenue metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn billing_invoice_to_analytics_event(invoice: &Invoice) -> BillingAnalyticsRevenueEvent {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.total.0.to_le_bytes());
    BillingAnalyticsRevenueEvent {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        revenue_cents: invoice.total.0,
        tax_cents: invoice.tax.0,
        line_item_count: invoice.lines.len() as u32,
    }
}

// ── Bridge 3: Billing → Ledger (financial journal entry) ─────────────────

/// Financial journal entry for ALICE-Ledger.
///
/// Each finalized invoice produces a double-entry bookkeeping record so the
/// ledger layer can maintain accurate accounts-receivable and revenue accounts.
pub struct BillingLedgerEntry {
    /// FNV-1a hash over invoice ID, debit, and credit bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID.
    pub invoice_id_hash: u64,
    /// Debit amount in cents (accounts receivable increases).
    pub debit_cents: i64,
    /// Credit amount in cents (revenue account increases).
    pub credit_cents: i64,
    /// Tax credit in cents (tax payable account).
    pub tax_credit_cents: i64,
    /// Entry type: 0=revenue_recognition, 1=tax_accrual, 2=refund.
    pub entry_type: u8,
}

/// Convert an invoice into a financial journal entry for ALICE-Ledger.
#[inline]
#[must_use]
pub fn billing_invoice_to_ledger_entry(invoice: &Invoice) -> BillingLedgerEntry {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.total.0.to_le_bytes());
    key[16..24].copy_from_slice(&invoice.tax.0.to_le_bytes());
    // Debit = total (AR), credit = subtotal (revenue) + tax (tax payable).
    BillingLedgerEntry {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        debit_cents: invoice.total.0,
        credit_cents: invoice.subtotal.0,
        tax_credit_cents: invoice.tax.0,
        entry_type: 0,
    }
}

// ── Bridge 4: Billing → Cache (pricing cache entry) ──────────────────────

/// Pricing cache entry for ALICE-Cache.
///
/// Caches the computed price for a (meter, quantity) pair so repeated
/// tiered-pricing lookups avoid re-traversing the tier structure.
/// Volatile prices (> 1 000 000 cents = $10 000) receive a shorter TTL.
pub struct BillingCachePricingEntry {
    /// FNV-1a hash over meter name and quantity bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the meter name — cache key component.
    pub meter_hash: u64,
    /// Quantity used to compute this price.
    pub quantity: u64,
    /// Computed price in cents.
    pub price_cents: i64,
    /// Cache TTL in seconds: 60 for prices <= $10 000, 10 for higher.
    pub ttl_secs: u32,
}

/// Build a pricing cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: high-value prices (> 1_000_000 cents)
/// receive a 10-second TTL; normal prices receive a 60-second TTL.
#[inline]
#[must_use]
pub fn billing_to_cache_pricing_entry(
    meter_name: &str,
    quantity: u64,
    price_cents: i64,
) -> BillingCachePricingEntry {
    let meter_hash = fnv1a(meter_name.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&meter_hash.to_le_bytes());
    key[8..16].copy_from_slice(&quantity.to_le_bytes());
    // Branchless TTL: high_value=1 → 60-50=10, normal=0 → 60-0=60.
    let high_value = (price_cents > 1_000_000) as u32;
    let ttl_secs = 60 - high_value * 50;
    BillingCachePricingEntry {
        content_hash: fnv1a(&key),
        meter_hash,
        quantity,
        price_cents,
        ttl_secs,
    }
}

// ── Bridge 5: Billing → Settlement (payment settlement request) ───────────

/// Payment settlement request for ALICE-Settlement.
///
/// Converts a finalized invoice into a settlement instruction so the
/// settlement engine can initiate the payment collection workflow.
pub struct BillingSettlementRequest {
    /// FNV-1a hash over invoice ID and total bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the invoice ID.
    pub invoice_id_hash: u64,
    /// Total amount due in cents.
    pub amount_cents: i64,
    /// Number of line items — used by settlement for itemized breakdowns.
    pub line_item_count: u32,
    /// Settlement method: 0=standard (T+2), 1=instant.
    pub settlement_method: u8,
    /// Number of days until settlement (2 for standard, 0 for instant).
    pub settlement_days: u8,
}

/// Convert a finalized invoice into a settlement request for ALICE-Settlement.
///
/// Invoices over $100 000 (10_000_000 cents) use standard T+2 settlement;
/// smaller invoices use instant settlement (0 days).
#[inline]
#[must_use]
pub fn billing_invoice_to_settlement_request(invoice: &Invoice) -> BillingSettlementRequest {
    let invoice_id_hash = fnv1a(invoice.id.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&invoice_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&invoice.total.0.to_le_bytes());
    // Large invoice (> $100k) → T+2 (method=0, days=2); else instant (method=1, days=0).
    let is_large = (invoice.total.0 > 10_000_000) as u8;
    let settlement_method = 1 - is_large; // large=T+2(0), small=instant(1)
    let settlement_days = is_large * 2;
    BillingSettlementRequest {
        content_hash: fnv1a(&key),
        invoice_id_hash,
        amount_cents: invoice.total.0,
        line_item_count: invoice.lines.len() as u32,
        settlement_method,
        settlement_days,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_billing::{generate_invoice, InvoiceLine, Amount};

    fn make_invoice(id: &str, lines: Vec<InvoiceLine>, tax_rate_bps: u32) -> Invoice {
        generate_invoice(id.to_string(), lines, tax_rate_bps)
    }

    fn make_line_item(desc: &str, qty: u64, unit_price_cents: i64) -> InvoiceLine {
        InvoiceLine {
            description: desc.to_string(),
            quantity: qty,
            unit_price: Amount::new(unit_price_cents),
            total: Amount::new(qty as i64 * unit_price_cents),
        }
    }

    #[test]
    fn test_billing_invoice_to_db_record_basic() {
        let inv = make_invoice(
            "inv-001",
            vec![make_line_item("Pro plan", 1, 9900)],
            1000,
        );
        let rec = billing_invoice_to_db_record(&inv);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.invoice_id_hash, 0);
        assert_eq!(rec.line_item_count, 1);
        assert_eq!(rec.subtotal_cents, 9900);
    }

    #[test]
    fn test_billing_invoice_to_db_record_tax_calculation() {
        let inv = make_invoice(
            "inv-002",
            vec![make_line_item("Enterprise", 1, 100_000)],
            800, // 8%
        );
        let rec = billing_invoice_to_db_record(&inv);
        assert_eq!(rec.subtotal_cents, 100_000);
        assert_eq!(rec.tax_cents, 8_000);
        assert_eq!(rec.total_cents, 108_000);
    }

    #[test]
    fn test_billing_invoice_to_analytics_event() {
        let inv = make_invoice(
            "inv-003",
            vec![
                make_line_item("API calls", 1000, 1),
                make_line_item("Storage", 50, 200),
            ],
            500,
        );
        let ev = billing_invoice_to_analytics_event(&inv);
        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.invoice_id_hash, 0);
        assert_eq!(ev.line_item_count, 2);
    }

    #[test]
    fn test_billing_invoice_to_ledger_entry() {
        let inv = make_invoice(
            "inv-004",
            vec![make_line_item("SaaS sub", 1, 50_000)],
            1000,
        );
        let entry = billing_invoice_to_ledger_entry(&inv);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.debit_cents, inv.total.0);
        assert_eq!(entry.credit_cents, inv.subtotal.0);
        assert_eq!(entry.tax_credit_cents, inv.tax.0);
        assert_eq!(entry.entry_type, 0);
    }

    #[test]
    fn test_billing_to_cache_pricing_entry_normal_ttl() {
        // price <= $10 000 → ttl = 60
        let entry = billing_to_cache_pricing_entry("api-calls", 1000, 500_000);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.meter_hash, 0);
        assert_eq!(entry.quantity, 1000);
        assert_eq!(entry.price_cents, 500_000);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_billing_to_cache_pricing_entry_high_value_ttl() {
        // price > $10 000 → ttl = 10
        let entry = billing_to_cache_pricing_entry("enterprise-tier", 500, 1_500_000);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 10);
    }

    #[test]
    fn test_billing_invoice_to_settlement_small() {
        // total <= $100k → instant (method=1, days=0)
        let inv = make_invoice("inv-005", vec![make_line_item("Basic", 1, 9900)], 0);
        let req = billing_invoice_to_settlement_request(&inv);
        assert_ne!(req.content_hash, 0);
        assert_eq!(req.settlement_method, 1);
        assert_eq!(req.settlement_days, 0);
        assert_eq!(req.amount_cents, inv.total.0);
    }

    #[test]
    fn test_billing_invoice_to_settlement_large() {
        // total > $100k → T+2 (method=0, days=2)
        let inv = make_invoice(
            "inv-006",
            vec![make_line_item("Enterprise annual", 1, 12_000_000)],
            0,
        );
        let req = billing_invoice_to_settlement_request(&inv);
        assert_ne!(req.content_hash, 0);
        assert_eq!(req.settlement_method, 0);
        assert_eq!(req.settlement_days, 2);
    }

    #[test]
    fn test_billing_hash_determinism() {
        let inv = make_invoice("inv-007", vec![make_line_item("Plan X", 1, 2900)], 0);
        let rec1 = billing_invoice_to_db_record(&inv);
        let rec2 = billing_invoice_to_db_record(&inv);
        assert_eq!(rec1.content_hash, rec2.content_hash);
        assert_eq!(rec1.invoice_id_hash, rec2.invoice_id_hash);
    }

    #[test]
    fn test_billing_usage_meter_zero_quantity() {
        // Zero-quantity pricing → price = 0, ttl = 60 (not high value).
        let entry = billing_to_cache_pricing_entry("free-tier", 0, 0);
        assert_eq!(entry.quantity, 0);
        assert_eq!(entry.price_cents, 0);
        assert_eq!(entry.ttl_secs, 60);
    }
}
