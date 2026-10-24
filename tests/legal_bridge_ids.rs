//! Two different inputs to a Legal bridge never get the same record identifier.
//!
//! Each bridge record's `content_hash` covers every field the record carries, and the identifiers
//! it carries from ALICE-Legal (`title_hash`, `actor_hash`, `entry_content_hash`, …) tell apart
//! inputs that a 64-bit FNV-1a hash does not: `A` and `B` below are two different strings with
//! the same FNV-1a 64-bit hash (`0x3ff74e522de530b1`, the pair ALICE-Legal's own collision test
//! uses). Every test changes one input and requires the identifier to change.

use alice_eco_system::bridge_legal::{
    legal_audit_to_analytics, legal_audit_to_db, legal_contract_to_analytics, legal_contract_to_db,
    legal_statute_to_analytics,
};
use alice_eco_system::bridge_legal_cross::{
    legal_audit_to_auth_record, legal_contract_to_crypto_sealed, legal_statute_to_search_index,
    legal_statute_to_text_record,
};
use alice_legal::{AuditEntry, AuditEventKind, AuditLog, ClauseKind, Contract, StatuteTree};

const A: &str = "c5bde799c2362419";
const B: &str = "a1a9a9bf38687075";

fn statute(title: &str, kinds: &[ClauseKind]) -> StatuteTree {
    let mut s = StatuteTree::new(1, title);
    for &k in kinds {
        s.add_clause(k, "clause", None, 0);
    }
    s
}

fn entry(kind: AuditEventKind, entity: u64, actor: &str, content: &str, ts: u64) -> AuditEntry {
    let mut log = AuditLog::new();
    log.append(kind, entity, actor, content, ts);
    log.entries[0].clone()
}

#[test]
fn statutes_with_titles_that_collide_under_fnv1a_get_different_ids() {
    let (a, b) = (
        statute(A, &[ClauseKind::Obligation]),
        statute(B, &[ClauseKind::Obligation]),
    );
    let (ea, eb) = (
        legal_statute_to_analytics(&a),
        legal_statute_to_analytics(&b),
    );
    assert_ne!(ea.title_hash, eb.title_hash);
    assert_ne!(ea.content_hash, eb.content_hash);
    let (ta, tb) = (
        legal_statute_to_text_record(&a),
        legal_statute_to_text_record(&b),
    );
    assert_ne!(ta.title_hash, tb.title_hash);
    assert_ne!(ta.content_hash, tb.content_hash);
    let (sa, sb) = (
        legal_statute_to_search_index(&a),
        legal_statute_to_search_index(&b),
    );
    assert_ne!(sa.title_hash, sb.title_hash);
    assert_ne!(sa.content_hash, sb.content_hash);
}

#[test]
fn a_statute_analytics_id_covers_its_clause_kinds() {
    // same title, same clause count and version; the obligation / prohibition counts differ
    let a = legal_statute_to_analytics(&statute("t", &[ClauseKind::Obligation]));
    let b = legal_statute_to_analytics(&statute("t", &[ClauseKind::Prohibition]));
    assert_ne!(a.content_hash, b.content_hash);
}

#[test]
fn a_search_index_id_covers_the_exception_flag() {
    let a = legal_statute_to_search_index(&statute("t", &[ClauseKind::Obligation]));
    let b = legal_statute_to_search_index(&statute("t", &[ClauseKind::Exception]));
    assert_ne!(a.content_hash, b.content_hash);
}

#[test]
fn a_contract_analytics_id_covers_parties_and_conditions() {
    let a = legal_contract_to_analytics(&Contract::new(1, &[1, 2], 0));
    let b = legal_contract_to_analytics(&Contract::new(1, &[1, 2, 3], 0));
    assert_ne!(a.content_hash, b.content_hash, "party count");
    let mut c = Contract::new(1, &[1, 2], 0);
    c.add_condition("cond", "eval");
    assert_ne!(
        a.content_hash,
        legal_contract_to_analytics(&c).content_hash,
        "condition count"
    );
}

#[test]
fn a_contract_db_id_covers_obligations_and_their_state() {
    let base = Contract::new(1, &[1, 2], 0);
    let mut one = base.clone();
    one.add_obligation(1, 2, 100, 10);
    let mut fulfilled = one.clone();
    fulfilled.fulfill_obligation(0);
    let mut more = Contract::new(1, &[1, 2], 0);
    more.add_obligation(1, 2, 200, 10);
    let ids = [
        legal_contract_to_db(&base).content_hash,
        legal_contract_to_db(&one).content_hash,
        legal_contract_to_db(&fulfilled).content_hash,
        legal_contract_to_db(&more).content_hash,
    ];
    for i in 0..ids.len() {
        for j in i + 1..ids.len() {
            assert_ne!(ids[i], ids[j], "{i} and {j}");
        }
    }
}

#[test]
fn an_audit_analytics_id_covers_the_entity() {
    let a = legal_audit_to_analytics(&entry(AuditEventKind::StatuteCreated, 1, "x", "y", 5));
    let b = legal_audit_to_analytics(&entry(AuditEventKind::StatuteCreated, 2, "x", "y", 5));
    assert_ne!(a.content_hash, b.content_hash);
}

#[test]
fn an_audit_db_id_covers_actor_kind_and_content() {
    let base = entry(AuditEventKind::StatuteCreated, 1, A, "y", 5);
    let ids = [
        legal_audit_to_db(&base),
        legal_audit_to_db(&entry(AuditEventKind::StatuteCreated, 1, B, "y", 5)),
        legal_audit_to_db(&entry(AuditEventKind::StatuteAmended, 1, A, "y", 5)),
        legal_audit_to_db(&entry(AuditEventKind::StatuteCreated, 1, A, "z", 5)),
    ];
    assert_ne!(ids[0].actor_hash, ids[1].actor_hash);
    for i in 0..ids.len() {
        for j in i + 1..ids.len() {
            assert_ne!(ids[i].content_hash, ids[j].content_hash, "{i} and {j}");
        }
    }
}

#[test]
fn an_auth_record_id_tells_actors_that_collide_under_fnv1a_apart() {
    let a = legal_audit_to_auth_record(&entry(AuditEventKind::StatuteCreated, 1, A, "y", 5));
    let b = legal_audit_to_auth_record(&entry(AuditEventKind::StatuteCreated, 1, B, "y", 5));
    assert_ne!(a.actor_hash, b.actor_hash);
    assert_ne!(a.content_hash, b.content_hash);
}

#[test]
fn a_crypto_seal_covers_each_party_and_its_order() {
    let a = legal_contract_to_crypto_sealed(&Contract::new(1, &[1, 2], 0));
    let b = legal_contract_to_crypto_sealed(&Contract::new(1, &[2, 1], 0));
    let c = legal_contract_to_crypto_sealed(&Contract::new(1, &[1, 3], 0));
    assert_ne!(a.seal_hash, b.seal_hash);
    assert_ne!(a.seal_hash, c.seal_hash);
    assert_ne!(a.content_hash, b.content_hash);
}
