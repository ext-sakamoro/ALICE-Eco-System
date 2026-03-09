//! FileSystem bridges — ALICE-FileSystem ↔ DB, Cache, Analytics, Monitor, Backup
//!
//! 5 bridges connecting filesystem metadata and I/O telemetry to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: FileSystem → DB (metadata) ─────────────────────────────────

/// Filesystem metadata record for ALICE-DB persistence.
///
/// Captures the structural state of a filesystem snapshot so that DB
/// consumers can track growth, inode exhaustion, and permission changes
/// without re-scanning the live filesystem.
pub struct FilesystemDbMetadata {
    /// FNV-1a hash of the mount-point path identifying this filesystem.
    pub content_hash: u64,
    /// Total number of inodes allocated on the filesystem.
    pub inode_count: u64,
    /// Total bytes used across all files.
    pub total_bytes: u64,
    /// Total regular file count.
    pub file_count: u64,
    /// Total directory count.
    pub dir_count: u64,
    /// Aggregated permission bits summary (OR of all mode_t values observed).
    pub permission_bits: u32,
    /// Unix timestamp in milliseconds of the snapshot.
    pub snapshot_ms: u64,
    /// Filesystem block size in bytes.
    pub block_size: u32,
}

/// Build a filesystem metadata record for ALICE-DB.
///
/// `content_hash` is derived from the mount-point path so that records for
/// different mount points never collide — deterministic, allocation-free.
#[inline]
#[must_use]
pub fn filesystem_to_db_metadata(
    mount_point: &[u8],
    inode_count: u64,
    total_bytes: u64,
    file_count: u64,
    dir_count: u64,
    permission_bits: u32,
    snapshot_ms: u64,
    block_size: u32,
) -> FilesystemDbMetadata {
    let content_hash = fnv1a(mount_point);
    FilesystemDbMetadata {
        content_hash,
        inode_count,
        total_bytes,
        file_count,
        dir_count,
        permission_bits,
        snapshot_ms,
        block_size,
    }
}

// ── Bridge 2: FileSystem → Cache (inode cache) ───────────────────────────

/// Inode cache entry for ALICE-Cache.
///
/// Caches resolved inode metadata so that repeated stat(2) calls for hot
/// paths can be served from cache rather than issuing kernel syscalls.
///
/// TTL is shorter for frequently-written directories (high `write_count`)
/// to limit stale-entry exposure.
pub struct FilesystemCacheInodeEntry {
    /// FNV-1a hash of the absolute file path used as the cache key.
    pub content_hash: u64,
    /// Inode number on the source filesystem.
    pub inode_count: u64,
    /// File size in bytes at the time of caching.
    pub total_bytes: u64,
    /// POSIX permission mode bits (e.g. 0o644).
    pub permission_bits: u32,
    /// Time-to-live in seconds for this inode cache entry.
    pub ttl_seconds: u32,
    /// Number of write operations observed since last cache refresh.
    pub write_count: u32,
}

/// Build an inode cache entry with write-adjusted TTL.
///
/// TTL formula (branchless):
/// - write_count == 0 → 120 s (read-only path, stable)
/// - write_count > 0  →  30 s (mutable path, expire quickly)
///
/// `is_mutable = (write_count > 0) as u32` → branchless selection via multiply.
#[inline]
#[must_use]
pub fn filesystem_to_cache_inode_entry(
    path: &[u8],
    inode_count: u64,
    total_bytes: u64,
    permission_bits: u32,
    write_count: u32,
) -> FilesystemCacheInodeEntry {
    let content_hash = fnv1a(path);
    // Branchless TTL: read-only=120 s, mutable=30 s.
    let is_mutable = (write_count > 0) as u32;
    let ttl_seconds = 120 - is_mutable * 90; // 120 or 30
    FilesystemCacheInodeEntry {
        content_hash,
        inode_count,
        total_bytes,
        permission_bits,
        ttl_seconds,
        write_count,
    }
}

// ── Bridge 3: FileSystem → Analytics (I/O metrics) ───────────────────────

/// I/O metrics event for ALICE-Analytics ingestion.
///
/// Records per-operation counters so that the analytics layer can build
/// throughput histograms and detect I/O hotspots without storing raw data.
pub struct FilesystemAnalyticsIoEvent {
    /// FNV-1a hash of the mount-point path identifying this filesystem.
    pub content_hash: u64,
    /// Total bytes read since the last sampling interval.
    pub total_bytes: u64,
    /// Total read operation count in the sampling interval.
    pub file_count: u64,
    /// Total write operation count in the sampling interval.
    pub dir_count: u64,
    /// Average read latency in microseconds during the interval.
    pub read_latency_us: u64,
    /// Average write latency in microseconds during the interval.
    pub write_latency_us: u64,
    /// Unix timestamp in milliseconds when the interval ended.
    pub timestamp_ms: u64,
    /// Estimated I/O throughput in bytes per second (read + write combined).
    pub throughput_bps: u64,
}

/// Build a filesystem I/O metrics event for ALICE-Analytics.
///
/// `throughput_bps` is estimated as `(read_bytes + write_bytes) / interval_ms * 1000`
/// using integer multiply-by-reciprocal: `(bytes * 1000) / interval_ms`.
/// Guard against zero interval by clamping denominator to 1.
#[inline]
#[must_use]
pub fn filesystem_to_analytics_io_event(
    mount_point: &[u8],
    read_bytes: u64,
    write_bytes: u64,
    read_ops: u64,
    write_ops: u64,
    read_latency_us: u64,
    write_latency_us: u64,
    timestamp_ms: u64,
    interval_ms: u64,
) -> FilesystemAnalyticsIoEvent {
    let content_hash = fnv1a(mount_point);
    let total_bytes = read_bytes.saturating_add(write_bytes);
    // throughput_bps: (total_bytes * 1000) / interval_ms — clamp denom to 1.
    let denom = interval_ms.max(1);
    let throughput_bps = total_bytes.saturating_mul(1000) / denom;
    FilesystemAnalyticsIoEvent {
        content_hash,
        total_bytes,
        file_count: read_ops,
        dir_count: write_ops,
        read_latency_us,
        write_latency_us,
        timestamp_ms,
        throughput_bps,
    }
}

// ── Bridge 4: FileSystem → Monitor (disk health) ─────────────────────────

/// Disk health record for ALICE-Monitor.
///
/// Encodes utilisation and error state so that the monitor layer can
/// raise alerts before a filesystem reaches capacity or accumulates
/// excessive I/O errors.
///
/// `health_level`: 0 = healthy, 1 = degraded (>80% full), 2 = critical (>95% full or errors).
pub struct FilesystemMonitorDiskHealth {
    /// FNV-1a hash of the mount-point path.
    pub content_hash: u64,
    /// Total filesystem capacity in bytes.
    pub total_bytes: u64,
    /// Bytes currently used.
    pub inode_count: u64,
    /// Number of I/O errors observed since last reset.
    pub file_count: u64,
    /// Utilisation in integer percent (0–100).
    pub utilisation_pct: u8,
    /// Health level (0=healthy, 1=degraded, 2=critical).
    pub health_level: u8,
    /// Unix timestamp in milliseconds of the health check.
    pub timestamp_ms: u64,
}

/// Build a disk health record for ALICE-Monitor.
///
/// Utilisation percent = `(used_bytes * 100) / capacity_bytes` — integer
/// division, branchless clamp via `.min(100)`.
/// Health level derivation:
/// - io_errors > 0 OR util >= 95 → critical (2)
/// - util >= 80               → degraded (1)
/// - else                     → healthy (0)
#[inline]
#[must_use]
pub fn filesystem_to_monitor_disk_health(
    mount_point: &[u8],
    capacity_bytes: u64,
    used_bytes: u64,
    io_errors: u64,
    timestamp_ms: u64,
) -> FilesystemMonitorDiskHealth {
    let content_hash = fnv1a(mount_point);
    let cap = capacity_bytes.max(1);
    let utilisation_pct = ((used_bytes * 100) / cap).min(100) as u8;
    // Branchless health level via integer conditions.
    let has_errors = (io_errors > 0) as u8;
    let is_critical_util = (utilisation_pct >= 95) as u8;
    let is_degraded = (utilisation_pct >= 80) as u8;
    let health_level = ((has_errors | is_critical_util) * 2).max(is_degraded);
    FilesystemMonitorDiskHealth {
        content_hash,
        total_bytes: capacity_bytes,
        inode_count: used_bytes,
        file_count: io_errors,
        utilisation_pct,
        health_level,
        timestamp_ms,
    }
}

// ── Bridge 5: FileSystem → Backup (snapshot) ─────────────────────────────

/// Backup snapshot record for ALICE-Backup.
///
/// Describes a filesystem snapshot so that the backup layer can schedule
/// incremental transfers and verify data integrity via hash comparison.
pub struct FilesystemBackupSnapshot {
    /// FNV-1a hash of the snapshot path used as the backup key.
    pub content_hash: u64,
    /// Total bytes included in this snapshot.
    pub total_bytes: u64,
    /// Number of files included in the snapshot.
    pub file_count: u64,
    /// Number of directories included in the snapshot.
    pub dir_count: u64,
    /// Permission bits summary for the snapshot root.
    pub permission_bits: u32,
    /// Estimated compressed size in bytes (using 60% compression ratio heuristic).
    pub compressed_bytes: u64,
    /// Unix timestamp in milliseconds when the snapshot was taken.
    pub snapshot_ms: u64,
    /// Snapshot generation number (monotonically increasing per mount point).
    pub generation: u32,
}

/// Build a filesystem backup snapshot record for ALICE-Backup.
///
/// `compressed_bytes` is estimated as `total_bytes * 6 / 10` — integer
/// multiply then divide, no floating-point, no branches.
/// Guard against zero total_bytes: result is 0, which is correct.
#[inline]
#[must_use]
pub fn filesystem_to_backup_snapshot(
    snapshot_path: &[u8],
    total_bytes: u64,
    file_count: u64,
    dir_count: u64,
    permission_bits: u32,
    snapshot_ms: u64,
    generation: u32,
) -> FilesystemBackupSnapshot {
    let content_hash = fnv1a(snapshot_path);
    // 60% compression ratio heuristic: * 6 / 10 — integer, no FP.
    let compressed_bytes = total_bytes * 6 / 10;
    FilesystemBackupSnapshot {
        content_hash,
        total_bytes,
        file_count,
        dir_count,
        permission_bits,
        compressed_bytes,
        snapshot_ms,
        generation,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_to_db_metadata_basic() {
        let m = filesystem_to_db_metadata(
            b"/data",
            500_000,
            10_000_000,
            400_000,
            100_000,
            0o755,
            1_700_000_000_000,
            4096,
        );
        assert_eq!(m.content_hash, fnv1a(b"/data"));
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.inode_count, 500_000);
        assert_eq!(m.total_bytes, 10_000_000);
        assert_eq!(m.file_count, 400_000);
        assert_eq!(m.dir_count, 100_000);
        assert_eq!(m.block_size, 4096);
    }

    #[test]
    fn test_filesystem_to_cache_inode_entry_readonly_ttl() {
        // write_count = 0 → TTL = 120 s
        let e = filesystem_to_cache_inode_entry(b"/etc/passwd", 12345, 1024, 0o644, 0);
        assert_eq!(e.ttl_seconds, 120);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_filesystem_to_cache_inode_entry_mutable_ttl() {
        // write_count > 0 → TTL = 30 s
        let e = filesystem_to_cache_inode_entry(b"/var/log/app.log", 99999, 2_048_000, 0o644, 42);
        assert_eq!(e.ttl_seconds, 30);
        assert_eq!(e.write_count, 42);
    }

    #[test]
    fn test_filesystem_to_analytics_io_event_throughput() {
        // (read=100_000 + write=50_000) * 1000 / 500 ms = 300_000 bps
        let ev = filesystem_to_analytics_io_event(
            b"/mnt/ssd",
            100_000,
            50_000,
            200,
            50,
            500,
            1200,
            0,
            500,
        );
        assert_eq!(ev.throughput_bps, 300_000);
        assert_eq!(ev.total_bytes, 150_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_filesystem_to_analytics_io_event_zero_interval() {
        // interval_ms = 0 must not panic (denominator clamped to 1)
        let ev = filesystem_to_analytics_io_event(b"/mnt/ssd", 1000, 500, 1, 1, 100, 200, 0, 0);
        assert!(ev.throughput_bps > 0);
    }

    #[test]
    fn test_filesystem_to_monitor_disk_health_levels() {
        // Healthy: 50% utilisation, no errors
        let h = filesystem_to_monitor_disk_health(b"/data", 1000, 500, 0, 0);
        assert_eq!(h.utilisation_pct, 50);
        assert_eq!(h.health_level, 0);
        // Degraded: 85% utilisation, no errors
        let d = filesystem_to_monitor_disk_health(b"/data", 1000, 850, 0, 0);
        assert_eq!(d.health_level, 1);
        // Critical: 97% utilisation
        let c = filesystem_to_monitor_disk_health(b"/data", 1000, 970, 0, 0);
        assert_eq!(c.health_level, 2);
        // Critical: has I/O errors regardless of utilisation
        let e = filesystem_to_monitor_disk_health(b"/data", 1000, 100, 3, 0);
        assert_eq!(e.health_level, 2);
    }

    #[test]
    fn test_filesystem_to_backup_snapshot_compressed_bytes() {
        // compressed_bytes = total_bytes * 6 / 10
        let s =
            filesystem_to_backup_snapshot(b"/snapshots/2026", 1_000_000, 8000, 200, 0o755, 0, 5);
        assert_eq!(s.compressed_bytes, 600_000);
        assert_eq!(s.generation, 5);
        assert_ne!(s.content_hash, 0);
    }

    #[test]
    fn test_fnv1a_deterministic_and_distinct() {
        let h1 = fnv1a(b"alice-filesystem");
        let h2 = fnv1a(b"alice-filesystem");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
        assert_ne!(fnv1a(b"/data"), fnv1a(b"/logs"));
    }
}
