//! Cross-domain bridges — ALICE-Legal ↔ Text, Search, Auth, Crypto
//!
//! 4 bridges connecting legal domain data to text compression records,
//! search index entries, auth-verified audit records, and crypto-sealed
//! contract metadata.

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

// ── Bridge 1: StatuteTree → Text (compression record) ──────────────────

/// Text compression record derived from a statute tree.
///
/// Encodes statute identity, clause counts, and estimated text size so
/// the Text layer can pre-allocate compression buffers and prioritise
/// statutes by content volume without parsing the legal tree itself.
pub struct LegalTextRecord {
    /// FNV-1a hash over `statute_id`, `clause_count`, `obligation_count`, `estimated_text_bytes`, `title_hash`.
    pub content_hash: u64,
    /// The statute's numeric identifier.
    pub statute_id: u64,
    /// Total number of clauses in the statute.
    pub clause_count: usize,
    /// Number of obligation clauses.
    pub obligation_count: usize,
    /// Estimated text size in bytes (`clause_count` * 64, approximate average clause length).
    pub estimated_text_bytes: usize,
    /// FNV-1a hash of the statute title.
    pub title_hash: u64,
}

/// Convert a statute tree into a text compression record.
#[inline]
#[must_use]
pub fn legal_statute_to_text_record(statute: &StatuteTree) -> LegalTextRecord {
    let clause_count = statute.clauses.len();
    let obligation_count = statute.obligations().len();
    let estimated_text_bytes = clause_count * 64;

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&statute.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(clause_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&(obligation_count as u64).to_le_bytes());
    key[24..32].copy_from_slice(&(estimated_text_bytes as u64).to_le_bytes());
    key[32..40].copy_from_slice(&statute.title_hash.to_le_bytes());

    LegalTextRecord {
        content_hash: fnv1a(&key),
        statute_id: statute.id.0,
        clause_count,
        obligation_count,
        estimated_text_bytes,
        title_hash: statute.title_hash,
    }
}

// ── Bridge 2: StatuteTree → Search (index record) ──────────────────────

/// Search index record derived from a statute tree.
///
/// Encodes statute identity, clause counts, and structural flags so the
/// Search layer can filter and rank statutes by complexity and content
/// type without loading the full clause tree.
pub struct LegalSearchIndex {
    /// FNV-1a hash over `statute_id`, `clause_count`, `title_hash`, `obligation_count`, `has_exceptions`.
    pub content_hash: u64,
    /// The statute's numeric identifier.
    pub statute_id: u64,
    /// Total number of clauses.
    pub clause_count: usize,
    /// FNV-1a hash of the statute title.
    pub title_hash: u64,
    /// Number of obligation clauses.
    pub obligation_count: usize,
    /// Whether the statute contains at least one Exception clause.
    pub has_exceptions: bool,
}

/// Convert a statute tree into a search index record.
#[inline]
#[must_use]
pub fn legal_statute_to_search_index(statute: &StatuteTree) -> LegalSearchIndex {
    let clause_count = statute.clauses.len();
    let obligation_count = statute.obligations().len();
    let has_exceptions = statute
        .clauses
        .iter()
        .any(|c| matches!(c.kind, ClauseKind::Exception));

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&statute.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(clause_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&statute.title_hash.to_le_bytes());
    key[24..32].copy_from_slice(&(obligation_count as u64).to_le_bytes());
    key[32] = has_exceptions as u8;

    LegalSearchIndex {
        content_hash: fnv1a(&key),
        statute_id: statute.id.0,
        clause_count,
        title_hash: statute.title_hash,
        obligation_count,
        has_exceptions,
    }
}

// ── Bridge 3: AuditEntry → Auth (verified record) ──────────────────────

/// Auth-verified audit record derived from a legal audit entry.
///
/// Attaches write-event classification to the audit entry so the Auth
/// layer can enforce access control policies (write events require
/// elevated privileges) without inspecting event kind semantics.
pub struct LegalAuthRecord {
    /// FNV-1a hash over `entity_id`, `actor_hash`, `event_kind`, `timestamp_ns`, `entry_hash`.
    pub content_hash: u64,
    /// The entity (statute/contract/procedure) affected.
    pub entity_id: u64,
    /// FNV-1a hash of the actor's identity.
    pub actor_hash: u64,
    /// Event kind as u8 discriminant.
    pub event_kind: u8,
    /// Event timestamp in Unix nanoseconds.
    pub timestamp_ns: u64,
    /// Content hash from the original audit entry.
    pub entry_hash: u64,
    /// True for write events: `StatuteCreated`, `StatuteAmended`, `ContractCreated`,
    /// `ProcedureStarted` (state-changing operations).
    pub is_write_event: bool,
}

/// Convert an audit entry into an auth-verified record.
#[inline]
#[must_use]
pub fn legal_audit_to_auth_record(entry: &AuditEntry) -> LegalAuthRecord {
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

    let is_write_event = matches!(
        entry.kind,
        AuditEventKind::StatuteCreated
            | AuditEventKind::StatuteAmended
            | AuditEventKind::ContractCreated
            | AuditEventKind::ProcedureStarted
    );

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&entry.entity_id.to_le_bytes());
    key[8..16].copy_from_slice(&entry.actor_hash.to_le_bytes());
    key[16] = kind_byte;
    key[17..25].copy_from_slice(&entry.timestamp_ns.to_le_bytes());
    key[25..33].copy_from_slice(&entry.content_hash.to_le_bytes());

    LegalAuthRecord {
        content_hash: fnv1a(&key),
        entity_id: entry.entity_id,
        actor_hash: entry.actor_hash,
        event_kind: kind_byte,
        timestamp_ns: entry.timestamp_ns,
        entry_hash: entry.content_hash,
        is_write_event,
    }
}

// ── Bridge 4: Contract → Crypto (sealed contract) ──────────────────────

/// Crypto-sealed contract metadata for tamper-evident storage.
///
/// Computes a seal hash from contract identity, party roster, and status
/// so the Crypto layer can verify contract integrity without accessing
/// the full obligation tree.
pub struct LegalCryptoSealed {
    /// FNV-1a hash over `contract_id`, `party_count`, status, `obligation_count`, `seal_hash`.
    pub content_hash: u64,
    /// The contract's numeric identifier.
    pub contract_id: u64,
    /// Number of parties in the contract.
    pub party_count: usize,
    /// Contract status as u8 discriminant.
    pub status: u8,
    /// Number of obligations in the contract.
    pub obligation_count: usize,
    /// FNV-1a seal: hash of `contract_id` + all party IDs + status byte.
    pub seal_hash: u64,
}

/// Convert a contract into a crypto-sealed metadata record.
#[inline]
#[must_use]
pub fn legal_contract_to_crypto_sealed(contract: &Contract) -> LegalCryptoSealed {
    let status_byte = match contract.status {
        ContractStatus::Draft => 0,
        ContractStatus::Active => 1,
        ContractStatus::Fulfilled => 2,
        ContractStatus::Breached => 3,
        ContractStatus::Terminated => 4,
        ContractStatus::Expired => 5,
    };

    let obligation_count = contract.obligations.len();

    // Build seal hash from contract_id + party IDs + status
    let seal_input_len = 8 + contract.parties.len() * 8 + 1;
    let mut seal_input = vec![0u8; seal_input_len];
    seal_input[0..8].copy_from_slice(&contract.id.0.to_le_bytes());
    for (i, party) in contract.parties.iter().enumerate() {
        seal_input[8 + i * 8..8 + (i + 1) * 8].copy_from_slice(&party.0.to_le_bytes());
    }
    seal_input[seal_input_len - 1] = status_byte;
    let seal_hash = fnv1a(&seal_input);

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&contract.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(contract.parties.len() as u64).to_le_bytes());
    key[16] = status_byte;
    key[17..25].copy_from_slice(&(obligation_count as u64).to_le_bytes());
    key[25..33].copy_from_slice(&seal_hash.to_le_bytes());

    LegalCryptoSealed {
        content_hash: fnv1a(&key),
        contract_id: contract.id.0,
        party_count: contract.parties.len(),
        status: status_byte,
        obligation_count,
        seal_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_legal::{AuditEventKind, AuditLog, ClauseKind, Contract, StatuteTree};

    // ── Bridge 1: statute → text record ─────────────────────────────────

    #[test]
    fn test_legal_statute_to_text_record() {
        let mut statute = StatuteTree::new(1, "Civil Code Article 1");
        statute.add_clause(ClauseKind::Obligation, "Must comply", None, 0);
        statute.add_clause(ClauseKind::Prohibition, "Must not violate", None, 0);
        statute.add_clause(ClauseKind::Obligation, "Report annually", None, 0);

        let rec = legal_statute_to_text_record(&statute);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.statute_id, 1);
        assert_eq!(rec.clause_count, 3);
        assert_eq!(rec.obligation_count, 2);
        assert_eq!(rec.estimated_text_bytes, 3 * 64);
        assert_ne!(rec.title_hash, 0);
    }

    #[test]
    fn test_legal_statute_to_text_record_deterministic() {
        let mut statute = StatuteTree::new(42, "Test Act");
        statute.add_clause(ClauseKind::Definition, "Term definition", None, 0);
        let r1 = legal_statute_to_text_record(&statute);
        let r2 = legal_statute_to_text_record(&statute);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_legal_statute_to_text_record_empty() {
        let statute = StatuteTree::new(99, "Empty Act");
        let rec = legal_statute_to_text_record(&statute);
        assert_eq!(rec.clause_count, 0);
        assert_eq!(rec.obligation_count, 0);
        assert_eq!(rec.estimated_text_bytes, 0);
    }

    // ── Bridge 2: statute → search index ────────────────────────────────

    #[test]
    fn test_legal_statute_to_search_index() {
        let mut statute = StatuteTree::new(1, "Criminal Code");
        statute.add_clause(ClauseKind::Obligation, "Obey", None, 0);
        statute.add_clause(ClauseKind::Exception, "Except minors", None, 0);
        statute.add_clause(ClauseKind::Prohibition, "Do not steal", None, 0);

        let idx = legal_statute_to_search_index(&statute);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.statute_id, 1);
        assert_eq!(idx.clause_count, 3);
        assert_eq!(idx.obligation_count, 1);
        assert!(idx.has_exceptions);
    }

    #[test]
    fn test_legal_statute_to_search_index_no_exceptions() {
        let mut statute = StatuteTree::new(2, "Simple Act");
        statute.add_clause(ClauseKind::Obligation, "Must do", None, 0);
        let idx = legal_statute_to_search_index(&statute);
        assert!(!idx.has_exceptions);
    }

    #[test]
    fn test_legal_statute_to_search_index_deterministic() {
        let mut statute = StatuteTree::new(5, "Tax Code");
        statute.add_clause(ClauseKind::Obligation, "Pay taxes", None, 0);
        let i1 = legal_statute_to_search_index(&statute);
        let i2 = legal_statute_to_search_index(&statute);
        assert_eq!(i1.content_hash, i2.content_hash);
    }

    // ── Bridge 3: audit → auth record ───────────────────────────────────

    #[test]
    fn test_legal_audit_to_auth_record_write_event() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::StatuteCreated,
            1,
            "admin",
            "Civil Code",
            1000,
        );
        let entry = &log.entries[0];
        let rec = legal_audit_to_auth_record(entry);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.entity_id, 1);
        assert_ne!(rec.actor_hash, 0);
        assert_eq!(rec.event_kind, 0); // StatuteCreated
        assert_eq!(rec.timestamp_ns, 1000);
        assert!(rec.is_write_event); // Created is a write event
    }

    #[test]
    fn test_legal_audit_to_auth_record_read_event() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::ContractFulfilled,
            100,
            "alice",
            "fulfilled",
            5000,
        );
        let entry = &log.entries[0];
        let rec = legal_audit_to_auth_record(entry);
        assert_eq!(rec.event_kind, 3); // ContractFulfilled
        assert!(!rec.is_write_event); // Fulfilled is not a write event
    }

    #[test]
    fn test_legal_audit_to_auth_record_amended_is_write() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::StatuteAmended,
            1,
            "admin",
            "amendment",
            3000,
        );
        let entry = &log.entries[0];
        let rec = legal_audit_to_auth_record(entry);
        assert_eq!(rec.event_kind, 1); // StatuteAmended
        assert!(rec.is_write_event);
    }

    #[test]
    fn test_legal_audit_to_auth_record_deterministic() {
        let mut log = AuditLog::new();
        log.append(
            AuditEventKind::ContractCreated,
            10,
            "bob",
            "new contract",
            2000,
        );
        let entry = &log.entries[0];
        let r1 = legal_audit_to_auth_record(entry);
        let r2 = legal_audit_to_auth_record(entry);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 4: contract → crypto sealed ──────────────────────────────

    #[test]
    fn test_legal_contract_to_crypto_sealed() {
        let mut contract = Contract::new(1, &[10, 20], 1_000_000);
        contract.add_obligation(10, 20, 5000_0000, 2_000_000);
        contract.check_status(500_000);

        let sealed = legal_contract_to_crypto_sealed(&contract);
        assert_ne!(sealed.content_hash, 0);
        assert_eq!(sealed.contract_id, 1);
        assert_eq!(sealed.party_count, 2);
        assert_eq!(sealed.status, 1); // Active
        assert_eq!(sealed.obligation_count, 1);
        assert_ne!(sealed.seal_hash, 0);
    }

    #[test]
    fn test_legal_contract_to_crypto_sealed_draft() {
        let contract = Contract::new(42, &[1, 2, 3], 0);
        let sealed = legal_contract_to_crypto_sealed(&contract);
        assert_eq!(sealed.status, 0); // Draft
        assert_eq!(sealed.party_count, 3);
        assert_eq!(sealed.obligation_count, 0);
    }

    #[test]
    fn test_legal_contract_to_crypto_sealed_deterministic() {
        let mut contract = Contract::new(5, &[10, 20], 1000);
        contract.add_obligation(10, 20, 100, 999_999);
        let s1 = legal_contract_to_crypto_sealed(&contract);
        let s2 = legal_contract_to_crypto_sealed(&contract);
        assert_eq!(s1.content_hash, s2.content_hash);
        assert_eq!(s1.seal_hash, s2.seal_hash);
    }

    #[test]
    fn test_legal_contract_to_crypto_sealed_different_status_different_seal() {
        let mut c1 = Contract::new(1, &[10, 20], 0);
        c1.add_obligation(10, 20, 100, 1000);
        // c1 is Draft
        let sealed1 = legal_contract_to_crypto_sealed(&c1);

        let mut c2 = Contract::new(1, &[10, 20], 0);
        c2.add_obligation(10, 20, 100, 1000);
        c2.check_status(500); // Promote to Active
        let sealed2 = legal_contract_to_crypto_sealed(&c2);

        // Different status should produce different seal and content hash
        assert_ne!(sealed1.seal_hash, sealed2.seal_hash);
        assert_ne!(sealed1.content_hash, sealed2.content_hash);
    }
}
