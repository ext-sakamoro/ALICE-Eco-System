//! FinCompliance bridges — ALICE-FinCompliance ↔ DB, Analytics, Risk, Ledger, FIX
//!
//! 5 bridges connecting ALICE-FinCompliance (Basel III capital ratios, VaR,
//! AML alert detection, RWA calculation) to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: FinCompliance → DB (compliance record persistence) ──────────

/// Financial compliance record for ALICE-DB persistence.
///
/// Stores a point-in-time snapshot of Basel III capital ratios so that
/// regulators and risk managers can audit capital adequacy over time.
pub struct FinComplianceDbRecord {
    /// FNV-1a hash of `entity_id || snapshot_timestamp` — primary DB key.
    pub content_hash: u64,
    /// FNV-1a hash of the entity identifier for entity-scoped queries.
    pub entity_hash: u64,
    /// CET1 ratio (×10000 as u32; e.g. 0.09 → 900).
    pub cet1_ratio_bps: u32,
    /// Tier 1 ratio (×10000 as u32).
    pub tier1_ratio_bps: u32,
    /// Total capital ratio (×10000 as u32).
    pub total_ratio_bps: u32,
    /// Leverage ratio (×10000 as u32).
    pub leverage_ratio_bps: u32,
    /// 1 when all Basel III minimums are met, 0 otherwise (branchless).
    pub meets_minimums: u8,
    /// Snapshot timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Serialize a Basel III capital ratio snapshot for ALICE-DB.
///
/// # Optimization notes
/// - All ratios stored as basis points (×10000) to avoid f64 serialization.
///   Clamp to [0, 65535] to fit u32 with room for >100% ratios.
/// - content_hash is derived over a 16-byte buffer packing `entity_hash`
///   and `timestamp_ms` — two values, one FNV pass, no heap allocation.
/// - `meets_minimums` is derived branchlessly from four comparison results
///   combined via bitwise AND, then cast to u8.
#[inline]
#[must_use]
pub fn fin_compliance_to_db_record(
    entity_id: &str,
    cet1_ratio: f64,
    tier1_ratio: f64,
    total_ratio: f64,
    leverage_ratio: f64,
    timestamp_ms: u64,
) -> FinComplianceDbRecord {
    let entity_hash = fnv1a(entity_id.as_bytes());

    // Composite content_hash over entity_hash and timestamp.
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&entity_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    let content_hash = fnv1a(&buf);

    // Store ratios as basis points (×10000), clamped to u32 range.
    let to_bps = |r: f64| -> u32 { (r * 10_000.0).clamp(0.0, 65_535.0) as u32 };

    // Branchless Basel III minimum checks (4.5%, 6.0%, 8.0%, 3.0%).
    let meets_minimums = ((cet1_ratio >= 0.045)
        & (tier1_ratio >= 0.06)
        & (total_ratio >= 0.08)
        & (leverage_ratio >= 0.03)) as u8;

    FinComplianceDbRecord {
        content_hash,
        entity_hash,
        cet1_ratio_bps: to_bps(cet1_ratio),
        tier1_ratio_bps: to_bps(tier1_ratio),
        total_ratio_bps: to_bps(total_ratio),
        leverage_ratio_bps: to_bps(leverage_ratio),
        meets_minimums,
        timestamp_ms,
    }
}

// ── Bridge 2: FinCompliance → Analytics (risk metrics) ───────────────────

/// Risk metrics event for ALICE-Analytics.
///
/// Publishes VaR estimates and AML alert counts to the analytics pipeline
/// for real-time risk dashboard updates.
pub struct FinComplianceAnalyticsEvent {
    /// FNV-1a hash of the portfolio identifier — analytics stream key.
    pub content_hash: u64,
    /// Historical VaR (×100 as i64; e.g. 12345.67 → 1_234_567).
    pub var_centis: i64,
    /// Parametric VaR (×100 as i64).
    pub parametric_var_centis: i64,
    /// Confidence level (×10000 as u32; e.g. 0.95 → 9500).
    pub confidence_bps: u32,
    /// Number of AML alerts generated in the observation window.
    pub aml_alert_count: u32,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a risk metrics analytics event from VaR and AML alert data.
///
/// # Optimization notes
/// - VaR values stored as centis (×100) to avoid f64 serialization.
/// - confidence stored as basis points (×10000) for lossless integer storage.
#[inline]
#[must_use]
pub fn fin_compliance_to_analytics_event(
    portfolio_id: &str,
    historical_var: f64,
    parametric_var: f64,
    confidence: f64,
    aml_alert_count: u32,
    timestamp_ms: u64,
) -> FinComplianceAnalyticsEvent {
    let content_hash = fnv1a(portfolio_id.as_bytes());
    let var_centis = (historical_var * 100.0) as i64;
    let parametric_var_centis = (parametric_var * 100.0) as i64;
    let confidence_bps = (confidence * 10_000.0).clamp(0.0, 10_000.0) as u32;

    FinComplianceAnalyticsEvent {
        content_hash,
        var_centis,
        parametric_var_centis,
        confidence_bps,
        aml_alert_count,
        timestamp_ms,
    }
}

// ── Bridge 3: FinCompliance → Risk (risk limits integration) ─────────────

/// Risk limit integration record for ALICE-Risk.
///
/// Forwards AML alert metadata to the risk engine so that pre-trade checks
/// can be tightened for flagged counterparties.
pub struct FinComplianceRiskRecord {
    /// FNV-1a hash of the transaction identifier — risk record key.
    pub content_hash: u64,
    /// FNV-1a hash of the alert rule identifier.
    pub rule_hash: u64,
    /// AML risk score (×100 as u32; e.g. 2.75 → 275).
    pub risk_score_centis: u32,
    /// Transaction amount in minor currency units.
    pub amount: i64,
    /// Alert rule type: 0=LARGE_TXN, 1=STRUCTURING.
    pub alert_type: u8,
    /// Alert timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a risk integration record from an AML alert.
///
/// # Optimization notes
/// - `alert_type` is passed as a pre-mapped u8 value; callers map
///   the `AmlAlert.rule` string via explicit comparison (no `as u8` cast).
/// - risk_score stored as centis (×100) for integer precision.
/// - Two FNV passes (txn_id + rule); no heap allocation.
#[inline]
#[must_use]
pub fn fin_compliance_to_risk_record(
    txn_id: &str,
    rule: &str,
    risk_score: f64,
    amount: i64,
    alert_type: u8,
    timestamp_ms: u64,
) -> FinComplianceRiskRecord {
    let content_hash = fnv1a(txn_id.as_bytes());
    let rule_hash = fnv1a(rule.as_bytes());
    let risk_score_centis = (risk_score * 100.0).clamp(0.0, 999_999.0) as u32;

    FinComplianceRiskRecord {
        content_hash,
        rule_hash,
        risk_score_centis,
        amount,
        alert_type,
        timestamp_ms,
    }
}

// ── Bridge 4: FinCompliance → Ledger (capital tracking) ──────────────────

/// Capital tracking record for ALICE-Ledger.
///
/// Posts risk-weighted asset totals to the ledger so that capital
/// consumption can be tracked against regulatory limits in real time.
pub struct FinComplianceLedgerRecord {
    /// FNV-1a hash of `entity_id || asset_class_code` — ledger entry key.
    pub content_hash: u64,
    /// Asset class code: 0=Sovereign, 1=Bank, 2=Corporate, 3=Retail, 4=Mortgage.
    pub asset_class: u8,
    /// Exposure amount in minor currency units.
    pub exposure: i64,
    /// Risk weight (×10000 as u32; e.g. 0.35 → 3500).
    pub risk_weight_bps: u32,
    /// Risk-weighted asset amount in minor currency units.
    pub rwa: i64,
    /// Record timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a ledger capital tracking record from an RWA calculation.
///
/// # Optimization notes
/// - `rwa` is computed as `exposure * risk_weight` in floating-point then
///   rounded to the nearest integer unit; no heap allocation.
/// - content_hash packs `entity_hash` and `asset_class_code` into a 9-byte
///   buffer for a single FNV pass.
#[inline]
#[must_use]
pub fn fin_compliance_to_ledger_record(
    entity_id: &str,
    asset_class: u8,
    exposure: i64,
    risk_weight: f64,
    timestamp_ms: u64,
) -> FinComplianceLedgerRecord {
    let entity_hash = fnv1a(entity_id.as_bytes());

    // Pack entity_hash (8 bytes) + asset_class (1 byte) for composite key.
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&entity_hash.to_le_bytes());
    buf[8] = asset_class;
    let content_hash = fnv1a(&buf);

    let risk_weight_bps = (risk_weight * 10_000.0).clamp(0.0, 20_000.0) as u32;
    // RWA = exposure × risk_weight, rounded to nearest integer.
    let rwa = (exposure as f64 * risk_weight).round() as i64;

    FinComplianceLedgerRecord {
        content_hash,
        asset_class,
        exposure,
        risk_weight_bps,
        rwa,
        timestamp_ms,
    }
}

// ── Bridge 5: FinCompliance → FIX (trade compliance) ─────────────────────

/// Trade compliance record for ALICE-FIX order routing.
///
/// Attaches compliance clearance metadata to an outbound FIX order so that
/// the gateway can confirm pre-trade checks before submission to the exchange.
pub struct FinComplianceFixRecord {
    /// FNV-1a hash of the FIX order identifier (ClOrdID) — routing key.
    pub content_hash: u64,
    /// 1 when the order passed all pre-trade compliance checks, 0 otherwise.
    pub compliance_pass: u8,
    /// VaR contribution of this order (×100 as i64).
    pub order_var_centis: i64,
    /// Capital consumption in minor currency units after this order.
    pub capital_consumed: i64,
    /// Number of AML rule violations associated with this order (0=clear).
    pub aml_violations: u32,
    /// Compliance check timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Build a FIX trade compliance record for pre-trade gateway validation.
///
/// # Optimization notes
/// - `compliance_pass` is derived branchlessly: `(aml_violations == 0
///   && var_within_limit) as u8` — two comparisons, one cast, no branch.
/// - order_var_centis stores VaR×100 for integer precision.
#[inline]
#[must_use]
pub fn fin_compliance_to_fix_record(
    cl_ord_id: &str,
    order_var: f64,
    capital_consumed: i64,
    aml_violations: u32,
    var_limit: f64,
    timestamp_ms: u64,
) -> FinComplianceFixRecord {
    let content_hash = fnv1a(cl_ord_id.as_bytes());
    let order_var_centis = (order_var * 100.0) as i64;

    // Branchless compliance pass: no AML violations AND VaR within limit.
    let var_within_limit = order_var <= var_limit;
    let compliance_pass = (aml_violations == 0 && var_within_limit) as u8;

    FinComplianceFixRecord {
        content_hash,
        compliance_pass,
        order_var_centis,
        capital_consumed,
        aml_violations,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ENTITY_ID: &str = "bank-entity-001";
    const PORTFOLIO_ID: &str = "portfolio-alpha";
    const TXN_ID: &str = "txn-20260307-001";
    const CL_ORD_ID: &str = "order-fix-20260307-42";

    #[test]
    fn test_db_record_meets_minimums() {
        // CET1=9%, Tier1=12%, Total=16%, Leverage=5% — all above Basel III minimums.
        let rec = fin_compliance_to_db_record(ENTITY_ID, 0.09, 0.12, 0.16, 0.05, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.entity_hash, 0);
        assert_eq!(rec.meets_minimums, 1);
        assert_eq!(rec.cet1_ratio_bps, 900); // 0.09 × 10000
        assert_eq!(rec.tier1_ratio_bps, 1_200);
        assert_eq!(rec.total_ratio_bps, 1_600);
        assert_eq!(rec.leverage_ratio_bps, 500);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_db_record_fails_minimums() {
        // CET1=3% — below 4.5% minimum.
        let rec = fin_compliance_to_db_record(ENTITY_ID, 0.03, 0.04, 0.06, 0.02, 0);
        assert_eq!(rec.meets_minimums, 0);
    }

    #[test]
    fn test_db_record_hash_determinism() {
        let a = fin_compliance_to_db_record(ENTITY_ID, 0.09, 0.12, 0.16, 0.05, 1_000);
        let b = fin_compliance_to_db_record(ENTITY_ID, 0.09, 0.12, 0.16, 0.05, 1_000);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_analytics_event_basic() {
        let ev = fin_compliance_to_analytics_event(
            PORTFOLIO_ID,
            12_345.67,
            11_000.0,
            0.95,
            2,
            1_700_000_001_000,
        );
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.var_centis, 1_234_567); // 12345.67 × 100
        assert_eq!(ev.parametric_var_centis, 1_100_000);
        assert_eq!(ev.confidence_bps, 9_500); // 0.95 × 10000
        assert_eq!(ev.aml_alert_count, 2);
        assert_eq!(ev.timestamp_ms, 1_700_000_001_000);
    }

    #[test]
    fn test_analytics_event_confidence_clamp() {
        // confidence > 1.0 → clamped to 10000 bps.
        let ev = fin_compliance_to_analytics_event(PORTFOLIO_ID, 0.0, 0.0, 1.5, 0, 0);
        assert_eq!(ev.confidence_bps, 10_000);
    }

    #[test]
    fn test_risk_record_large_txn() {
        let rec =
            fin_compliance_to_risk_record(TXN_ID, "LARGE_TXN", 5.0, 500_000, 0, 1_700_000_002_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.rule_hash, 0);
        assert_eq!(rec.risk_score_centis, 500); // 5.0 × 100
        assert_eq!(rec.amount, 500_000);
        assert_eq!(rec.alert_type, 0);
        assert_eq!(rec.timestamp_ms, 1_700_000_002_000);
    }

    #[test]
    fn test_risk_record_structuring() {
        let rec = fin_compliance_to_risk_record("txn-99", "STRUCTURING", 2.5, 45_000, 1, 0);
        assert_eq!(rec.alert_type, 1);
        assert_eq!(rec.risk_score_centis, 250);
    }

    #[test]
    fn test_ledger_record_corporate_rwa() {
        // Corporate: risk_weight = 1.0 → rwa == exposure.
        let rec = fin_compliance_to_ledger_record(ENTITY_ID, 2, 200_000, 1.0, 1_700_000_003_000);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.asset_class, 2); // Corporate
        assert_eq!(rec.exposure, 200_000);
        assert_eq!(rec.risk_weight_bps, 10_000); // 1.0 × 10000
        assert_eq!(rec.rwa, 200_000);
        assert_eq!(rec.timestamp_ms, 1_700_000_003_000);
    }

    #[test]
    fn test_ledger_record_mortgage_rwa() {
        // Mortgage: risk_weight = 0.35 → rwa = 300000 × 0.35 = 105000.
        let rec = fin_compliance_to_ledger_record(ENTITY_ID, 4, 300_000, 0.35, 0);
        assert_eq!(rec.risk_weight_bps, 3_500);
        assert_eq!(rec.rwa, 105_000);
    }

    #[test]
    fn test_fix_record_compliant_order() {
        // 0 AML violations, VaR below limit → compliance_pass = 1.
        let rec = fin_compliance_to_fix_record(
            CL_ORD_ID,
            5_000.0,
            1_000_000,
            0,
            10_000.0,
            1_700_000_004_000,
        );
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.compliance_pass, 1);
        assert_eq!(rec.order_var_centis, 500_000); // 5000.0 × 100
        assert_eq!(rec.capital_consumed, 1_000_000);
        assert_eq!(rec.aml_violations, 0);
    }

    #[test]
    fn test_fix_record_aml_violation_blocks_compliance() {
        // AML violation present → compliance_pass = 0.
        let rec = fin_compliance_to_fix_record(CL_ORD_ID, 1_000.0, 500_000, 1, 10_000.0, 0);
        assert_eq!(rec.compliance_pass, 0);
        assert_eq!(rec.aml_violations, 1);
    }

    #[test]
    fn test_fix_record_var_exceeds_limit() {
        // VaR above limit → compliance_pass = 0 even with no AML violations.
        let rec = fin_compliance_to_fix_record(CL_ORD_ID, 15_000.0, 500_000, 0, 10_000.0, 0);
        assert_eq!(rec.compliance_pass, 0);
    }
}
