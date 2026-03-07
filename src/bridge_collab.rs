//! Collab bridges — ALICE-Collab ↔ DB, Cache, Analytics, Sync, VCS
//!
//! 5 bridges connecting the collaborative editing layer to the ALICE ecosystem.
//! Covers CRDT state persistence, state caching, collab metric telemetry,
//! state synchronization, and version-control tracking.

extern crate alloc;

use alice_collab::{apply_op, GCounter, LWWRegister, PNCounter, TextOp};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Collab → DB (CRDT state persistence) ───────────────────────

/// CRDT state persistence record for ALICE-DB.
///
/// Written periodically so that CRDT state survives process restarts.
/// The `content_hash` is derived from both the replica ID and the current
/// counter value so that concurrent snapshots from different replicas are
/// distinguishable.
pub struct CollabDbCrdtRecord {
    /// FNV-1a hash of replica ID bytes XOR counter value bytes.
    pub content_hash: u64,
    /// Replica identifier that produced this snapshot.
    pub replica_id: u64,
    /// G-Counter value at snapshot time.
    pub gcounter_value: u64,
    /// PN-Counter value at snapshot time (signed).
    pub pncounter_value: i64,
    /// Document version (monotonically increasing snapshot index).
    pub doc_version: u64,
}

/// Build a CRDT state persistence record for ALICE-DB.
#[inline]
#[must_use]
pub fn collab_to_db_crdt_record(
    replica_id: u64,
    gcounter: &GCounter,
    pncounter: &PNCounter,
    doc_version: u64,
) -> CollabDbCrdtRecord {
    let mut data = [0u8; 24];
    data[0..8].copy_from_slice(&replica_id.to_le_bytes());
    data[8..16].copy_from_slice(&gcounter.value().to_le_bytes());
    data[16..24].copy_from_slice(&doc_version.to_le_bytes());
    CollabDbCrdtRecord {
        content_hash: fnv1a(&data),
        replica_id,
        gcounter_value: gcounter.value(),
        pncounter_value: pncounter.value(),
        doc_version,
    }
}

// ── Bridge 2: Collab → Cache (CRDT state cache) ──────────────────────────

/// CRDT state cache entry for ALICE-Cache.
///
/// Caches the last-known CRDT state so that newly joining collaborators can
/// bootstrap without a full DB read.  TTL is set branchlessly: 60 s when the
/// document is actively edited (gcounter_value > 0), else 300 s (idle doc).
pub struct CollabCacheCrdtEntry {
    /// FNV-1a hash of the document identifier.
    pub content_hash: u64,
    /// G-Counter value (edit count proxy).
    pub gcounter_value: u64,
    /// PN-Counter value (net operation count).
    pub pncounter_value: i64,
    /// Cache TTL in seconds (branchless: 60 active, 300 idle).
    pub ttl_secs: u32,
    /// Number of known replicas (used by joiner for convergence check).
    pub replica_count: u32,
}

/// Build a CRDT state cache entry for ALICE-Cache.
///
/// `ttl_secs` is branchless: 60 when `gcounter_value > 0` (active), else 300.
#[inline]
#[must_use]
pub fn collab_to_cache_crdt_entry(
    doc_id: &str,
    gcounter: &GCounter,
    pncounter: &PNCounter,
    replica_count: u32,
) -> CollabCacheCrdtEntry {
    let content_hash = fnv1a(doc_id.as_bytes());
    let gcv = gcounter.value();
    let is_active = (gcv > 0) as u32;
    // ブランチレス TTL: アクティブ=60s, アイドル=300s
    let ttl_secs = 300 - is_active * 240;
    CollabCacheCrdtEntry {
        content_hash,
        gcounter_value: gcv,
        pncounter_value: pncounter.value(),
        ttl_secs,
        replica_count,
    }
}

// ── Bridge 3: Collab → Analytics (collab metrics) ────────────────────────

/// Collaborative editing metric event for ALICE-Analytics.
///
/// Emitted when an OT operation is applied so the analytics layer can track
/// edit velocity, conflict rates, and per-document collaboration intensity.
pub struct CollabAnalyticsOpEvent {
    /// FNV-1a hash of the document identifier.
    pub content_hash: u64,
    /// Operation type: 0=Insert, 1=Delete.
    pub op_type: u8,
    /// Character position of the operation.
    pub position: usize,
    /// Number of characters inserted or deleted.
    pub char_count: usize,
    /// Document length after applying the operation.
    pub doc_len_after: usize,
}

/// Build a collab analytics event from an applied `TextOp`.
///
/// `doc_before` is the document text before applying `op`; the post-op
/// document length is computed via `apply_op` to avoid an extra parameter.
#[inline]
#[must_use]
pub fn collab_to_analytics_op_event(
    doc_id: &str,
    doc_before: &str,
    op: &TextOp,
) -> CollabAnalyticsOpEvent {
    let content_hash = fnv1a(doc_id.as_bytes());
    let (op_type, position, char_count) = match op {
        TextOp::Insert { pos, text } => (0u8, *pos, text.len()),
        TextOp::Delete { pos, len } => (1u8, *pos, *len),
    };
    let doc_after = apply_op(doc_before, op);
    CollabAnalyticsOpEvent {
        content_hash,
        op_type,
        position,
        char_count,
        doc_len_after: doc_after.len(),
    }
}

// ── Bridge 4: Collab → Sync (CRDT state sync) ────────────────────────────

/// CRDT state synchronization payload for ALICE-Sync.
///
/// When a replica's local state diverges from the cluster, this record is
/// forwarded to ALICE-Sync which orchestrates the merge and re-broadcast.
pub struct CollabSyncPayload {
    /// FNV-1a hash of replica ID + G-Counter value.
    pub content_hash: u64,
    /// Originating replica identifier.
    pub replica_id: u64,
    /// G-Counter value at the time of sync.
    pub gcounter_value: u64,
    /// LWW-Register timestamp (last write wins, used for conflict resolution).
    pub lww_timestamp: u64,
    /// True when the PN-Counter is negative (net-delete document state).
    pub is_net_delete: bool,
}

/// Build a CRDT sync payload for ALICE-Sync.
#[inline]
#[must_use]
pub fn collab_to_sync_payload(
    replica_id: u64,
    gcounter: &GCounter,
    pncounter: &PNCounter,
    lww: &LWWRegister<u64>,
) -> CollabSyncPayload {
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&replica_id.to_le_bytes());
    data[8..16].copy_from_slice(&gcounter.value().to_le_bytes());
    CollabSyncPayload {
        content_hash: fnv1a(&data),
        replica_id,
        gcounter_value: gcounter.value(),
        lww_timestamp: *lww.get(),
        is_net_delete: pncounter.value() < 0,
    }
}

// ── Bridge 5: Collab → VCS (version tracking) ────────────────────────────

/// Version control record for ALICE-VCS.
///
/// Each time a document snapshot is committed, this record is forwarded to
/// ALICE-VCS to create a version entry linked to the collaborating replica.
pub struct CollabVcsCommit {
    /// FNV-1a hash of the document snapshot content.
    pub content_hash: u64,
    /// Replica that triggered the commit.
    pub replica_id: u64,
    /// G-Counter value at commit time (used as logical clock).
    pub logical_clock: u64,
    /// Snapshot text length in bytes.
    pub snapshot_bytes: usize,
    /// PN-Counter value (net edit count for diff sizing).
    pub net_edits: i64,
}

/// Build a VCS commit record for ALICE-VCS from a document snapshot.
#[inline]
#[must_use]
pub fn collab_to_vcs_commit(
    replica_id: u64,
    snapshot: &str,
    gcounter: &GCounter,
    pncounter: &PNCounter,
) -> CollabVcsCommit {
    let content_hash = fnv1a(snapshot.as_bytes());
    CollabVcsCommit {
        content_hash,
        replica_id,
        logical_clock: gcounter.value(),
        snapshot_bytes: snapshot.len(),
        net_edits: pncounter.value(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_collab::{GCounter, LWWRegister, PNCounter, TextOp};

    fn counters() -> (GCounter, PNCounter) {
        let mut gc = GCounter::new();
        gc.increment(1);
        gc.increment(1);
        gc.increment(2);
        let mut pn = PNCounter::new();
        pn.increment(1);
        pn.increment(1);
        pn.decrement(2);
        (gc, pn)
    }

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_collab_to_db_crdt_record_basic() {
        let (gc, pn) = counters();
        let rec = collab_to_db_crdt_record(42, &gc, &pn, 7);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.replica_id, 42);
        assert_eq!(rec.gcounter_value, 3); // 2 + 1
        assert_eq!(rec.pncounter_value, 1); // 2 - 1
        assert_eq!(rec.doc_version, 7);
    }

    #[test]
    fn test_collab_to_db_crdt_record_determinism() {
        let (gc, pn) = counters();
        let r1 = collab_to_db_crdt_record(1, &gc, &pn, 0);
        let r2 = collab_to_db_crdt_record(1, &gc, &pn, 0);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_collab_to_cache_crdt_entry_active_ttl() {
        let (gc, pn) = counters();
        let entry = collab_to_cache_crdt_entry("doc-abc", &gc, &pn, 3);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60); // gcounter > 0 → active
        assert_eq!(entry.replica_count, 3);
    }

    #[test]
    fn test_collab_to_cache_crdt_entry_idle_ttl() {
        let gc = GCounter::new(); // value = 0 → idle
        let pn = PNCounter::new();
        let entry = collab_to_cache_crdt_entry("doc-new", &gc, &pn, 1);
        assert_eq!(entry.ttl_secs, 300); // idle → 300s
    }

    #[test]
    fn test_collab_to_cache_crdt_entry_determinism() {
        let (gc, pn) = counters();
        let e1 = collab_to_cache_crdt_entry("doc-xyz", &gc, &pn, 2);
        let e2 = collab_to_cache_crdt_entry("doc-xyz", &gc, &pn, 2);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_collab_to_analytics_op_event_insert() {
        let op = TextOp::Insert {
            pos: 5,
            text: alloc::string::String::from("XY"),
        };
        let ev = collab_to_analytics_op_event("doc-1", "hello world", &op);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.op_type, 0); // Insert
        assert_eq!(ev.position, 5);
        assert_eq!(ev.char_count, 2);
        assert_eq!(ev.doc_len_after, "hello world".len() + 2);
    }

    #[test]
    fn test_collab_to_analytics_op_event_delete() {
        let op = TextOp::Delete { pos: 0, len: 3 };
        let ev = collab_to_analytics_op_event("doc-2", "hello", &op);
        assert_eq!(ev.op_type, 1); // Delete
        assert_eq!(ev.char_count, 3);
        assert_eq!(ev.doc_len_after, 2);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_collab_to_sync_payload_basic() {
        let (gc, pn) = counters();
        let lww = LWWRegister::new(1_700_000_000_000u64, 100);
        let p = collab_to_sync_payload(99, &gc, &pn, &lww);
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.replica_id, 99);
        assert_eq!(p.gcounter_value, 3);
        assert_eq!(p.lww_timestamp, 1_700_000_000_000);
        assert!(!p.is_net_delete); // pncounter = 1 ≥ 0
    }

    #[test]
    fn test_collab_to_sync_payload_net_delete() {
        let gc = GCounter::new();
        let mut pn = PNCounter::new();
        pn.decrement(1);
        pn.decrement(1); // value = -2
        let lww = LWWRegister::new(0u64, 0);
        let p = collab_to_sync_payload(1, &gc, &pn, &lww);
        assert!(p.is_net_delete);
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_collab_to_vcs_commit_basic() {
        let (gc, pn) = counters();
        let commit = collab_to_vcs_commit(7, "final document text", &gc, &pn);
        assert_ne!(commit.content_hash, 0);
        assert_eq!(commit.replica_id, 7);
        assert_eq!(commit.logical_clock, 3);
        assert_eq!(commit.snapshot_bytes, "final document text".len());
        assert_eq!(commit.net_edits, 1);
    }

    #[test]
    fn test_collab_to_vcs_commit_empty_snapshot() {
        let gc = GCounter::new();
        let pn = PNCounter::new();
        let commit = collab_to_vcs_commit(0, "", &gc, &pn);
        // FNV-1a of empty slice is the basis value (non-zero)
        assert_ne!(commit.content_hash, 0);
        assert_eq!(commit.snapshot_bytes, 0);
        assert_eq!(commit.net_edits, 0);
    }
}
