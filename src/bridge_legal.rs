//! Legal bridges — ALICE-Legal ↔ Analytics, DB, Search
//!
//! 5 bridges connecting legal domain data to the ALICE ecosystem.

use alice_legal::{AuditEntry, AuditEventKind, ClauseKind, Contract, ContractStatus, StatuteTree};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: StatuteTree → Analytics (statute complexity metrics) ──────

/// Statute complexity metrics for ALICE-Analytics ingestion.
pub struct LegalAnalyticsStatuteEvent {
    /// Content hash over statute title hash and clause count bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the statute title.
    pub title_hash: u64,
    /// Total number of clauses in the statute tree.
    pub clause_count: usize,
    /// Number of obligation clauses.
    pub obligation_count: usize,
    /// Number of prohibition clauses.
    pub prohibition_count: usize,
    /// Statute version counter.
    pub version: u32,
}

/// Convert a statute tree into an analytics complexity event.
#[inline]
#[must_use]
pub fn legal_statute_to_analytics(statute: &StatuteTree) -> LegalAnalyticsStatuteEvent {
    let clause_count = statute.clauses.len();
    let obligation_count = statute.obligations().len();
    let prohibition_count = statute
        .clauses
        .iter()
        .filter(|c| matches!(c.kind, ClauseKind::Prohibition))
        .count();

    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&statute.title_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(clause_count as u64).to_le_bytes());
    key[16..20].copy_from_slice(&statute.version.to_le_bytes());

    LegalAnalyticsStatuteEvent {
        content_hash: fnv1a(&key),
        title_hash: statute.title_hash,
        clause_count,
        obligation_count,
        prohibition_count,
        version: statute.version,
    }
}

// ── Bridge 2: Contract → Analytics (contract status metrics) ────────────

/// Contract status metrics for ALICE-Analytics ingestion.
pub struct LegalAnalyticsContractEvent {
    /// Content hash over contract ID and status bytes.
    pub content_hash: u64,
    /// Inner u64 of the contract ID.
    pub contract_id: u64,
    /// Number of parties in the contract.
    pub party_count: usize,
    /// Number of obligations.
    pub obligation_count: usize,
    /// Number of conditions.
    pub condition_count: usize,
    /// Contract status: 0=Draft, 1=Active, 2=Fulfilled, 3=Breached, 4=Terminated, 5=Expired.
    pub status: u8,
}

/// Convert a contract into an analytics status event.
///
/// The caller must call `contract.check_status(now_ns)` before invoking this
/// bridge if an up-to-date status is needed. This bridge reads `contract.status`
/// directly without mutating the contract.
#[inline]
#[must_use]
pub fn legal_contract_to_analytics(contract: &Contract) -> LegalAnalyticsContractEvent {
    let status_byte = match contract.status {
        ContractStatus::Draft => 0,
        ContractStatus::Active => 1,
        ContractStatus::Fulfilled => 2,
        ContractStatus::Breached => 3,
        ContractStatus::Terminated => 4,
        ContractStatus::Expired => 5,
    };

    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&contract.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(contract.obligations.len() as u64).to_le_bytes());
    key[16] = status_byte;

    LegalAnalyticsContractEvent {
        content_hash: fnv1a(&key),
        contract_id: contract.id.0,
        party_count: contract.parties.len(),
        obligation_count: contract.obligations.len(),
        condition_count: contract.conditions.len(),
        status: status_byte,
    }
}

// ── Bridge 3: Contract → DB (contract state record) ─────────────────────

/// Contract state record for ALICE-DB persistence.
pub struct LegalDbContractRecord {
    /// Content hash over contract ID, party count, and timestamp bytes.
    pub content_hash: u64,
    /// Inner u64 of the contract ID.
    pub contract_id: u64,
    /// Number of parties.
    pub party_count: usize,
    /// Number of fulfilled obligations.
    pub fulfilled_count: usize,
    /// Total number of obligations.
    pub total_obligations: usize,
    /// Contract creation timestamp.
    pub created_ns: u64,
    /// Total obligation amount in fixed-point ticks.
    pub total_obligation_ticks: i128,
}

/// Convert a contract into a DB state record.
#[inline]
#[must_use]
pub fn legal_contract_to_db(contract: &Contract) -> LegalDbContractRecord {
    let fulfilled_count = contract.obligations.iter().filter(|o| o.fulfilled).count();

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&contract.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(contract.parties.len() as u64).to_le_bytes());
    key[16..24].copy_from_slice(&contract.created_ns.to_le_bytes());

    LegalDbContractRecord {
        content_hash: fnv1a(&key),
        contract_id: contract.id.0,
        party_count: contract.parties.len(),
        fulfilled_count,
        total_obligations: contract.obligations.len(),
        created_ns: contract.created_ns,
        total_obligation_ticks: contract.total_obligation(),
    }
}

// ── Bridge 4: AuditEntry → Analytics (audit trail metrics) ──────────────

/// Audit trail event for ALICE-Analytics ingestion.
pub struct LegalAnalyticsAuditEvent {
    /// Content hash over sequence and timestamp bytes.
    pub content_hash: u64,
    /// Sequence number of the audit entry.
    pub sequence: u64,
    /// Audit event kind: 0-7 mapping.
    pub event_kind: u8,
    /// Entity ID that was affected.
    pub entity_id: u64,
    /// Timestamp of the audit event.
    pub timestamp_ns: u64,
}

/// Convert an audit entry into an analytics event.
#[inline]
#[must_use]
pub fn legal_audit_to_analytics(entry: &AuditEntry) -> LegalAnalyticsAuditEvent {
    let kind_byte = match entry.kind {
        AuditEventKind::StatuteCreated => 0,
        AuditEventKind::StatuteAmended => 1,
        AuditEventKind::ContractCreated => 2,
        AuditEventKind::ContractFulfilled => 3,
        AuditEventKind::ContractBreached => 4,
        AuditEventKind::ProcedureStarted => 5,
        AuditEventKind::ProcedureCompleted => 6,
        AuditEventKind::ProcedureRejected => 7,
    };

    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&entry.sequence.to_le_bytes());
    key[8..16].copy_from_slice(&entry.timestamp_ns.to_le_bytes());
    key[16] = kind_byte;

    LegalAnalyticsAuditEvent {
        content_hash: fnv1a(&key),
        sequence: entry.sequence,
        event_kind: kind_byte,
        entity_id: entry.entity_id,
        timestamp_ns: entry.timestamp_ns,
    }
}

// ── Bridge 5: AuditEntry → DB (audit persistence) ──────────────────────

/// Audit persistence record for ALICE-DB.
pub struct LegalDbAuditRecord {
    /// Content hash over sequence, entity ID, and timestamp bytes.
    pub content_hash: u64,
    /// Sequence number.
    pub sequence: u64,
    /// Entity ID.
    pub entity_id: u64,
    /// Actor hash (who performed the action).
    pub actor_hash: u64,
    /// Event kind byte.
    pub event_kind: u8,
    /// Timestamp of the event.
    pub timestamp_ns: u64,
    /// Content hash from the audit entry itself.
    pub entry_content_hash: u64,
}

/// Convert an audit entry into a DB persistence record.
#[inline]
#[must_use]
pub fn legal_audit_to_db(entry: &AuditEntry) -> LegalDbAuditRecord {
    let kind_byte = match entry.kind {
        AuditEventKind::StatuteCreated => 0,
        AuditEventKind::StatuteAmended => 1,
        AuditEventKind::ContractCreated => 2,
        AuditEventKind::ContractFulfilled => 3,
        AuditEventKind::ContractBreached => 4,
        AuditEventKind::ProcedureStarted => 5,
        AuditEventKind::ProcedureCompleted => 6,
        AuditEventKind::ProcedureRejected => 7,
    };

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&entry.sequence.to_le_bytes());
    key[8..16].copy_from_slice(&entry.entity_id.to_le_bytes());
    key[16..24].copy_from_slice(&entry.timestamp_ns.to_le_bytes());

    LegalDbAuditRecord {
        content_hash: fnv1a(&key),
        sequence: entry.sequence,
        entity_id: entry.entity_id,
        actor_hash: entry.actor_hash,
        event_kind: kind_byte,
        timestamp_ns: entry.timestamp_ns,
        entry_content_hash: entry.content_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_legal::{AuditEventKind, AuditLog, ClauseKind, Contract, StatuteTree};

    #[test]
    fn test_statute_to_analytics() {
        let mut statute = StatuteTree::new(1, "Test Act");
        statute.add_clause(ClauseKind::Obligation, "Must comply", None, 0);
        statute.add_clause(ClauseKind::Prohibition, "Must not violate", None, 0);
        let ev = legal_statute_to_analytics(&statute);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.clause_count, 2);
        assert_eq!(ev.obligation_count, 1);
        assert_eq!(ev.prohibition_count, 1);
        assert_eq!(ev.version, 1);
    }

    #[test]
    fn test_contract_to_analytics() {
        let mut contract = Contract::new(1, &[10, 20], 1_000_000);
        contract.add_obligation(10, 20, 5000_0000, 2_000_000);
        contract.check_status(500_000);
        let ev = legal_contract_to_analytics(&contract);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.contract_id, 1);
        assert_eq!(ev.party_count, 2);
        assert_eq!(ev.obligation_count, 1);
        assert_eq!(ev.status, 1); // Active (promoted from Draft by check_status)
    }

    #[test]
    fn test_contract_to_db() {
        let mut contract = Contract::new(42, &[10, 20], 500);
        contract.add_obligation(10, 20, 1000, 999_999);
        let rec = legal_contract_to_db(&contract);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.contract_id, 42);
        assert_eq!(rec.party_count, 2);
        assert_eq!(rec.fulfilled_count, 0);
        assert_eq!(rec.total_obligations, 1);
        assert_eq!(rec.created_ns, 500);
        assert_eq!(rec.total_obligation_ticks, 1000);
    }

    #[test]
    fn test_audit_to_analytics() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::StatuteCreated,
            1,
            "admin",
            "Civil Code",
            1000,
        );
        let entry = &log.entries[0];
        let ev = legal_audit_to_analytics(entry);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.sequence, 0);
        assert_eq!(ev.event_kind, 0); // StatuteCreated
        assert_eq!(ev.entity_id, 1);
    }

    #[test]
    fn test_audit_to_db() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::ContractCreated,
            100,
            "alice",
            "new contract",
            5000,
        );
        let entry = &log.entries[0];
        let rec = legal_audit_to_db(entry);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.entity_id, 100);
        assert_eq!(rec.event_kind, 2); // ContractCreated
        assert_eq!(rec.timestamp_ns, 5000);
    }

    #[test]
    fn test_hash_determinism() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::StatuteAmended,
            1,
            "admin",
            "amendment",
            3000,
        );
        let entry = &log.entries[0];
        let r1 = legal_audit_to_db(entry);
        let r2 = legal_audit_to_db(entry);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── 追加テスト ────────────────────────────────────────────────────────

    #[test]
    fn test_statute_analytics_determinism() {
        // 同一 StatuteTree で2回呼び出すと content_hash が一致すること。
        let mut statute = StatuteTree::new(2, "Privacy Act");
        statute.add_clause(ClauseKind::Obligation, "Data protection required", None, 0);
        let ev1 = legal_statute_to_analytics(&statute);
        let ev2 = legal_statute_to_analytics(&statute);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.title_hash, ev2.title_hash);
    }

    #[test]
    fn test_contract_db_determinism() {
        // 同一 Contract で2回呼び出すと content_hash が一致すること。
        let contract = Contract::new(99, &[1, 2, 3], 1_000);
        let r1 = legal_contract_to_db(&contract);
        let r2 = legal_contract_to_db(&contract);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.party_count, 3);
        assert_eq!(r1.contract_id, 99);
    }
}
