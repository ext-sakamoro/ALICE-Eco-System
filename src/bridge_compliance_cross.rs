//! Cross-domain bridges — ALICE-Compliance ↔ Legal/LegalAI
//!
//! 5 bridges connecting compliance rules and violations to legal audit
//! entries, LegalAI clause analysis, contract rule checks, analytics
//! records, and cache entries.

use alice_compliance::{ComplianceRule, Regulation, Severity, Violation};
use alice_legal::{Contract, ContractStatus};
use alice_legal_ai::{Clause, ClauseType, RiskLevel};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: ComplianceRule → Legal audit entry ────────────────────────

/// Legal audit entry derived from a compliance rule.
///
/// Maps compliance rule identity, regulation type, and severity into a
/// legal audit record so the Legal layer can maintain an audit trail
/// of compliance rules applied to legal entities.
pub struct ComplianceLegalAudit {
    /// FNV-1a hash over `rule_id_hash`, `regulation`, `severity`, `description_hash`.
    pub content_hash: u64,
    /// FNV-1a hash of the rule ID string.
    pub rule_id_hash: u64,
    /// Regulation type as u8 discriminant.
    pub regulation: u8,
    /// Severity level as u8 discriminant.
    pub severity: u8,
    /// FNV-1a hash of the rule description.
    pub description_hash: u64,
}

/// Convert a compliance rule into a legal audit entry.
#[inline]
#[must_use]
pub fn compliance_rule_to_legal_audit(rule: &ComplianceRule) -> ComplianceLegalAudit {
    let rule_id_hash = fnv1a(rule.id.as_bytes());
    let regulation_byte = match rule.regulation {
        Regulation::Gdpr => 0,
        Regulation::Sox => 1,
        Regulation::Hipaa => 2,
        Regulation::Pci => 3,
        Regulation::Iso27001 => 4,
    };
    let severity_byte = match rule.severity {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    };
    let description_hash = fnv1a(rule.description.as_bytes());

    let mut key = [0u8; 26];
    key[0..8].copy_from_slice(&rule_id_hash.to_le_bytes());
    key[8] = regulation_byte;
    key[9] = severity_byte;
    key[10..18].copy_from_slice(&description_hash.to_le_bytes());
    // パディング (整列用)
    key[18..26].copy_from_slice(&0u64.to_le_bytes());

    ComplianceLegalAudit {
        content_hash: fnv1a(&key),
        rule_id_hash,
        regulation: regulation_byte,
        severity: severity_byte,
        description_hash,
    }
}

// ── Bridge 2: Violation → LegalAI clause analysis ───────────────────────

/// LegalAI clause analysis record derived from a compliance violation.
///
/// Maps violation details into LegalAI analysis domain so the AI engine
/// can identify relevant contract clauses related to the violation.
pub struct ComplianceLegalAiClause {
    /// FNV-1a hash over `rule_id_hash`, `resource_hash`, `severity`, `detail_hash`.
    pub content_hash: u64,
    /// FNV-1a hash of the violated rule ID.
    pub rule_id_hash: u64,
    /// FNV-1a hash of the affected resource.
    pub resource_hash: u64,
    /// Severity as u8 discriminant.
    pub severity: u8,
    /// FNV-1a hash of the violation detail text.
    pub detail_hash: u64,
    /// Estimated risk level mapped from compliance severity.
    pub estimated_risk: u8,
}

/// Convert a compliance violation into a LegalAI clause analysis record.
#[inline]
#[must_use]
pub fn compliance_violation_to_legal_ai_clause(violation: &Violation) -> ComplianceLegalAiClause {
    let rule_id_hash = fnv1a(violation.rule_id.as_bytes());
    let resource_hash = fnv1a(violation.resource.as_bytes());
    let severity_byte = match violation.severity {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    };
    let detail_hash = fnv1a(violation.detail.as_bytes());

    // コンプライアンス severity → LegalAI RiskLevel マッピング
    let estimated_risk = match violation.severity {
        Severity::Info | Severity::Low => 0, // RiskLevel::Low
        Severity::Medium => 1,               // RiskLevel::Medium
        Severity::High => 2,                 // RiskLevel::High
        Severity::Critical => 3,             // RiskLevel::Critical
    };

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&rule_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&resource_hash.to_le_bytes());
    key[16] = severity_byte;
    key[17..25].copy_from_slice(&detail_hash.to_le_bytes());
    key[25..33].copy_from_slice(&(estimated_risk as u64).to_le_bytes());

    ComplianceLegalAiClause {
        content_hash: fnv1a(&key),
        rule_id_hash,
        resource_hash,
        severity: severity_byte,
        detail_hash,
        estimated_risk,
    }
}

// ── Bridge 3: Legal Contract → compliance rule check ────────────────────

/// Compliance rule check derived from a Legal Contract.
///
/// Extracts contract structure and status to determine if compliance
/// rules (party count limits, obligation caps) are satisfied.
pub struct ComplianceContractCheck {
    /// FNV-1a hash over `contract_id`, `party_count`, `obligation_count`, `status`, `content_hash_src`.
    pub content_hash: u64,
    /// Contract identifier.
    pub contract_id: u64,
    /// Number of parties in the contract.
    pub party_count: usize,
    /// Number of obligations.
    pub obligation_count: usize,
    /// Contract status as u8 discriminant.
    pub status: u8,
    /// Content hash from the original contract.
    pub content_hash_src: u64,
    /// Whether unfulfilled obligations exist (potential compliance issue).
    pub has_unfulfilled: bool,
}

/// Convert a Legal Contract into a compliance rule check record.
#[inline]
#[must_use]
pub fn compliance_legal_contract_to_rule(contract: &Contract) -> ComplianceContractCheck {
    let status_byte = match contract.status {
        ContractStatus::Draft => 0,
        ContractStatus::Active => 1,
        ContractStatus::Fulfilled => 2,
        ContractStatus::Breached => 3,
        ContractStatus::Terminated => 4,
        ContractStatus::Expired => 5,
    };

    let obligation_count = contract.obligations.len();
    let has_unfulfilled = contract.obligations.iter().any(|o| !o.fulfilled);

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&contract.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(contract.parties.len() as u64).to_le_bytes());
    key[16..24].copy_from_slice(&(obligation_count as u64).to_le_bytes());
    key[24] = status_byte;
    key[25..33].copy_from_slice(&contract.content_hash.to_le_bytes());

    ComplianceContractCheck {
        content_hash: fnv1a(&key),
        contract_id: contract.id.0,
        party_count: contract.parties.len(),
        obligation_count,
        status: status_byte,
        content_hash_src: contract.content_hash,
        has_unfulfilled,
    }
}

// ── Bridge 4: LegalAI risk → compliance analytics ───────────────────────

/// Compliance analytics record derived from LegalAI clause risk analysis.
///
/// Aggregates clause-level risk data from LegalAI into a compliance
/// analytics summary for reporting and dashboards.
pub struct ComplianceLegalAiAnalytics {
    /// FNV-1a hash over `clause_id_hash`, `clause_type`, `risk_level`, `section_hash`, `risk_score`.
    pub content_hash: u64,
    /// FNV-1a hash of the clause ID.
    pub clause_id_hash: u64,
    /// Clause type as u8 discriminant.
    pub clause_type: u8,
    /// Risk level as u8 discriminant.
    pub risk_level: u8,
    /// FNV-1a hash of the section identifier.
    pub section_hash: u64,
    /// Numeric risk score (1=Low, 3=Medium, 7=High, 10=Critical).
    pub risk_score: u32,
}

/// Convert a LegalAI Clause into a compliance analytics record.
#[inline]
#[must_use]
pub fn compliance_legal_ai_risk_to_analytics(clause: &Clause) -> ComplianceLegalAiAnalytics {
    let clause_id_hash = fnv1a(clause.id.as_bytes());
    let clause_type_byte = match clause.clause_type {
        ClauseType::Indemnification => 0,
        ClauseType::Limitation => 1,
        ClauseType::Termination => 2,
        ClauseType::Confidentiality => 3,
        ClauseType::Ip => 4,
        ClauseType::Warranty => 5,
        ClauseType::Governing => 6,
        ClauseType::Other => 7,
    };
    let risk_level_byte = match clause.risk_level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    };
    let section_hash = fnv1a(clause.section.as_bytes());
    let risk_score: u32 = match clause.risk_level {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 3,
        RiskLevel::High => 7,
        RiskLevel::Critical => 10,
    };

    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&clause_id_hash.to_le_bytes());
    key[8] = clause_type_byte;
    key[9] = risk_level_byte;
    key[10..18].copy_from_slice(&section_hash.to_le_bytes());
    key[18..22].copy_from_slice(&risk_score.to_le_bytes());
    // パディング
    key[22..30].copy_from_slice(&0u64.to_le_bytes());
    key[30..34].copy_from_slice(&0u32.to_le_bytes());

    ComplianceLegalAiAnalytics {
        content_hash: fnv1a(&key),
        clause_id_hash,
        clause_type: clause_type_byte,
        risk_level: risk_level_byte,
        section_hash,
        risk_score,
    }
}

// ── Bridge 5: audit trail → cache ───────────────────────────────────────

/// Cache record for compliance audit trail entries.
///
/// Provides a cacheable summary of compliance audit data with branchless
/// TTL: critical/high severity entries get short TTL (frequent refresh),
/// low/info entries get long TTL.
pub struct ComplianceAuditCache {
    /// FNV-1a hash over `rule_id_hash`, `regulation`, `severity`, `description_hash`, `ttl_secs`.
    pub content_hash: u64,
    /// FNV-1a hash of the rule ID.
    pub rule_id_hash: u64,
    /// Regulation type as u8 discriminant.
    pub regulation: u8,
    /// Severity as u8 discriminant.
    pub severity: u8,
    /// FNV-1a hash of the description.
    pub description_hash: u64,
    /// Branchless TTL: critical/high=120s, それ以外=1800s.
    pub ttl_secs: u32,
}

/// Convert a compliance rule into a cache record.
#[inline]
#[must_use]
pub fn compliance_audit_to_cache(rule: &ComplianceRule) -> ComplianceAuditCache {
    let rule_id_hash = fnv1a(rule.id.as_bytes());
    let regulation_byte = match rule.regulation {
        Regulation::Gdpr => 0,
        Regulation::Sox => 1,
        Regulation::Hipaa => 2,
        Regulation::Pci => 3,
        Regulation::Iso27001 => 4,
    };
    let severity_byte = match rule.severity {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    };
    let description_hash = fnv1a(rule.description.as_bytes());

    // Branchless TTL: high(3)/critical(4)→120s, それ以外→1800s
    let is_urgent = (severity_byte >= 3) as u32;
    let ttl_secs = 1800 - is_urgent * 1680;

    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&rule_id_hash.to_le_bytes());
    key[8] = regulation_byte;
    key[9] = severity_byte;
    key[10..18].copy_from_slice(&description_hash.to_le_bytes());
    key[18..22].copy_from_slice(&ttl_secs.to_le_bytes());
    key[22..34].copy_from_slice(&0u128.to_le_bytes()[..12]);

    ComplianceAuditCache {
        content_hash: fnv1a(&key),
        rule_id_hash,
        regulation: regulation_byte,
        severity: severity_byte,
        description_hash,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_compliance::{ComplianceRule, Regulation, Severity, Violation};
    use alice_legal::Contract;
    use alice_legal_ai::{Clause, ClauseType, RiskLevel};

    fn sample_rule(severity: Severity) -> ComplianceRule {
        ComplianceRule {
            id: String::from("GDPR-ENC-001"),
            regulation: Regulation::Gdpr,
            description: String::from("personal data must be encrypted"),
            severity,
        }
    }

    // ── Bridge 1: compliance rule → legal audit ─────────────────────────

    #[test]
    fn test_compliance_rule_to_legal_audit() {
        let rule = sample_rule(Severity::High);
        let audit = compliance_rule_to_legal_audit(&rule);
        assert_ne!(audit.content_hash, 0);
        assert_ne!(audit.rule_id_hash, 0);
        assert_eq!(audit.regulation, 0); // Gdpr
        assert_eq!(audit.severity, 3); // High
        assert_ne!(audit.description_hash, 0);
    }

    #[test]
    fn test_compliance_rule_to_legal_audit_deterministic() {
        let rule = sample_rule(Severity::Critical);
        let a1 = compliance_rule_to_legal_audit(&rule);
        let a2 = compliance_rule_to_legal_audit(&rule);
        assert_eq!(a1.content_hash, a2.content_hash);
    }

    // ── Bridge 2: violation → LegalAI clause ────────────────────────────

    #[test]
    fn test_compliance_violation_to_legal_ai_clause() {
        let violation = Violation {
            rule_id: String::from("GDPR-ENC-001"),
            resource: String::from("email"),
            detail: String::from("personal data must be encrypted"),
            severity: Severity::Critical,
        };
        let clause = compliance_violation_to_legal_ai_clause(&violation);
        assert_ne!(clause.content_hash, 0);
        assert_eq!(clause.severity, 4); // Critical
        assert_eq!(clause.estimated_risk, 3); // Critical → 3
    }

    #[test]
    fn test_compliance_violation_low_severity() {
        let violation = Violation {
            rule_id: String::from("INFO-001"),
            resource: String::from("log"),
            detail: String::from("informational"),
            severity: Severity::Info,
        };
        let clause = compliance_violation_to_legal_ai_clause(&violation);
        assert_eq!(clause.severity, 0); // Info
        assert_eq!(clause.estimated_risk, 0); // Low
    }

    // ── Bridge 3: legal contract → compliance rule check ────────────────

    #[test]
    fn test_compliance_legal_contract_to_rule() {
        let mut contract = Contract::new(1, &[10, 20], 1_000_000);
        contract.add_obligation(10, 20, 5000_0000, 2_000_000);
        contract.check_status(500_000);

        let check = compliance_legal_contract_to_rule(&contract);
        assert_ne!(check.content_hash, 0);
        assert_eq!(check.contract_id, 1);
        assert_eq!(check.party_count, 2);
        assert_eq!(check.obligation_count, 1);
        assert_eq!(check.status, 1); // Active
        assert!(check.has_unfulfilled);
    }

    #[test]
    fn test_compliance_legal_contract_draft() {
        let contract = Contract::new(42, &[1, 2, 3], 0);
        let check = compliance_legal_contract_to_rule(&contract);
        assert_eq!(check.status, 0); // Draft
        assert_eq!(check.party_count, 3);
        assert!(!check.has_unfulfilled); // 義務なし
    }

    // ── Bridge 4: LegalAI risk → analytics ──────────────────────────────

    #[test]
    fn test_compliance_legal_ai_risk_to_analytics() {
        let clause = Clause {
            id: String::from("CL-001"),
            section: String::from("3.1"),
            text: String::from("indemnification clause"),
            clause_type: ClauseType::Indemnification,
            risk_level: RiskLevel::Critical,
        };
        let analytics = compliance_legal_ai_risk_to_analytics(&clause);
        assert_ne!(analytics.content_hash, 0);
        assert_eq!(analytics.clause_type, 0); // Indemnification
        assert_eq!(analytics.risk_level, 3); // Critical
        assert_eq!(analytics.risk_score, 10); // Critical → 10
    }

    #[test]
    fn test_compliance_legal_ai_risk_low() {
        let clause = Clause {
            id: String::from("CL-002"),
            section: String::from("1.0"),
            text: String::from("governing law"),
            clause_type: ClauseType::Governing,
            risk_level: RiskLevel::Low,
        };
        let analytics = compliance_legal_ai_risk_to_analytics(&clause);
        assert_eq!(analytics.clause_type, 6); // Governing
        assert_eq!(analytics.risk_level, 0); // Low
        assert_eq!(analytics.risk_score, 1); // Low → 1
    }

    // ── Bridge 5: audit → cache ─────────────────────────────────────────

    #[test]
    fn test_compliance_audit_to_cache_critical() {
        let rule = sample_rule(Severity::Critical);
        let cache = compliance_audit_to_cache(&rule);
        assert_ne!(cache.content_hash, 0);
        assert_eq!(cache.severity, 4); // Critical
        assert_eq!(cache.ttl_secs, 120); // 短いTTL
    }

    #[test]
    fn test_compliance_audit_to_cache_low() {
        let rule = sample_rule(Severity::Low);
        let cache = compliance_audit_to_cache(&rule);
        assert_eq!(cache.severity, 1); // Low
        assert_eq!(cache.ttl_secs, 1800); // 長いTTL
    }
}
