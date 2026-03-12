//! LLM domain bridges — ALICE-LLM ↔ Document, Legal-AI, Ledger, Settlement, Cloud-Gateway
//!
//! 5 bridges connecting LLM inference to domain-specific services.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LLM → Document (document analysis / summarization) ────────

/// Document analysis request for ALICE-Document.
pub struct LlmDocumentAnalysis {
    /// Content hash over the analysis request.
    pub content_hash: u64,
    /// Document size in bytes.
    pub doc_size_bytes: u64,
    /// Document format hash (e.g. fnv1a of "pdf", "docx", "md").
    pub format_hash: u64,
    /// Number of pages (0 if unknown or non-paginated).
    pub page_count: u32,
    /// Estimated token count for the document content.
    pub estimated_tokens: u64,
    /// Analysis mode (0=summarize, 1=extract, 2=classify, 3=translate).
    pub mode: u8,
}

/// Build a document analysis request from document metadata.
///
/// Token estimate: ~4 bytes per token (average for English/Japanese mixed content).
#[inline]
#[must_use]
pub fn llm_to_document_analysis(
    doc_size_bytes: u64,
    format_hash: u64,
    page_count: u32,
    mode: u8,
) -> LlmDocumentAnalysis {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&doc_size_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&format_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&page_count.to_le_bytes());
    buf[20] = mode;
    // ~4 bytes per token on average
    let estimated_tokens = doc_size_bytes / 4;
    LlmDocumentAnalysis {
        content_hash: fnv1a(&buf),
        doc_size_bytes,
        format_hash,
        page_count,
        estimated_tokens,
        mode,
    }
}

// ── Bridge 2: LLM → Legal-AI (legal text analysis) ─────────────────────

/// Legal text analysis request for ALICE-Legal-AI.
pub struct LlmLegalAnalysis {
    /// Content hash over the legal analysis request.
    pub content_hash: u64,
    /// Document size in bytes.
    pub doc_size_bytes: u64,
    /// Jurisdiction hash (e.g. fnv1a of "JP", "US", "EU").
    pub jurisdiction_hash: u64,
    /// Analysis type (0=contract_review, 1=compliance_check, 2=risk_assessment, 3=clause_extract).
    pub analysis_type: u8,
    /// Estimated token count for the legal document.
    pub estimated_tokens: u64,
    /// Confidence threshold for legal assertions (0.0–1.0).
    pub confidence_threshold: f32,
}

/// Build a legal text analysis request from document metadata.
#[inline]
#[must_use]
pub fn llm_to_legal_analysis(
    doc_size_bytes: u64,
    jurisdiction_hash: u64,
    analysis_type: u8,
    confidence_threshold: f32,
) -> LlmLegalAnalysis {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&doc_size_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&jurisdiction_hash.to_le_bytes());
    buf[16] = analysis_type;
    buf[17..21].copy_from_slice(&confidence_threshold.to_bits().to_le_bytes());
    let estimated_tokens = doc_size_bytes / 4;
    LlmLegalAnalysis {
        content_hash: fnv1a(&buf),
        doc_size_bytes,
        jurisdiction_hash,
        analysis_type,
        estimated_tokens,
        confidence_threshold,
    }
}

// ── Bridge 3: LLM → Ledger (financial transaction analysis) ─────────────

/// Financial transaction analysis request for ALICE-Ledger.
pub struct LlmLedgerEntry {
    /// Content hash over the ledger entry.
    pub content_hash: u64,
    /// Transaction amount in minor units (e.g. cents, yen).
    pub amount_minor: i64,
    /// Currency hash (e.g. fnv1a of "JPY", "USD").
    pub currency_hash: u64,
    /// Transaction description token count.
    pub description_tokens: u32,
    /// Category hash from LLM classification.
    pub category_hash: u64,
    /// Anomaly score from LLM (0.0=normal, 1.0=highly anomalous).
    pub anomaly_score: f32,
}

/// Build a ledger entry from transaction data with LLM-assisted classification.
#[inline]
#[must_use]
pub fn llm_to_ledger_entry(
    amount_minor: i64,
    currency_hash: u64,
    description_tokens: u32,
    category_hash: u64,
    anomaly_score: f32,
) -> LlmLedgerEntry {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&amount_minor.to_le_bytes());
    buf[8..16].copy_from_slice(&currency_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&description_tokens.to_le_bytes());
    buf[20..28].copy_from_slice(&category_hash.to_le_bytes());
    buf[28..32].copy_from_slice(&anomaly_score.to_bits().to_le_bytes());
    LlmLedgerEntry {
        content_hash: fnv1a(&buf),
        amount_minor,
        currency_hash,
        description_tokens,
        category_hash,
        anomaly_score,
    }
}

// ── Bridge 4: LLM → Settlement (dispute resolution) ─────────────────────

/// Settlement dispute resolution request for ALICE-Settlement.
pub struct LlmSettlementRequest {
    /// Content hash over the settlement request.
    pub content_hash: u64,
    /// Dispute amount in minor units.
    pub dispute_amount_minor: i64,
    /// Number of evidence documents.
    pub evidence_count: u32,
    /// Total evidence token count (sum of all documents).
    pub total_evidence_tokens: u64,
    /// Resolution type (0=auto_approve, 1=manual_review, 2=escalate, 3=reject).
    pub resolution_type: u8,
    /// Confidence in the resolution (0.0–1.0).
    pub confidence: f32,
}

/// Build a settlement request from dispute data with LLM-assisted resolution.
///
/// Resolution type: auto_approve if confidence >= 0.9, manual_review if >= 0.7,
/// escalate if >= 0.5, reject otherwise.
#[inline]
#[must_use]
pub fn llm_to_settlement_request(
    dispute_amount_minor: i64,
    evidence_count: u32,
    total_evidence_tokens: u64,
    confidence: f32,
) -> LlmSettlementRequest {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&dispute_amount_minor.to_le_bytes());
    buf[8..12].copy_from_slice(&evidence_count.to_le_bytes());
    buf[12..20].copy_from_slice(&total_evidence_tokens.to_le_bytes());
    buf[20..24].copy_from_slice(&confidence.to_bits().to_le_bytes());
    // Branchless resolution type via threshold comparison
    let r0 = (confidence >= 0.9) as u8; // auto_approve
    let r1 = (confidence >= 0.7) as u8; // manual_review
    let r2 = (confidence >= 0.5) as u8; // escalate
    // 3 - r0 - r1 - r2: 3=reject, 2=escalate, 1=manual, 0=auto
    let resolution_type = 3 - r0 - r1 - r2;
    LlmSettlementRequest {
        content_hash: fnv1a(&buf),
        dispute_amount_minor,
        evidence_count,
        total_evidence_tokens,
        resolution_type,
        confidence,
    }
}

// ── Bridge 5: LLM → Cloud-Gateway (HTTP server routing) ─────────────────

/// Cloud gateway routing descriptor for ALICE-Cloud-Gateway.
pub struct LlmGatewayRoute {
    /// Content hash over the route descriptor.
    pub content_hash: u64,
    /// Model identifier hash.
    pub model_hash: u64,
    /// Server port number.
    pub port: u16,
    /// Maximum concurrent requests.
    pub max_concurrent: u32,
    /// Whether the server supports streaming responses.
    pub streaming: bool,
    /// Estimated tokens per second for capacity planning.
    pub estimated_tps: f32,
}

/// Build a gateway route from ALICE-LLM server configuration.
#[inline]
#[must_use]
pub fn llm_to_gateway_route(
    model_hash: u64,
    port: u16,
    max_concurrent: u32,
    streaming: bool,
    estimated_tps: f32,
) -> LlmGatewayRoute {
    let mut buf = [0u8; 19];
    buf[0..8].copy_from_slice(&model_hash.to_le_bytes());
    buf[8..10].copy_from_slice(&port.to_le_bytes());
    buf[10..14].copy_from_slice(&max_concurrent.to_le_bytes());
    buf[14] = streaming as u8;
    buf[15..19].copy_from_slice(&estimated_tps.to_bits().to_le_bytes());
    LlmGatewayRoute {
        content_hash: fnv1a(&buf),
        model_hash,
        port,
        max_concurrent,
        streaming,
        estimated_tps,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_analysis_summarize() {
        let a = llm_to_document_analysis(100_000, 0xaabb, 25, 0);
        assert_ne!(a.content_hash, 0);
        assert_eq!(a.estimated_tokens, 25_000); // 100000 / 4
        assert_eq!(a.mode, 0);
        assert_eq!(a.page_count, 25);
    }

    #[test]
    fn test_document_analysis_hash_determinism() {
        let a = llm_to_document_analysis(50_000, 0xccdd, 10, 1);
        let b = llm_to_document_analysis(50_000, 0xccdd, 10, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_legal_analysis_contract_review() {
        let l = llm_to_legal_analysis(200_000, 0x4a50, 0, 0.85);
        assert_ne!(l.content_hash, 0);
        assert_eq!(l.analysis_type, 0);
        assert_eq!(l.estimated_tokens, 50_000);
        assert!((l.confidence_threshold - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_ledger_entry_normal() {
        let e = llm_to_ledger_entry(150_000, 0x4a5059, 12, 0xf00d, 0.05);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.amount_minor, 150_000);
        assert!((e.anomaly_score - 0.05).abs() < 0.01);
    }

    #[test]
    fn test_ledger_entry_anomalous() {
        let e = llm_to_ledger_entry(-9_999_999, 0x555344, 3, 0x0, 0.95);
        assert!((e.anomaly_score - 0.95).abs() < 0.01);
        assert_eq!(e.amount_minor, -9_999_999);
    }

    #[test]
    fn test_settlement_auto_approve() {
        let s = llm_to_settlement_request(50_000, 3, 2048, 0.95);
        assert_ne!(s.content_hash, 0);
        assert_eq!(s.resolution_type, 0); // auto_approve
    }

    #[test]
    fn test_settlement_escalate() {
        let s = llm_to_settlement_request(500_000, 1, 512, 0.55);
        assert_eq!(s.resolution_type, 2); // escalate
    }

    #[test]
    fn test_settlement_reject() {
        let s = llm_to_settlement_request(1_000_000, 0, 0, 0.3);
        assert_eq!(s.resolution_type, 3); // reject
    }

    #[test]
    fn test_gateway_route_streaming() {
        let r = llm_to_gateway_route(0xbeef, 8090, 16, true, 12.5);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.port, 8090);
        assert!(r.streaming);
        assert!((r.estimated_tps - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_gateway_route_hash_determinism() {
        let a = llm_to_gateway_route(0xdead, 8091, 8, false, 5.0);
        let b = llm_to_gateway_route(0xdead, 8091, 8, false, 5.0);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
