//! LegalAI bridges — ALICE-Legal-AI ↔ DB, Analytics, Legal, Cache, Search
//!
//! 5 bridges connecting the legal AI analysis layer to the ALICE ecosystem.
//! Covers analysis records in DB, legal metrics in Analytics, statute and
//! contract linkage to Legal, analysis caching, and search indexing.

use alice_legal_ai::{Clause, ClauseType, RiskLevel};

/// Risk assessment result produced by the LegalAI pipeline.
///
/// This type is defined here in the Eco-System bridge layer because
/// `alice_legal_ai` exposes individual scoring functions rather than a
/// combined assessment struct.
pub struct RiskAssessment {
    /// Aggregate risk level bucket.
    pub level: RiskLevel,
    /// Continuous risk score in [0.0, 1.0].
    pub score: f64,
    /// Human-readable risk factors that contributed to the score.
    pub factors: Vec<String>,
}

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Map a `ClauseType` to its numeric code.
///
/// Indemnification=0, Limitation=1, Termination=2, Confidentiality=3,
/// Ip=4, Warranty=5, Governing=6, Other=7.
#[inline(always)]
const fn clause_type_to_u8(ct: &ClauseType) -> u8 {
    match ct {
        ClauseType::Indemnification => 0,
        ClauseType::Limitation => 1,
        ClauseType::Termination => 2,
        ClauseType::Confidentiality => 3,
        ClauseType::Ip => 4,
        ClauseType::Warranty => 5,
        ClauseType::Governing => 6,
        ClauseType::Other => 7,
    }
}

/// Map a `RiskLevel` to its numeric code.
///
/// Low=0, Medium=1, High=2, Critical=3.
#[inline(always)]
const fn risk_level_to_u8(level: &RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    }
}

// ── Bridge 1: LegalAI → DB (analysis record persistence) ─────────────────

/// Legal analysis record for ALICE-DB persistence.
///
/// Written when clause analysis is complete so the database layer can
/// store and query clause records by document, type, or risk level.
pub struct LegalAiDbAnalysisRecord {
    /// FNV-1a hash over clause text bytes.
    pub content_hash: u64,
    /// Clause type code: 0=Indemnification … 7=Other.
    pub clause_type: u8,
    /// Length of the clause text in bytes.
    pub text_byte_len: usize,
}

/// Convert a clause into an analysis record for ALICE-DB.
#[inline]
#[must_use]
pub fn legal_ai_clause_to_db_record(clause: &Clause) -> LegalAiDbAnalysisRecord {
    LegalAiDbAnalysisRecord {
        content_hash: fnv1a(clause.text.as_bytes()),
        clause_type: clause_type_to_u8(&clause.clause_type),
        text_byte_len: clause.text.len(),
    }
}

// ── Bridge 2: LegalAI → Analytics (legal metrics event) ──────────────────

/// Legal metrics event for ALICE-Analytics.
///
/// Emitted after risk assessment so the analytics layer can compute
/// aggregate risk distributions, clause-type frequencies, and score trends.
pub struct LegalAiAnalyticsRiskEvent {
    /// FNV-1a hash over risk score bits and factor count bytes.
    pub content_hash: u64,
    /// Risk level code: 0=Low, 1=Medium, 2=High, 3=Critical.
    pub risk_level: u8,
    /// Risk score scaled to permille (0–1000).
    pub risk_score_permille: u32,
    /// Number of risk factors identified.
    pub factor_count: u32,
}

/// Convert a risk assessment into a metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn legal_ai_risk_to_analytics_event(
    assessment: &RiskAssessment,
) -> LegalAiAnalyticsRiskEvent {
    let score_bits = assessment.score.to_bits();
    let factor_count = assessment.factors.len() as u32;
    let mut key = [0u8; 12];
    key[0..8].copy_from_slice(&score_bits.to_le_bytes());
    key[8..12].copy_from_slice(&factor_count.to_le_bytes());
    // Scale f64 score [0.0, 1.0] to permille branchlessly.
    let risk_score_permille = (assessment.score * 1000.0) as u32;
    LegalAiAnalyticsRiskEvent {
        content_hash: fnv1a(&key),
        risk_level: risk_level_to_u8(&assessment.level),
        risk_score_permille,
        factor_count,
    }
}

// ── Bridge 3: LegalAI → Legal (statute/contract link) ────────────────────

/// Legal document link record for ALICE-Legal.
///
/// Connects an AI-identified clause to the corresponding statute or contract
/// section in the ALICE-Legal corpus for cross-reference lookup.
pub struct LegalAiLegalLink {
    /// FNV-1a hash over clause text bytes — link identity key.
    pub content_hash: u64,
    /// FNV-1a hash of the clause text for Legal corpus lookup.
    pub clause_hash: u64,
    /// Clause type code for Legal corpus routing.
    pub clause_type: u8,
    /// Risk level code for priority sorting in Legal review queues.
    pub risk_level: u8,
    /// Text length in bytes — used by Legal for snippet rendering.
    pub text_byte_len: usize,
}

/// Build a Legal corpus link from a clause and its risk assessment.
#[inline]
#[must_use]
pub fn legal_ai_clause_to_legal_link(
    clause: &Clause,
    assessment: &RiskAssessment,
) -> LegalAiLegalLink {
    let clause_hash = fnv1a(clause.text.as_bytes());
    let mut key = [0u8; 9];
    key[0..8].copy_from_slice(&clause_hash.to_le_bytes());
    key[8] = clause_type_to_u8(&clause.clause_type);
    LegalAiLegalLink {
        content_hash: fnv1a(&key),
        clause_hash,
        clause_type: clause_type_to_u8(&clause.clause_type),
        risk_level: risk_level_to_u8(&assessment.level),
        text_byte_len: clause.text.len(),
    }
}

// ── Bridge 4: LegalAI → Cache (analysis result cache) ────────────────────

/// Analysis result cache entry for ALICE-Cache.
///
/// Caches the risk assessment result for a clause text hash so repeated
/// analysis of the same clause text skips the NLP pipeline.
/// Critical-risk results receive a shorter TTL to force re-evaluation.
pub struct LegalAiCacheEntry {
    /// FNV-1a hash over clause text bytes — cache key.
    pub content_hash: u64,
    /// Risk level code of the cached assessment.
    pub risk_level: u8,
    /// Risk score scaled to permille.
    pub risk_score_permille: u32,
    /// Number of risk factors in the cached result.
    pub factor_count: u32,
    /// Cache TTL in seconds: 300 for Low/Medium, 60 for High/Critical.
    pub ttl_secs: u32,
}

/// Build an analysis result cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: High/Critical (level >= 2) → 60 s;
/// Low/Medium (level < 2) → 300 s.
#[inline]
#[must_use]
pub fn legal_ai_to_cache_entry(
    clause: &Clause,
    assessment: &RiskAssessment,
) -> LegalAiCacheEntry {
    let content_hash = fnv1a(clause.text.as_bytes());
    let level_code = risk_level_to_u8(&assessment.level);
    // Branchless TTL: elevated=1 → 300-240=60, normal=0 → 300.
    let elevated = (level_code >= 2) as u32;
    let ttl_secs = 300 - elevated * 240;
    LegalAiCacheEntry {
        content_hash,
        risk_level: level_code,
        risk_score_permille: (assessment.score * 1000.0) as u32,
        factor_count: assessment.factors.len() as u32,
        ttl_secs,
    }
}

// ── Bridge 5: LegalAI → Search (legal search index record) ───────────────

/// Search index record for ALICE-Search.
///
/// Enables full-text search over legal clauses with risk-level faceting
/// and clause-type filtering in the ALICE-Search index.
pub struct LegalAiSearchRecord {
    /// FNV-1a hash over clause text bytes — search document ID.
    pub content_hash: u64,
    /// Clause text byte length for search snippet sizing.
    pub text_byte_len: usize,
    /// Clause type code for faceted filtering.
    pub clause_type: u8,
    /// Risk level code for relevance boosting.
    pub risk_level: u8,
    /// Risk score permille for numeric range queries.
    pub risk_score_permille: u32,
}

/// Build a search index record from a clause and risk assessment for ALICE-Search.
#[inline]
#[must_use]
pub fn legal_ai_to_search_record(
    clause: &Clause,
    assessment: &RiskAssessment,
) -> LegalAiSearchRecord {
    LegalAiSearchRecord {
        content_hash: fnv1a(clause.text.as_bytes()),
        text_byte_len: clause.text.len(),
        clause_type: clause_type_to_u8(&clause.clause_type),
        risk_level: risk_level_to_u8(&assessment.level),
        risk_score_permille: (assessment.score * 1000.0) as u32,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_legal_ai::{Clause, ClauseType, RiskLevel};

    fn make_clause(text: &str, clause_type: ClauseType) -> Clause {
        Clause {
            id: String::new(),
            section: String::new(),
            text: text.to_string(),
            clause_type,
            risk_level: RiskLevel::Low,
        }
    }

    fn make_assessment(level: RiskLevel, score: f64, factors: Vec<String>) -> RiskAssessment {
        RiskAssessment { level, factors, score }
    }

    #[test]
    fn test_clause_to_db_record_obligation() {
        let clause = make_clause("The licensee shall pay within 30 days.", ClauseType::Indemnification);
        let rec = legal_ai_clause_to_db_record(&clause);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.clause_type, 0); // Indemnification → 0
        assert_eq!(rec.text_byte_len, clause.text.len());
    }

    #[test]
    fn test_clause_to_db_record_confidentiality() {
        let clause = make_clause("All information is strictly confidential.", ClauseType::Confidentiality);
        let rec = legal_ai_clause_to_db_record(&clause);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.clause_type, 3); // Confidentiality → 3
    }

    #[test]
    fn test_risk_to_analytics_event_high() {
        let assessment = make_assessment(
            RiskLevel::High,
            0.75,
            vec!["unlimited liability".to_string(), "no indemnity cap".to_string()],
        );
        let ev = legal_ai_risk_to_analytics_event(&assessment);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.risk_level, 2); // High → 2
        assert_eq!(ev.risk_score_permille, 750);
        assert_eq!(ev.factor_count, 2);
    }

    #[test]
    fn test_risk_to_analytics_event_critical() {
        let assessment = make_assessment(RiskLevel::Critical, 0.95, vec!["waiver of rights".to_string()]);
        let ev = legal_ai_risk_to_analytics_event(&assessment);
        assert_eq!(ev.risk_level, 3); // Critical → 3
        assert_eq!(ev.risk_score_permille, 950);
    }

    #[test]
    fn test_clause_to_legal_link() {
        let clause = make_clause("Termination upon 30-day notice.", ClauseType::Termination);
        let assessment = make_assessment(RiskLevel::Medium, 0.4, vec![]);
        let link = legal_ai_clause_to_legal_link(&clause, &assessment);
        assert_ne!(link.content_hash, 0);
        assert_ne!(link.clause_hash, 0);
        assert_eq!(link.clause_type, 2); // Termination → 2
        assert_eq!(link.risk_level, 1); // Medium → 1
        assert_eq!(link.text_byte_len, clause.text.len());
    }

    #[test]
    fn test_cache_entry_low_risk_ttl() {
        // Low risk → ttl = 300
        let clause = make_clause("Permission to use name in marketing.", ClauseType::Ip);
        let assessment = make_assessment(RiskLevel::Low, 0.1, vec![]);
        let entry = legal_ai_to_cache_entry(&clause, &assessment);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_cache_entry_critical_risk_ttl() {
        // Critical risk → ttl = 60
        let clause = make_clause("Waiver of all liability.", ClauseType::Indemnification);
        let assessment = make_assessment(RiskLevel::Critical, 0.98, vec!["waiver".to_string()]);
        let entry = legal_ai_to_cache_entry(&clause, &assessment);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_search_record_fields() {
        let clause = make_clause("Warranty of merchantability implied.", ClauseType::Warranty);
        let assessment = make_assessment(RiskLevel::Medium, 0.55, vec!["implied warranty".to_string()]);
        let rec = legal_ai_to_search_record(&clause, &assessment);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.clause_type, 5); // Warranty → 5
        assert_eq!(rec.risk_level, 1);  // Medium → 1
        assert_eq!(rec.risk_score_permille, 550);
        assert_eq!(rec.text_byte_len, clause.text.len());
    }

    #[test]
    fn test_hash_determinism() {
        let clause = make_clause("The parties agree to arbitration.", ClauseType::Indemnification);
        let rec1 = legal_ai_clause_to_db_record(&clause);
        let rec2 = legal_ai_clause_to_db_record(&clause);
        assert_eq!(rec1.content_hash, rec2.content_hash);
    }
}
