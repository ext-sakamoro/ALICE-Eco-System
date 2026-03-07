//! Consensus bridges — ALICE-Consensus ↔ DB, Analytics, Cache, Sync, Edge
//!
//! 5 bridges connecting the distributed consensus layer to the ALICE ecosystem.

use alice_consensus::{has_quorum, LogEntry, RaftState, Role};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// `Role` を u8 に変換（`as u8` キャスト禁止ルール準拠）
#[inline(always)]
fn role_to_u8(role: Role) -> u8 {
    match role {
        Role::Follower => 0,
        Role::Candidate => 1,
        Role::Leader => 2,
    }
}

// ── Bridge 1: Consensus → DB (consensus log persistence) ─────────────────

/// Raft log entry persistence record for ALICE-DB.
///
/// Written by the leader after appending a new log entry so that crash
/// recovery can restore the Raft log from the database instead of requiring
/// a full snapshot replay.
pub struct ConsensusDbLogRecord {
    /// FNV-1a hash of the log entry command — DB row key.
    pub content_hash: u64,
    /// Raft term when the entry was created.
    pub term: u64,
    /// Log index (monotonically increasing).
    pub index: u64,
    /// Command payload size in bytes.
    pub command_bytes: usize,
    /// FNV-1a hash of the command payload — integrity check.
    pub command_hash: u64,
    /// Node ID that created this entry.
    pub node_id: u64,
}

/// Build a Raft log entry persistence record for ALICE-DB.
#[inline]
#[must_use]
pub fn consensus_to_db_log_record(
    entry: &LogEntry,
    node_id: u64,
) -> ConsensusDbLogRecord {
    let content_hash = fnv1a(&entry.command);
    let command_hash = fnv1a(&entry.command);
    ConsensusDbLogRecord {
        content_hash,
        term: entry.term,
        index: entry.index,
        command_bytes: entry.command.len(),
        command_hash,
        node_id,
    }
}

// ── Bridge 2: Consensus → Analytics (consensus metrics) ──────────────────

/// Consensus state metrics event for ALICE-Analytics.
///
/// Emitted periodically so the analytics layer can track term progression,
/// leader election frequency, commit-apply lag, and quorum health.
pub struct ConsensusAnalyticsEvent {
    /// FNV-1a hash of node ID + current term — analytics stream key.
    pub content_hash: u64,
    /// Current node role as u8: 0=Follower, 1=Candidate, 2=Leader.
    pub role: u8,
    /// Current Raft term.
    pub current_term: u64,
    /// Commit index.
    pub commit_index: u64,
    /// Last applied index.
    pub last_applied: u64,
    /// Log length (number of entries).
    pub log_len: usize,
    /// Commit-apply lag (commit_index - last_applied).
    pub apply_lag: u64,
}

/// Build a consensus state metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn consensus_to_analytics_event(state: &RaftState) -> ConsensusAnalyticsEvent {
    let mut hash_data = [0u8; 16];
    hash_data[..8].copy_from_slice(&state.id.to_le_bytes());
    hash_data[8..16].copy_from_slice(&state.current_term.to_le_bytes());
    let content_hash = fnv1a(&hash_data);
    let apply_lag = state.commit_index.saturating_sub(state.last_applied);
    ConsensusAnalyticsEvent {
        content_hash,
        role: role_to_u8(state.role),
        current_term: state.current_term,
        commit_index: state.commit_index,
        last_applied: state.last_applied,
        log_len: state.log.len(),
        apply_lag,
    }
}

// ── Bridge 3: Consensus → Cache (leader info cache) ───────────────────────

/// Cached leader information for ALICE-Cache.
///
/// Clients and other services query the cache to find the current Raft leader
/// without sending a full round of RequestVote messages.
/// TTL is branchlessly set to 5 seconds when the node is a Leader (short TTL
/// to detect leadership loss quickly) and 30 seconds when Follower/Candidate.
pub struct ConsensusCacheLeaderEntry {
    /// FNV-1a hash of node ID — cache key.
    pub content_hash: u64,
    /// Node ID of the current leader (self.id when this node is the leader).
    pub leader_id: u64,
    /// Current Raft term.
    pub current_term: u64,
    /// Role as u8.
    pub role: u8,
    /// Cache TTL in seconds (branchless: 5 for Leader, 30 otherwise).
    pub ttl_secs: u32,
    /// True when this node believes it is the leader.
    pub is_self_leader: bool,
}

/// Build a cached leader info entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn consensus_to_cache_leader_entry(state: &RaftState) -> ConsensusCacheLeaderEntry {
    let content_hash = fnv1a(&state.id.to_le_bytes());
    let role_u8 = role_to_u8(state.role);
    let is_self_leader = state.role == Role::Leader;
    // ブランチレスTTL: Leader(2) → 5秒、それ以外 → 30秒
    let is_leader = is_self_leader as u32;
    let ttl_secs = 30 - is_leader * 25;
    ConsensusCacheLeaderEntry {
        content_hash,
        leader_id: state.id,
        current_term: state.current_term,
        role: role_u8,
        ttl_secs,
        is_self_leader,
    }
}

// ── Bridge 4: Consensus → Sync (state sync) ───────────────────────────────

/// State synchronization payload for ALICE-Sync.
///
/// After a leader commits a new entry, the Sync layer distributes the
/// committed state to read replicas and edge caches so they can serve
/// strongly-consistent reads.
pub struct ConsensusSyncPayload {
    /// FNV-1a hash of commit_index + last_applied — Sync routing key.
    pub content_hash: u64,
    /// Commit index of the state being synced.
    pub commit_index: u64,
    /// Last applied index at sync time.
    pub last_applied: u64,
    /// Current term.
    pub current_term: u64,
    /// Number of log entries since the last sync checkpoint.
    pub new_entries: usize,
    /// True when a quorum is reachable (from `has_quorum`).
    pub quorum_ok: bool,
}

/// Build a state synchronization payload for ALICE-Sync.
///
/// `votes_received` is the number of acknowledgements collected by the leader
/// for the latest AppendEntries round.
#[inline]
#[must_use]
pub fn consensus_to_sync_payload(
    state: &RaftState,
    votes_received: usize,
    checkpoint_index: u64,
) -> ConsensusSyncPayload {
    let mut hash_data = [0u8; 16];
    hash_data[..8].copy_from_slice(&state.commit_index.to_le_bytes());
    hash_data[8..16].copy_from_slice(&state.last_applied.to_le_bytes());
    let content_hash = fnv1a(&hash_data);
    let new_entries = state
        .commit_index
        .saturating_sub(checkpoint_index) as usize;
    let quorum_ok = has_quorum(votes_received, state.cluster_size);
    ConsensusSyncPayload {
        content_hash,
        commit_index: state.commit_index,
        last_applied: state.last_applied,
        current_term: state.current_term,
        new_entries,
        quorum_ok,
    }
}

// ── Bridge 5: Consensus → Edge (consensus events) ─────────────────────────

/// Consensus state change event for ALICE-Edge.
///
/// Forwarded to edge agents on every term change or leader election so they
/// can invalidate stale leader caches and re-route writes to the new leader
/// without waiting for the next health-check cycle.
pub struct ConsensusEdgeEvent {
    /// FNV-1a hash of node ID + new term — edge routing key.
    pub content_hash: u64,
    /// New Raft term after the state change.
    pub current_term: u64,
    /// Node role after the change: 0=Follower, 1=Candidate, 2=Leader.
    pub role: u8,
    /// Commit index at the time of the event.
    pub commit_index: u64,
    /// Log length at the time of the event.
    pub log_len: usize,
    /// True when the node is the new leader.
    pub is_leader: bool,
}

/// Build a consensus state change event for ALICE-Edge.
#[inline]
#[must_use]
pub fn consensus_to_edge_event(state: &RaftState) -> ConsensusEdgeEvent {
    let mut hash_data = [0u8; 16];
    hash_data[..8].copy_from_slice(&state.id.to_le_bytes());
    hash_data[8..16].copy_from_slice(&state.current_term.to_le_bytes());
    let content_hash = fnv1a(&hash_data);
    ConsensusEdgeEvent {
        content_hash,
        current_term: state.current_term,
        role: role_to_u8(state.role),
        commit_index: state.commit_index,
        log_len: state.log.len(),
        is_leader: state.role == Role::Leader,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_follower() -> RaftState {
        RaftState::new(1, 3)
    }

    fn make_leader() -> RaftState {
        let mut s = RaftState::new(2, 3);
        s.role = Role::Leader;
        s.current_term = 3;
        s
    }

    fn make_log_entry() -> LogEntry {
        LogEntry { term: 1, index: 1, command: vec![0xCA, 0xFE, 0xBA, 0xBE] }
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_log_record_hash_nonzero() {
        let entry = make_log_entry();
        let rec = consensus_to_db_log_record(&entry, 1);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_db_log_record_fields() {
        let entry = make_log_entry();
        let rec = consensus_to_db_log_record(&entry, 42);
        assert_eq!(rec.term, 1);
        assert_eq!(rec.index, 1);
        assert_eq!(rec.command_bytes, 4);
        assert_eq!(rec.node_id, 42);
        assert_ne!(rec.command_hash, 0);
    }

    #[test]
    fn test_db_log_record_determinism() {
        let entry = make_log_entry();
        let r1 = consensus_to_db_log_record(&entry, 1);
        let r2 = consensus_to_db_log_record(&entry, 1);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_event_hash_nonzero() {
        let state = make_follower();
        let ev = consensus_to_analytics_event(&state);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_analytics_event_follower_fields() {
        let state = make_follower();
        let ev = consensus_to_analytics_event(&state);
        assert_eq!(ev.role, 0); // Follower
        assert_eq!(ev.current_term, 0);
        assert_eq!(ev.log_len, 0);
        assert_eq!(ev.apply_lag, 0);
    }

    #[test]
    fn test_analytics_event_leader_apply_lag() {
        let mut state = make_leader();
        state.commit_index = 5;
        state.last_applied = 3;
        let ev = consensus_to_analytics_event(&state);
        assert_eq!(ev.role, 2); // Leader
        assert_eq!(ev.apply_lag, 2);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_leader_entry_follower_ttl() {
        let state = make_follower();
        let entry = consensus_to_cache_leader_entry(&state);
        assert_ne!(entry.content_hash, 0);
        // Follower → TTL = 30
        assert_eq!(entry.ttl_secs, 30);
        assert!(!entry.is_self_leader);
    }

    #[test]
    fn test_cache_leader_entry_leader_ttl() {
        let state = make_leader();
        let entry = consensus_to_cache_leader_entry(&state);
        // Leader → TTL = 5
        assert_eq!(entry.ttl_secs, 5);
        assert!(entry.is_self_leader);
    }

    #[test]
    fn test_cache_leader_entry_determinism() {
        let state = make_follower();
        let e1 = consensus_to_cache_leader_entry(&state);
        let e2 = consensus_to_cache_leader_entry(&state);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_sync_payload_hash_nonzero() {
        let state = make_leader();
        let payload = consensus_to_sync_payload(&state, 2, 0);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_sync_payload_quorum_ok() {
        let state = make_leader();
        // 2 votes in cluster of 3 → quorum
        let payload = consensus_to_sync_payload(&state, 2, 0);
        assert!(payload.quorum_ok);
    }

    #[test]
    fn test_sync_payload_no_quorum() {
        let state = make_follower();
        // 1 vote in cluster of 3 → no quorum
        let payload = consensus_to_sync_payload(&state, 1, 0);
        assert!(!payload.quorum_ok);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_event_hash_nonzero() {
        let state = make_leader();
        let ev = consensus_to_edge_event(&state);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_edge_event_leader_fields() {
        let state = make_leader();
        let ev = consensus_to_edge_event(&state);
        assert_eq!(ev.role, 2); // Leader
        assert_eq!(ev.current_term, 3);
        assert!(ev.is_leader);
    }

    #[test]
    fn test_edge_event_determinism() {
        let state = make_follower();
        let e1 = consensus_to_edge_event(&state);
        let e2 = consensus_to_edge_event(&state);
        assert_eq!(e1.content_hash, e2.content_hash);
    }
}
