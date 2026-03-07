//! Backup bridges — ALICE-Backup ↔ DB, Cache, Analytics, Crypto, Edge
//!
//! 5 bridges connecting the backup engine to the ALICE ecosystem.

use alice_backup::{expired_snapshots, RetentionPolicy, Snapshot};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Backup → DB (backup records) ───────────────────────────────

/// Snapshot metadata record for ALICE-DB.
///
/// Written when a new snapshot is created so that backup catalogs remain
/// durable across application restarts and storage migrations.
pub struct BackupDbRecord {
    /// FNV-1a hash of the snapshot ID — DB row key.
    pub content_hash: u64,
    /// Snapshot ID assigned by `GenerationManager`.
    pub snapshot_id: u64,
    /// Parent snapshot ID (0 when this is a full snapshot).
    pub parent_id: u64,
    /// Snapshot creation timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Full-data checksum from `Snapshot::full_checksum`.
    pub full_checksum: u64,
    /// True when this is a full (non-incremental) snapshot.
    pub is_full: bool,
    /// Number of changed blocks in this snapshot's diff.
    pub block_count: usize,
}

/// Build a snapshot metadata record for ALICE-DB.
#[inline]
#[must_use]
pub fn backup_to_db_record(snapshot: &Snapshot) -> BackupDbRecord {
    let id_bytes = snapshot.id.to_le_bytes();
    let content_hash = fnv1a(&id_bytes);
    BackupDbRecord {
        content_hash,
        snapshot_id: snapshot.id,
        parent_id: snapshot.parent_id.unwrap_or(0),
        timestamp_ms: snapshot.timestamp,
        full_checksum: snapshot.full_checksum,
        is_full: snapshot.is_full,
        block_count: snapshot.diff.len(),
    }
}

// ── Bridge 2: Backup → Cache (backup cache) ───────────────────────────────

/// Cached snapshot entry for ALICE-Cache.
///
/// The latest snapshot ID and checksum are cached so that incremental backup
/// jobs can quickly locate the parent without scanning the full catalog.
/// TTL is branchlessly set to 120 seconds for full snapshots (longer validity)
/// and 60 seconds for incremental snapshots.
pub struct BackupCacheEntry {
    /// FNV-1a hash of the snapshot checksum — cache key.
    pub content_hash: u64,
    /// Snapshot ID.
    pub snapshot_id: u64,
    /// Full-data checksum.
    pub full_checksum: u64,
    /// Number of diff blocks.
    pub block_count: usize,
    /// Cache TTL in seconds (branchless: 120 for full, 60 for incremental).
    pub ttl_secs: u32,
}

/// Build a cached snapshot entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn backup_to_cache_entry(snapshot: &Snapshot) -> BackupCacheEntry {
    let content_hash = fnv1a(&snapshot.full_checksum.to_le_bytes());
    // ブランチレスTTL: full → 120秒、incremental → 60秒
    let is_full = snapshot.is_full as u32;
    let ttl_secs = 60 + is_full * 60;
    BackupCacheEntry {
        content_hash,
        snapshot_id: snapshot.id,
        full_checksum: snapshot.full_checksum,
        block_count: snapshot.diff.len(),
        ttl_secs,
    }
}

// ── Bridge 3: Backup → Analytics (backup metrics) ────────────────────────

/// Backup run metrics event for ALICE-Analytics.
///
/// Emitted after each backup run to track snapshot frequency, data growth,
/// retention compliance, and storage utilization trends.
pub struct BackupAnalyticsMetrics {
    /// FNV-1a hash of snapshot counts — analytics stream key.
    pub content_hash: u64,
    /// Total number of snapshots under management.
    pub snapshot_count: usize,
    /// Number of snapshots that are full (non-incremental).
    pub full_count: usize,
    /// Number of expired snapshots under the retention policy.
    pub expired_count: usize,
    /// Full snapshot ratio in permille.
    pub full_permille: u32,
    /// Retention policy type: 0=KeepAll, 1=KeepLast, 2=KeepDays.
    pub policy_type: u8,
}

/// Build backup run metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn backup_to_analytics_metrics(
    snapshots: &[Snapshot],
    policy: RetentionPolicy,
    now_ms: u64,
) -> BackupAnalyticsMetrics {
    let snapshot_count = snapshots.len();
    let full_count = snapshots.iter().filter(|s| s.is_full).count();
    let expired = expired_snapshots(snapshots, policy, now_ms);
    let expired_count = expired.len();
    let total_safe = snapshot_count.max(1);
    let full_permille = (full_count.min(total_safe) * 1_000 / total_safe) as u32;
    let policy_type = match policy {
        RetentionPolicy::KeepAll => 0u8,
        RetentionPolicy::KeepLast(_) => 1u8,
        RetentionPolicy::KeepDays(_) => 2u8,
    };
    let mut hash_data = [0u8; 8];
    hash_data[..4].copy_from_slice(&(snapshot_count as u32).to_le_bytes());
    hash_data[4..8].copy_from_slice(&(expired_count as u32).to_le_bytes());
    let content_hash = fnv1a(&hash_data);
    BackupAnalyticsMetrics {
        content_hash,
        snapshot_count,
        full_count,
        expired_count,
        full_permille,
        policy_type,
    }
}

// ── Bridge 4: Backup → Crypto (encrypted backup metadata) ────────────────

/// Encrypted backup metadata descriptor for ALICE-Crypto.
///
/// Backup manifests (containing checksums, block maps, and storage paths)
/// are encrypted at rest so that a compromised storage node cannot
/// reconstruct the data layout.
pub struct BackupCryptoMetadata {
    /// FNV-1a hash of the snapshot full_checksum — Crypto envelope key.
    pub content_hash: u64,
    /// Snapshot ID — nonce seed for the Crypto layer.
    pub snapshot_id: u64,
    /// Number of diff blocks — determines the manifest size.
    pub block_count: usize,
    /// Cipher hint: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
    pub cipher: u8,
    /// Estimated manifest plaintext size in bytes.
    pub manifest_bytes: usize,
    /// Estimated ciphertext size (manifest_bytes + 16 tag bytes).
    pub ciphertext_bytes: usize,
}

/// Build an encrypted backup metadata descriptor for ALICE-Crypto.
///
/// `cipher`: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
#[inline]
#[must_use]
pub fn backup_to_crypto_metadata(snapshot: &Snapshot, cipher: u8) -> BackupCryptoMetadata {
    let content_hash = fnv1a(&snapshot.full_checksum.to_le_bytes());
    // マニフェストサイズ: スナップショットID(8) + タイムスタンプ(8) + ブロックごとのオフセット(8) + チェックサム(8)
    let manifest_bytes = 16 + snapshot.diff.len() * 16;
    BackupCryptoMetadata {
        content_hash,
        snapshot_id: snapshot.id,
        block_count: snapshot.diff.len(),
        cipher: cipher.min(1),
        manifest_bytes,
        ciphertext_bytes: manifest_bytes + 16,
    }
}

// ── Bridge 5: Backup → Edge (backup events) ───────────────────────────────

/// Backup completion event for ALICE-Edge.
///
/// Notifies edge agents that a backup has completed so they can update their
/// local recovery point objective (RPO) tracking and trigger replication
/// to secondary storage if needed.
pub struct BackupEdgeEvent {
    /// FNV-1a hash of the snapshot ID — edge routing key.
    pub content_hash: u64,
    /// Snapshot ID of the completed backup.
    pub snapshot_id: u64,
    /// Snapshot creation timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// True when this is a full (non-incremental) snapshot.
    pub is_full: bool,
    /// Number of changed blocks in the diff (0 for full snapshots).
    pub changed_blocks: usize,
    /// Data integrity checksum.
    pub checksum_val: u64,
}

/// Build a backup completion event for ALICE-Edge.
#[inline]
#[must_use]
pub fn backup_to_edge_event(snapshot: &Snapshot) -> BackupEdgeEvent {
    let mut id_bytes = [0u8; 16];
    id_bytes[..8].copy_from_slice(&snapshot.id.to_le_bytes());
    id_bytes[8..16].copy_from_slice(&snapshot.timestamp.to_le_bytes());
    let content_hash = fnv1a(&id_bytes);
    BackupEdgeEvent {
        content_hash,
        snapshot_id: snapshot.id,
        timestamp_ms: snapshot.timestamp,
        is_full: snapshot.is_full,
        changed_blocks: snapshot.diff.len(),
        checksum_val: snapshot.full_checksum,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_backup::GenerationManager;

    fn make_full_snapshot() -> Snapshot {
        let mut mgr = GenerationManager::new(10);
        let id = mgr.create_full(b"test data v1", 1_700_000_000_000);
        mgr.get(id).unwrap().clone()
    }

    fn make_incremental_snapshot() -> Snapshot {
        let mut mgr = GenerationManager::new(10);
        mgr.create_full(b"base data here", 1_000_000_000_000);
        let id = mgr.create_incremental(b"base data here", b"base data XXXX", 4, 1_700_000_000_000);
        mgr.get(id).unwrap().clone()
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_record_hash_nonzero() {
        let snap = make_full_snapshot();
        let rec = backup_to_db_record(&snap);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_db_record_full_fields() {
        let snap = make_full_snapshot();
        let rec = backup_to_db_record(&snap);
        assert_eq!(rec.snapshot_id, 0);
        assert_eq!(rec.parent_id, 0);
        assert!(rec.is_full);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
        assert_ne!(rec.full_checksum, 0);
    }

    #[test]
    fn test_db_record_determinism() {
        let snap = make_full_snapshot();
        let r1 = backup_to_db_record(&snap);
        let r2 = backup_to_db_record(&snap);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_full_ttl() {
        let snap = make_full_snapshot();
        let entry = backup_to_cache_entry(&snap);
        assert_ne!(entry.content_hash, 0);
        // full → TTL = 120
        assert_eq!(entry.ttl_secs, 120);
    }

    #[test]
    fn test_cache_entry_incremental_ttl() {
        let snap = make_incremental_snapshot();
        let entry = backup_to_cache_entry(&snap);
        // incremental → TTL = 60
        assert_eq!(entry.ttl_secs, 60);
        assert!(!snap.is_full);
    }

    #[test]
    fn test_cache_entry_hash_nonzero() {
        let snap = make_full_snapshot();
        let entry = backup_to_cache_entry(&snap);
        assert_ne!(entry.content_hash, 0);
        assert_ne!(entry.full_checksum, 0);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_metrics_hash_nonzero() {
        let snap = make_full_snapshot();
        let snapshots = vec![snap];
        let m = backup_to_analytics_metrics(&snapshots, RetentionPolicy::KeepAll, 0);
        assert_ne!(m.content_hash, 0);
    }

    #[test]
    fn test_analytics_metrics_fields() {
        let snaps: Vec<Snapshot> = (0..3)
            .map(|i| Snapshot {
                id: i,
                parent_id: None,
                timestamp: i * 1000,
                full_checksum: 12345,
                diff: vec![],
                is_full: i == 0,
            })
            .collect();
        let m = backup_to_analytics_metrics(&snaps, RetentionPolicy::KeepLast(2), 5_000);
        assert_eq!(m.snapshot_count, 3);
        assert_eq!(m.full_count, 1);
        assert_eq!(m.expired_count, 1);
        assert_eq!(m.policy_type, 1); // KeepLast
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_crypto_metadata_hash_nonzero() {
        let snap = make_full_snapshot();
        let meta = backup_to_crypto_metadata(&snap, 0);
        assert_ne!(meta.content_hash, 0);
    }

    #[test]
    fn test_crypto_metadata_fields() {
        let snap = make_full_snapshot();
        let meta = backup_to_crypto_metadata(&snap, 0);
        assert_eq!(meta.cipher, 0);
        assert_eq!(meta.ciphertext_bytes, meta.manifest_bytes + 16);
        assert_eq!(meta.snapshot_id, 0);
    }

    #[test]
    fn test_crypto_metadata_cipher_clamped() {
        let snap = make_full_snapshot();
        let meta = backup_to_crypto_metadata(&snap, 99);
        assert_eq!(meta.cipher, 1);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_event_hash_nonzero() {
        let snap = make_full_snapshot();
        let ev = backup_to_edge_event(&snap);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_edge_event_determinism() {
        let snap = make_full_snapshot();
        let e1 = backup_to_edge_event(&snap);
        let e2 = backup_to_edge_event(&snap);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.checksum_val, e2.checksum_val);
    }
}
