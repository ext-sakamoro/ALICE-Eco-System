//! Blockchain bridges — ALICE-Blockchain ↔ DB, Cache, Analytics, Ledger, Monitor
//!
//! 5 bridges connecting blockchain block and transaction data (extracted as
//! primitives) to the ALICE ecosystem. No external crate types are imported;
//! all fields use primitive types derived from serialised blockchain state.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Blockchain → DB (block persistence) ─────────────────────────

/// Blockchain block record for ALICE-DB persistence.
pub struct BlockchainDbRecord {
    /// Content hash over chain_hash, block_height, and timestamp_ms.
    pub content_hash: u64,
    /// Current block height (number of confirmed blocks).
    pub block_height: u64,
    /// Number of transactions in this block.
    pub tx_count: u64,
    /// Hash of the block's chain segment identifier.
    pub chain_hash: u64,
    /// Mining difficulty target at this block height.
    pub difficulty: u64,
    /// Unix timestamp in milliseconds when the block was mined.
    pub timestamp_ms: u64,
}

/// Build a DB persistence record from extracted blockchain block data.
#[inline]
#[must_use]
pub fn blockchain_to_db_record(
    chain_id: &[u8],
    block_height: u64,
    tx_count: u64,
    difficulty: u64,
    timestamp_ms: u64,
) -> BlockchainDbRecord {
    let chain_hash = fnv1a(chain_id);
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&chain_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&block_height.to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ms.to_le_bytes());
    BlockchainDbRecord {
        content_hash: fnv1a(&buf),
        block_height,
        tx_count,
        chain_hash,
        difficulty,
        timestamp_ms,
    }
}

// ── Bridge 2: Blockchain → Cache (block header caching) ───────────────────

/// Cached blockchain block header entry for ALICE-Cache.
pub struct BlockchainCacheEntry {
    /// Content hash over block_hash and tx_count.
    pub content_hash: u64,
    /// Hash of the cached block header used as cache key.
    pub block_hash: u64,
    /// TTL in seconds for this cache entry.
    pub ttl_secs: u32,
    /// Number of transactions in the cached block.
    pub tx_count: u64,
    /// Serialised block data size in bytes.
    pub block_bytes: u64,
}

/// Build a cache entry for a blockchain block header.
///
/// TTL is 300 s by default; reduced to 30 s when `tx_count` exceeds 10 000
/// to keep high-throughput chain data current.
#[inline]
#[must_use]
pub fn blockchain_to_cache_entry(
    block_id: &[u8],
    tx_count: u64,
    block_bytes: u64,
) -> BlockchainCacheEntry {
    let block_hash = fnv1a(block_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&block_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tx_count.to_le_bytes());
    let high_throughput = (tx_count > 10_000) as u32;
    let ttl_secs = 300 - high_throughput * 270;
    BlockchainCacheEntry {
        content_hash: fnv1a(&buf),
        block_hash,
        ttl_secs,
        tx_count,
        block_bytes,
    }
}

// ── Bridge 3: Blockchain → Analytics (chain metrics ingestion) ────────────

/// Blockchain chain metrics event for ALICE-Analytics ingestion.
pub struct BlockchainAnalyticsEvent {
    /// Content hash over chain_hash and timestamp_ms.
    pub content_hash: u64,
    /// Number of transactions in the most recent block.
    pub tx_count: u64,
    /// Time to mine the most recent block in milliseconds.
    pub block_time_ms: u64,
    /// Total gas units consumed in the most recent block.
    pub gas_used: u64,
    /// Total transaction fees collected in the most recent block.
    pub fee_total: u64,
    /// Unix timestamp in milliseconds when the event was recorded.
    pub timestamp_ms: u64,
}

/// Build an analytics ingestion event from blockchain chain metrics.
#[inline]
#[must_use]
pub fn blockchain_to_analytics_event(
    chain_id: &[u8],
    tx_count: u64,
    block_time_ms: u64,
    gas_used: u64,
    fee_total: u64,
    timestamp_ms: u64,
) -> BlockchainAnalyticsEvent {
    let chain_hash = fnv1a(chain_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&chain_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    BlockchainAnalyticsEvent {
        content_hash: fnv1a(&buf),
        tx_count,
        block_time_ms,
        gas_used,
        fee_total,
        timestamp_ms,
    }
}

// ── Bridge 4: Blockchain → Ledger (transaction entry) ─────────────────────

/// Blockchain transaction entry for ALICE-Ledger recording.
pub struct BlockchainLedgerEntry {
    /// Content hash over tx_hash and block_height.
    pub content_hash: u64,
    /// Block height at which this transaction was confirmed.
    pub block_height: u64,
    /// Hash of the transaction.
    pub tx_hash: u64,
    /// Hash of the sender address.
    pub from_hash: u64,
    /// Hash of the recipient address.
    pub to_hash: u64,
    /// Transfer amount in the chain's smallest denomination.
    pub amount: u64,
}

/// Build a ledger entry from a confirmed blockchain transaction.
#[inline]
#[must_use]
pub fn blockchain_to_ledger_entry(
    tx_id: &[u8],
    from_addr: &[u8],
    to_addr: &[u8],
    block_height: u64,
    amount: u64,
) -> BlockchainLedgerEntry {
    let tx_hash = fnv1a(tx_id);
    let from_hash = fnv1a(from_addr);
    let to_hash = fnv1a(to_addr);
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&tx_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&block_height.to_le_bytes());
    buf[16..24].copy_from_slice(&from_hash.to_le_bytes());
    buf[24..32].copy_from_slice(&to_hash.to_le_bytes());
    BlockchainLedgerEntry {
        content_hash: fnv1a(&buf),
        block_height,
        tx_hash,
        from_hash,
        to_hash,
        amount,
    }
}

// ── Bridge 5: Blockchain → Monitor (node health status) ───────────────────

/// Blockchain node health status for ALICE-Monitor.
pub struct BlockchainMonitorStatus {
    /// Content hash over chain_hash and timestamp_ms.
    pub content_hash: u64,
    /// Current block height known to this node.
    pub block_height: u64,
    /// Number of connected peers.
    pub peer_count: u32,
    /// Chain sync progress as a percentage (0–100).
    pub sync_pct: u8,
    /// Whether the node has fully synchronised with the network.
    pub is_synced: bool,
    /// Unix timestamp in milliseconds of this status report.
    pub timestamp_ms: u64,
}

/// Build a monitor health status from blockchain node state.
#[inline]
#[must_use]
pub fn blockchain_to_monitor_status(
    chain_id: &[u8],
    block_height: u64,
    peer_count: u32,
    sync_pct: u8,
    is_synced: bool,
    timestamp_ms: u64,
) -> BlockchainMonitorStatus {
    let chain_hash = fnv1a(chain_id);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&chain_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
    BlockchainMonitorStatus {
        content_hash: fnv1a(&buf),
        block_height,
        peer_count,
        sync_pct,
        is_synced,
        timestamp_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DB record tests ───────────────────────────────────────────────────

    #[test]
    fn db_record_content_hash_nonzero() {
        let rec = blockchain_to_db_record(b"mainnet", 800_000, 2_500, 50_000_000_000, 1_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.chain_hash, 0);
    }

    #[test]
    fn db_record_fields_preserved() {
        let rec = blockchain_to_db_record(b"c", 100, 50, 1_000, 99_000);
        assert_eq!(rec.block_height, 100);
        assert_eq!(rec.tx_count, 50);
        assert_eq!(rec.difficulty, 1_000);
        assert_eq!(rec.timestamp_ms, 99_000);
    }

    #[test]
    fn db_record_hash_deterministic() {
        let a = blockchain_to_db_record(b"cx", 0, 0, 1, 0);
        let b = blockchain_to_db_record(b"cx", 0, 0, 1, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Cache entry tests ─────────────────────────────────────────────────

    #[test]
    fn cache_entry_low_tx_count_long_ttl() {
        let entry = blockchain_to_cache_entry(b"blk1", 100, 4_096);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn cache_entry_high_tx_count_short_ttl() {
        let entry = blockchain_to_cache_entry(b"blk2", 50_000, 8_192);
        assert_eq!(entry.ttl_secs, 30);
    }

    // ── Analytics event tests ─────────────────────────────────────────────

    #[test]
    fn analytics_event_fields_and_hash() {
        let ev =
            blockchain_to_analytics_event(b"net-1", 3_000, 12_000, 15_000_000, 500_000, 6_000_000);
        assert_eq!(ev.tx_count, 3_000);
        assert_eq!(ev.block_time_ms, 12_000);
        assert_eq!(ev.gas_used, 15_000_000);
        assert_eq!(ev.fee_total, 500_000);
        assert_ne!(ev.content_hash, 0);
    }

    // ── Ledger entry tests ────────────────────────────────────────────────

    #[test]
    fn ledger_entry_hashes_differ_for_different_addresses() {
        let a = blockchain_to_ledger_entry(b"tx1", b"alice", b"bob", 100, 1_000);
        let b = blockchain_to_ledger_entry(b"tx1", b"alice", b"carol", 100, 1_000);
        assert_ne!(a.to_hash, b.to_hash);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn ledger_entry_amount_preserved() {
        let entry = blockchain_to_ledger_entry(b"tx2", b"from", b"to", 200, 42_000);
        assert_eq!(entry.amount, 42_000);
        assert_eq!(entry.block_height, 200);
        assert_ne!(entry.content_hash, 0);
    }

    // ── Monitor status tests ──────────────────────────────────────────────

    #[test]
    fn monitor_status_synced_flag_preserved() {
        let st = blockchain_to_monitor_status(b"mn", 900_000, 25, 100, true, 4_000_000);
        assert!(st.is_synced);
        assert_eq!(st.peer_count, 25);
        assert_eq!(st.sync_pct, 100);
        assert_ne!(st.content_hash, 0);
    }
}
