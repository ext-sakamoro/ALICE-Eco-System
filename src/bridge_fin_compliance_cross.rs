//! Cross-domain bridges — ALICE-FinCompliance ↔ Risk/Ledger/FIX
//!
//! 5 bridges connecting financial compliance rules to risk limits,
//! ledger position checks, FIX message audit, risk reject alerts,
//! and cache records.

use alice_fincompliance::{AmlAlert, CapitalRatio};
use alice_fix::message::FixMessage;
use alice_fix::tag;
use alice_ledger::position::Position;
use alice_risk::check::RiskReject;
use alice_risk::limit::RiskLimits;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: FinCompliance rule → Risk limit ───────────────────────────

/// Risk limit record derived from FinCompliance capital ratio analysis.
///
/// Maps Basel III capital adequacy metrics into risk limit parameters
/// so the Risk layer can dynamically adjust position limits based on
/// capital health.
pub struct FinComplianceRiskLimit {
    /// FNV-1a hash over `cet1_bps`, `tier1_bps`, `total_bps`, `leverage_bps`, `violations_count`.
    pub content_hash: u64,
    /// CET1 ratio in basis points (e.g. 900 = 9.0%).
    pub cet1_bps: u32,
    /// Tier 1 ratio in basis points.
    pub tier1_bps: u32,
    /// Total capital ratio in basis points.
    pub total_bps: u32,
    /// Leverage ratio in basis points.
    pub leverage_bps: u32,
    /// Number of Basel III minimum violations.
    pub violations_count: usize,
    /// Whether any Basel III minimum is breached.
    pub is_capital_deficient: bool,
}

/// Convert a FinCompliance capital ratio into a risk limit record.
#[inline]
#[must_use]
pub fn fin_compliance_rule_to_risk_limit(
    ratio: &CapitalRatio,
    violations: &[String],
) -> FinComplianceRiskLimit {
    let cet1_bps = (ratio.cet1_ratio * 10000.0) as u32;
    let tier1_bps = (ratio.tier1_ratio * 10000.0) as u32;
    let total_bps = (ratio.total_ratio * 10000.0) as u32;
    let leverage_bps = (ratio.leverage_ratio * 10000.0) as u32;
    let violations_count = violations.len();
    let is_capital_deficient = !violations.is_empty();

    let mut key = [0u8; 24];
    key[0..4].copy_from_slice(&cet1_bps.to_le_bytes());
    key[4..8].copy_from_slice(&tier1_bps.to_le_bytes());
    key[8..12].copy_from_slice(&total_bps.to_le_bytes());
    key[12..16].copy_from_slice(&leverage_bps.to_le_bytes());
    key[16..24].copy_from_slice(&(violations_count as u64).to_le_bytes());

    FinComplianceRiskLimit {
        content_hash: fnv1a(&key),
        cet1_bps,
        tier1_bps,
        total_bps,
        leverage_bps,
        violations_count,
        is_capital_deficient,
    }
}

// ── Bridge 2: Ledger Position → compliance check ────────────────────────

/// Compliance check record derived from a Ledger Position.
///
/// Maps open position data into financial compliance domain so
/// concentration limits and position-based regulatory checks
/// can be evaluated.
pub struct FinCompliancePositionCheck {
    /// FNV-1a hash over `symbol_hash`, `net_quantity`, `realized_pnl`, `unrealized_pnl`, `trade_count`.
    pub content_hash: u64,
    /// Symbol hash from the position.
    pub symbol_hash: u64,
    /// Signed net position size.
    pub net_quantity: i64,
    /// Realized P&L in ticks.
    pub realized_pnl: i64,
    /// Unrealized P&L in ticks.
    pub unrealized_pnl: i64,
    /// Total trade count for this position.
    pub trade_count: u64,
    /// Whether position exceeds regulatory threshold (|net| > 500).
    pub exceeds_threshold: bool,
}

/// Convert a Ledger Position into a compliance check record.
#[inline]
#[must_use]
pub fn fin_compliance_ledger_position_to_check(
    position: &Position,
    threshold: i64,
) -> FinCompliancePositionCheck {
    let abs_qty = position.net_quantity.unsigned_abs() as i64;
    let exceeds_threshold = abs_qty > threshold;

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&position.symbol_hash.to_le_bytes());
    key[8..16].copy_from_slice(&position.net_quantity.to_le_bytes());
    key[16..24].copy_from_slice(&position.realized_pnl.to_le_bytes());
    key[24..32].copy_from_slice(&position.unrealized_pnl.to_le_bytes());
    key[32..40].copy_from_slice(&position.trade_count.to_le_bytes());

    FinCompliancePositionCheck {
        content_hash: fnv1a(&key),
        symbol_hash: position.symbol_hash,
        net_quantity: position.net_quantity,
        realized_pnl: position.realized_pnl,
        unrealized_pnl: position.unrealized_pnl,
        trade_count: position.trade_count,
        exceeds_threshold,
    }
}

// ── Bridge 3: FIX message → compliance audit ────────────────────────────

/// Compliance audit record derived from a FIX message.
///
/// Extracts MiFID II relevant fields from FIX messages (sender, target,
/// message type, symbol, order attributes) for regulatory audit trail
/// and best execution reporting.
pub struct FinComplianceFixAudit {
    /// FNV-1a hash over `sender_hash`, `target_hash`, `msg_type_hash`, `seq_num`, `symbol_hash`.
    pub content_hash: u64,
    /// FNV-1a hash of SenderCompID (tag 49).
    pub sender_hash: u64,
    /// FNV-1a hash of TargetCompID (tag 56).
    pub target_hash: u64,
    /// FNV-1a hash of MsgType (tag 35).
    pub msg_type_hash: u64,
    /// Message sequence number (tag 34), 0 if absent.
    pub seq_num: u64,
    /// FNV-1a hash of Symbol (tag 55), 0 if absent.
    pub symbol_hash: u64,
    /// Whether this is an order-related message (D, F, G, 8).
    pub is_order_message: bool,
}

/// Convert a FIX message into a compliance audit record.
#[inline]
#[must_use]
pub fn fin_compliance_fix_message_to_audit(msg: &FixMessage) -> FinComplianceFixAudit {
    let sender_hash = msg
        .get(tag::SENDER_COMP_ID)
        .map(|s| fnv1a(s.as_bytes()))
        .unwrap_or(0);
    let target_hash = msg
        .get(tag::TARGET_COMP_ID)
        .map(|s| fnv1a(s.as_bytes()))
        .unwrap_or(0);
    let msg_type_hash = fnv1a(msg.msg_type.as_bytes());
    let seq_num = msg.get_u64(tag::MSG_SEQ_NUM).unwrap_or(0);
    let symbol_hash = msg
        .get(tag::SYMBOL)
        .map(|s| fnv1a(s.as_bytes()))
        .unwrap_or(0);

    // 注文関連メッセージ: D=NewOrderSingle, F=CancelRequest, G=CancelReplace, 8=ExecutionReport
    let is_order_message = matches!(msg.msg_type.as_str(), "D" | "F" | "G" | "8");

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&sender_hash.to_le_bytes());
    key[8..16].copy_from_slice(&target_hash.to_le_bytes());
    key[16..24].copy_from_slice(&msg_type_hash.to_le_bytes());
    key[24..32].copy_from_slice(&seq_num.to_le_bytes());
    key[32..40].copy_from_slice(&symbol_hash.to_le_bytes());

    FinComplianceFixAudit {
        content_hash: fnv1a(&key),
        sender_hash,
        target_hash,
        msg_type_hash,
        seq_num,
        symbol_hash,
        is_order_message,
    }
}

// ── Bridge 4: Risk reject → compliance alert ────────────────────────────

/// Compliance alert record derived from a Risk reject event.
///
/// Maps risk rejection reasons into compliance alert domain for
/// regulatory reporting of blocked orders and risk limit breaches.
pub struct FinComplianceRiskAlert {
    /// FNV-1a hash over `reject_type`, `detail_a`, `detail_b`, `limit_value`.
    pub content_hash: u64,
    /// Reject reason type as u8 discriminant.
    pub reject_type: u8,
    /// First detail value (meaning varies by reject type).
    pub detail_a: i64,
    /// Second detail value (meaning varies by reject type).
    pub detail_b: i64,
    /// Configured limit that was breached.
    pub limit_value: i64,
    /// Whether this is a kill-switch level alert.
    pub is_kill_switch: bool,
}

/// Convert a Risk reject into a compliance alert record.
#[inline]
#[must_use]
pub fn fin_compliance_risk_reject_to_alert(reject: &RiskReject) -> FinComplianceRiskAlert {
    let (reject_type, detail_a, detail_b, limit_value, is_kill_switch) = match reject {
        RiskReject::PositionLimitBreached {
            current,
            after,
            limit,
        } => (0u8, *current, *after, *limit as i64, false),
        RiskReject::OrderSizeTooLarge { size, limit } => {
            (1u8, *size as i64, 0i64, *limit as i64, false)
        }
        RiskReject::NotionalExceeded { notional, limit } => {
            (2u8, *notional, 0i64, *limit, false)
        }
        RiskReject::MaxOpenOrdersReached { count, limit } => {
            (3u8, *count as i64, 0i64, *limit as i64, false)
        }
        RiskReject::DailyLossLimitHit { loss, limit } => (4u8, *loss, 0i64, *limit, true),
        RiskReject::CircuitBreakerTripped => (5u8, 0i64, 0i64, 0i64, true),
    };

    let mut key = [0u8; 25];
    key[0] = reject_type;
    key[1..9].copy_from_slice(&detail_a.to_le_bytes());
    key[9..17].copy_from_slice(&detail_b.to_le_bytes());
    key[17..25].copy_from_slice(&limit_value.to_le_bytes());

    FinComplianceRiskAlert {
        content_hash: fnv1a(&key),
        reject_type,
        detail_a,
        detail_b,
        limit_value,
        is_kill_switch,
    }
}

// ── Bridge 5: compliance check result → cache ───────────────────────────

/// Cache record for financial compliance check results.
///
/// Provides a cacheable summary of AML alert data with branchless TTL:
/// high risk scores get short TTL (60s), low scores get long TTL (1800s).
pub struct FinComplianceCheckCache {
    /// FNV-1a hash over `rule_hash`, `txn_count`, `risk_score_bps`, `ttl_secs`.
    pub content_hash: u64,
    /// FNV-1a hash of the alert rule name.
    pub rule_hash: u64,
    /// Number of flagged transactions.
    pub txn_count: usize,
    /// Risk score in basis points (e.g. 150 = 1.50).
    pub risk_score_bps: u32,
    /// Branchless TTL: high risk (>100bps)=60s, low risk=1800s.
    pub ttl_secs: u32,
}

/// Convert an AML alert into a compliance check cache record.
#[inline]
#[must_use]
pub fn fin_compliance_check_to_cache(alert: &AmlAlert) -> FinComplianceCheckCache {
    let rule_hash = fnv1a(alert.rule.as_bytes());
    let txn_count = alert.transactions.len();
    let risk_score_bps = (alert.risk_score * 100.0) as u32;

    // Branchless TTL: 高リスク(>100bps=1.0x)→60s, 低リスク→1800s
    let is_high_risk = (risk_score_bps > 100) as u32;
    let ttl_secs = 1800 - is_high_risk * 1740;

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&rule_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(txn_count as u64).to_le_bytes());
    key[16..20].copy_from_slice(&risk_score_bps.to_le_bytes());
    key[20..24].copy_from_slice(&ttl_secs.to_le_bytes());

    FinComplianceCheckCache {
        content_hash: fnv1a(&key),
        rule_hash,
        txn_count,
        risk_score_bps,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_fincompliance::{calculate_capital_ratios, check_basel3_minimums, AmlAlert};
    use alice_fix::message::FixMessage;
    use alice_fix::tag;
    use alice_ledger::position::Position;
    use alice_risk::check::RiskReject;

    // ── Bridge 1: capital ratio → risk limit ────────────────────────────

    #[test]
    fn test_fin_compliance_rule_to_risk_limit_healthy() {
        let ratio = calculate_capital_ratios(50.0, 10.0, 10.0, 500.0, 1000.0);
        let violations = check_basel3_minimums(&ratio);
        let limit = fin_compliance_rule_to_risk_limit(&ratio, &violations);
        assert_ne!(limit.content_hash, 0);
        assert_eq!(limit.cet1_bps, 1000); // 10.0%
        assert_eq!(limit.violations_count, 0);
        assert!(!limit.is_capital_deficient);
    }

    #[test]
    fn test_fin_compliance_rule_to_risk_limit_deficient() {
        let ratio = calculate_capital_ratios(10.0, 5.0, 5.0, 500.0, 1000.0);
        let violations = check_basel3_minimums(&ratio);
        let limit = fin_compliance_rule_to_risk_limit(&ratio, &violations);
        assert!(limit.violations_count > 0);
        assert!(limit.is_capital_deficient);
    }

    // ── Bridge 2: position → compliance check ───────────────────────────

    #[test]
    fn test_fin_compliance_ledger_position_to_check() {
        let pos = Position {
            symbol_hash: 0xABCD,
            net_quantity: 100,
            avg_entry_price: 50_000,
            realized_pnl: 1000,
            unrealized_pnl: 500,
            trade_count: 10,
        };
        let check = fin_compliance_ledger_position_to_check(&pos, 500);
        assert_ne!(check.content_hash, 0);
        assert_eq!(check.symbol_hash, 0xABCD);
        assert_eq!(check.net_quantity, 100);
        assert!(!check.exceeds_threshold); // 100 <= 500
    }

    #[test]
    fn test_fin_compliance_position_exceeds_threshold() {
        let pos = Position {
            symbol_hash: 0xBEEF,
            net_quantity: -1000,
            avg_entry_price: 45_000,
            realized_pnl: -5000,
            unrealized_pnl: -2000,
            trade_count: 50,
        };
        let check = fin_compliance_ledger_position_to_check(&pos, 500);
        assert!(check.exceeds_threshold); // |-1000| = 1000 > 500
    }

    // ── Bridge 3: FIX message → compliance audit ────────────────────────

    #[test]
    fn test_fin_compliance_fix_message_to_audit_order() {
        let mut msg = FixMessage::new("FIX.4.4", "D"); // NewOrderSingle
        msg.set(tag::SENDER_COMP_ID, "ALICE");
        msg.set(tag::TARGET_COMP_ID, "BROKER");
        msg.set(tag::MSG_SEQ_NUM, "42");
        msg.set(tag::SYMBOL, "BTCUSD");

        let audit = fin_compliance_fix_message_to_audit(&msg);
        assert_ne!(audit.content_hash, 0);
        assert_ne!(audit.sender_hash, 0);
        assert_ne!(audit.target_hash, 0);
        assert_eq!(audit.seq_num, 42);
        assert_ne!(audit.symbol_hash, 0);
        assert!(audit.is_order_message); // "D" is order message
    }

    #[test]
    fn test_fin_compliance_fix_message_heartbeat() {
        let mut msg = FixMessage::new("FIX.4.4", "0"); // Heartbeat
        msg.set(tag::SENDER_COMP_ID, "ALICE");
        msg.set(tag::TARGET_COMP_ID, "BROKER");

        let audit = fin_compliance_fix_message_to_audit(&msg);
        assert!(!audit.is_order_message); // "0" is not order message
        assert_eq!(audit.symbol_hash, 0); // No symbol in heartbeat
    }

    // ── Bridge 4: risk reject → compliance alert ────────────────────────

    #[test]
    fn test_fin_compliance_risk_reject_position() {
        let reject = RiskReject::PositionLimitBreached {
            current: 900,
            after: 1100,
            limit: 1000,
        };
        let alert = fin_compliance_risk_reject_to_alert(&reject);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.reject_type, 0); // PositionLimitBreached
        assert_eq!(alert.detail_a, 900);
        assert_eq!(alert.detail_b, 1100);
        assert_eq!(alert.limit_value, 1000);
        assert!(!alert.is_kill_switch);
    }

    #[test]
    fn test_fin_compliance_risk_reject_kill_switch() {
        let reject = RiskReject::DailyLossLimitHit {
            loss: -600_000,
            limit: -500_000,
        };
        let alert = fin_compliance_risk_reject_to_alert(&reject);
        assert_eq!(alert.reject_type, 4); // DailyLossLimitHit
        assert!(alert.is_kill_switch);
    }

    #[test]
    fn test_fin_compliance_risk_reject_circuit_breaker() {
        let reject = RiskReject::CircuitBreakerTripped;
        let alert = fin_compliance_risk_reject_to_alert(&reject);
        assert_eq!(alert.reject_type, 5);
        assert!(alert.is_kill_switch);
    }

    // ── Bridge 5: AML alert → cache ─────────────────────────────────────

    #[test]
    fn test_fin_compliance_check_to_cache_high_risk() {
        let alert = AmlAlert {
            rule: String::from("LARGE_TXN"),
            transactions: vec![String::from("t1"), String::from("t2")],
            risk_score: 5.0, // 500 bps, high risk
        };
        let cache = fin_compliance_check_to_cache(&alert);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.txn_count, 2);
        assert_eq!(cache.risk_score_bps, 500);
        assert_eq!(cache.ttl_secs, 60); // 高リスク: 短いTTL
    }

    #[test]
    fn test_fin_compliance_check_to_cache_low_risk() {
        let alert = AmlAlert {
            rule: String::from("STRUCTURING"),
            transactions: vec![String::from("t1")],
            risk_score: 0.5, // 50 bps, low risk
        };
        let cache = fin_compliance_check_to_cache(&alert);
        assert_eq!(cache.risk_score_bps, 50);
        assert_eq!(cache.ttl_secs, 1800); // 低リスク: 長いTTL
    }
}
