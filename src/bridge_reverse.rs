//! Reverse bridges — persistence records → domain objects (deserialize / restore)
//!
//! 5 reverse bridges recovering domain objects from DB records and cache entries.
//! Used for warm-start, replay, and checkpoint restoration across the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: DbRecord → EdgeSensorRestore ───────────────────────────────

/// Restored edge sensor calibration from ALICE-DB persistence record.
///
/// Recovers linear model parameters (Q16.16) and sample statistics
/// for sensor warm-start without re-fitting from raw data.
pub struct DbRecordToEdgeSensorRestore {
    /// FNV-1a content hash of the restoration input.
    pub content_hash: u64,
    /// FNV-1a hash of the original sensor identifier bytes.
    pub sensor_id_hash: u64,
    /// Slope coefficient in Q16.16 fixed-point format.
    pub slope_q16: i32,
    /// Intercept coefficient in Q16.16 fixed-point format.
    pub intercept_q16: i32,
    /// Number of samples from the original calibration run.
    pub sample_count: u32,
    /// Nanosecond timestamp when this restore record was created.
    pub restored_at_ns: u64,
}

/// Restore an edge sensor calibration record from persistence bytes.
///
/// `sensor_id_bytes` — raw bytes of the original sensor identifier.
/// `slope_q16` / `intercept_q16` — Q16.16 fixed-point coefficients.
/// `sample_count` — number of samples in the original fit.
/// `restored_at_ns` — wall-clock nanosecond timestamp of restoration.
#[inline]
#[must_use]
pub fn restore_edge_sensor_from_db(
    sensor_id_bytes: &[u8],
    slope_q16: i32,
    intercept_q16: i32,
    sample_count: u32,
    restored_at_ns: u64,
) -> DbRecordToEdgeSensorRestore {
    // content_hash はセンサーID・係数・サンプル数すべてを含む決定的ハッシュ
    let mut payload = Vec::with_capacity(sensor_id_bytes.len() + 16);
    payload.extend_from_slice(sensor_id_bytes);
    payload.extend_from_slice(&slope_q16.to_le_bytes());
    payload.extend_from_slice(&intercept_q16.to_le_bytes());
    payload.extend_from_slice(&sample_count.to_le_bytes());
    let content_hash = fnv1a(&payload);
    let sensor_id_hash = fnv1a(sensor_id_bytes);
    DbRecordToEdgeSensorRestore {
        content_hash,
        sensor_id_hash,
        slope_q16,
        intercept_q16,
        sample_count,
        restored_at_ns,
    }
}

// ── Bridge 2: CacheEntry → ViewRestore ───────────────────────────────────

/// Restored view frame metadata from ALICE-Cache entry.
///
/// Allows GPU timeline reconstruction from cached frame descriptors
/// without re-rendering the full SDF scene.
pub struct CacheEntryToViewRestore {
    /// FNV-1a content hash of the cache entry.
    pub content_hash: u64,
    /// Frame width in pixels.
    pub frame_width: u32,
    /// Frame height in pixels.
    pub frame_height: u32,
    /// GPU render time for the original frame in microseconds.
    pub gpu_time_us: u64,
    /// Whether the restore succeeded (false = cache miss or corrupt).
    pub restored: bool,
}

/// Restore a view frame descriptor from a cache entry.
///
/// `frame_width` / `frame_height` — pixel dimensions of the cached frame.
/// `gpu_time_us` — original GPU render time in microseconds.
/// `cache_hit` — whether the cache lookup succeeded.
#[inline]
#[must_use]
pub fn restore_view_from_cache(
    frame_width: u32,
    frame_height: u32,
    gpu_time_us: u64,
    cache_hit: bool,
) -> CacheEntryToViewRestore {
    // フレーム寸法とGPU時間で決定的ハッシュを生成
    let mut payload = [0u8; 16];
    payload[0..4].copy_from_slice(&frame_width.to_le_bytes());
    payload[4..8].copy_from_slice(&frame_height.to_le_bytes());
    payload[8..16].copy_from_slice(&gpu_time_us.to_le_bytes());
    let content_hash = fnv1a(&payload);
    CacheEntryToViewRestore {
        content_hash,
        frame_width,
        frame_height,
        gpu_time_us,
        restored: cache_hit,
    }
}

// ── Bridge 3: DbRecord → SdfRestore ──────────────────────────────────────

/// Restored SDF scene descriptor from ALICE-DB persistence record.
///
/// Recovers scene topology metadata for LOD-aware warm-start,
/// allowing the runtime to decide whether a full rebuild is needed.
pub struct DbRecordToSdfRestore {
    /// FNV-1a content hash of the SDF restore descriptor.
    pub content_hash: u64,
    /// Number of SDF nodes in the restored scene graph.
    pub node_count: u32,
    /// Axis-aligned bounding volume of the scene (cubic units).
    pub bounds_volume: f64,
    /// Level-of-detail level at which this snapshot was captured (0 = finest).
    pub lod_level: u8,
    /// Whether the scene requires a full rebuild before use.
    pub needs_rebuild: bool,
}

/// Restore an SDF scene descriptor from a DB persistence record.
///
/// `node_count` — number of SDF nodes in the stored scene graph.
/// `bounds_volume` — axis-aligned bounding volume in cubic scene units.
/// `lod_level` — LOD level at snapshot time (0 = finest detail).
/// `needs_rebuild` — true when the stored snapshot is stale.
#[inline]
#[must_use]
pub fn restore_sdf_from_db(
    node_count: u32,
    bounds_volume: f64,
    lod_level: u8,
    needs_rebuild: bool,
) -> DbRecordToSdfRestore {
    // ノード数・バウンディングボリューム・LODレベルで決定的ハッシュを生成
    let mut payload = [0u8; 13];
    payload[0..4].copy_from_slice(&node_count.to_le_bytes());
    payload[4..12].copy_from_slice(&bounds_volume.to_le_bytes());
    payload[12] = lod_level;
    let content_hash = fnv1a(&payload);
    DbRecordToSdfRestore {
        content_hash,
        node_count,
        bounds_volume,
        lod_level,
        needs_rebuild,
    }
}

// ── Bridge 4: CacheEntry → PhysicsRestore ────────────────────────────────

/// Restored physics world snapshot from ALICE-Cache entry.
///
/// Allows the physics engine to resume from a cached mid-simulation
/// checkpoint, preserving body and joint topology counts.
pub struct CacheEntryToPhysicsRestore {
    /// FNV-1a content hash of the physics snapshot.
    pub content_hash: u64,
    /// Number of rigid bodies in the cached world.
    pub body_count: u32,
    /// Number of constraints/joints in the cached world.
    pub joint_count: u32,
    /// Tick number at which the snapshot was captured.
    pub tick_number: u64,
    /// Whether the simulation was actively running at snapshot time.
    pub simulation_active: bool,
}

/// Restore a physics world snapshot from a cache entry.
///
/// `body_count` — number of rigid bodies at snapshot time.
/// `joint_count` — number of constraints/joints at snapshot time.
/// `tick_number` — simulation tick counter at snapshot time.
/// `simulation_active` — true if the sim was running when snapshotted.
#[inline]
#[must_use]
pub fn restore_physics_from_cache(
    body_count: u32,
    joint_count: u32,
    tick_number: u64,
    simulation_active: bool,
) -> CacheEntryToPhysicsRestore {
    // ボディ数・ジョイント数・ティック番号で決定的ハッシュを生成
    let mut payload = [0u8; 16];
    payload[0..4].copy_from_slice(&body_count.to_le_bytes());
    payload[4..8].copy_from_slice(&joint_count.to_le_bytes());
    payload[8..16].copy_from_slice(&tick_number.to_le_bytes());
    let content_hash = fnv1a(&payload);
    CacheEntryToPhysicsRestore {
        content_hash,
        body_count,
        joint_count,
        tick_number,
        simulation_active,
    }
}

// ── Bridge 5: DbRecord → CryptoRestore ───────────────────────────────────

/// Restored cryptographic key descriptor from ALICE-DB persistence record.
///
/// Recovers SSS shard metadata and key lifecycle information
/// without exposing any raw key material.
pub struct DbRecordToCryptoRestore {
    /// FNV-1a content hash of the crypto restore descriptor.
    pub content_hash: u64,
    /// First 32 bytes of the key fingerprint (SHA-256 digest of public key).
    pub key_fingerprint: [u8; 32],
    /// Number of Shamir Secret Sharing shards.
    pub shard_count: u8,
    /// Nanosecond timestamp of key creation.
    pub created_at_ns: u64,
    /// Whether the key has passed its expiry time.
    pub expired: bool,
}

/// Restore a cryptographic key descriptor from a DB persistence record.
///
/// `key_fingerprint` — 32-byte fingerprint (SHA-256 of the public key).
/// `shard_count` — number of SSS shards issued for this key.
/// `created_at_ns` — original key creation timestamp in nanoseconds.
/// `expired` — true when the key is past its validity window.
#[inline]
#[must_use]
pub fn restore_crypto_from_db(
    key_fingerprint: [u8; 32],
    shard_count: u8,
    created_at_ns: u64,
    expired: bool,
) -> DbRecordToCryptoRestore {
    // フィンガープリント・シャード数・作成日時で決定的ハッシュを生成
    let mut payload = Vec::with_capacity(41);
    payload.extend_from_slice(&key_fingerprint);
    payload.push(shard_count);
    payload.extend_from_slice(&created_at_ns.to_le_bytes());
    let content_hash = fnv1a(&payload);
    DbRecordToCryptoRestore {
        content_hash,
        key_fingerprint,
        shard_count,
        created_at_ns,
        expired,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bridge 1: DbRecord → EdgeSensorRestore ───────────────────────────

    #[test]
    fn test_restore_edge_sensor_content_hash_nonzero() {
        let r = restore_edge_sensor_from_db(b"sensor-001", 65536, 0, 100, 1_000_000_000);
        assert_ne!(r.content_hash, 0);
    }

    #[test]
    fn test_restore_edge_sensor_fields() {
        let r = restore_edge_sensor_from_db(b"sensor-42", 131072, -32768, 200, 9_999_999_999);
        assert_eq!(r.slope_q16, 131072);
        assert_eq!(r.intercept_q16, -32768);
        assert_eq!(r.sample_count, 200);
        assert_eq!(r.restored_at_ns, 9_999_999_999);
        assert_ne!(r.sensor_id_hash, 0);
    }

    #[test]
    fn test_restore_edge_sensor_deterministic() {
        // 同一入力→同一ハッシュ（決定性検証）
        let a = restore_edge_sensor_from_db(b"sensor-x", 1000, 500, 50, 0);
        let b = restore_edge_sensor_from_db(b"sensor-x", 1000, 500, 50, 0);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.sensor_id_hash, b.sensor_id_hash);
    }

    #[test]
    fn test_restore_edge_sensor_different_ids_differ() {
        let a = restore_edge_sensor_from_db(b"sensor-A", 1000, 0, 10, 0);
        let b = restore_edge_sensor_from_db(b"sensor-B", 1000, 0, 10, 0);
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(a.sensor_id_hash, b.sensor_id_hash);
    }

    // ── Bridge 2: CacheEntry → ViewRestore ──────────────────────────────

    #[test]
    fn test_restore_view_cache_hit() {
        let r = restore_view_from_cache(1920, 1080, 8_500, true);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.frame_width, 1920);
        assert_eq!(r.frame_height, 1080);
        assert_eq!(r.gpu_time_us, 8_500);
        assert!(r.restored);
    }

    #[test]
    fn test_restore_view_cache_miss() {
        let r = restore_view_from_cache(1280, 720, 0, false);
        assert!(!r.restored);
        assert_eq!(r.frame_width, 1280);
        assert_eq!(r.frame_height, 720);
    }

    #[test]
    fn test_restore_view_deterministic() {
        let a = restore_view_from_cache(640, 480, 3_000, true);
        let b = restore_view_from_cache(640, 480, 3_000, true);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 3: DbRecord → SdfRestore ─────────────────────────────────

    #[test]
    fn test_restore_sdf_fields() {
        let r = restore_sdf_from_db(42, 1000.5, 2, false);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.node_count, 42);
        assert!((r.bounds_volume - 1000.5).abs() < f64::EPSILON);
        assert_eq!(r.lod_level, 2);
        assert!(!r.needs_rebuild);
    }

    #[test]
    fn test_restore_sdf_needs_rebuild_flag() {
        let r = restore_sdf_from_db(10, 500.0, 0, true);
        assert!(r.needs_rebuild);
        assert_eq!(r.lod_level, 0);
    }

    #[test]
    fn test_restore_sdf_deterministic() {
        let a = restore_sdf_from_db(8, 256.0, 1, false);
        let b = restore_sdf_from_db(8, 256.0, 1, false);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 4: CacheEntry → PhysicsRestore ───────────────────────────

    #[test]
    fn test_restore_physics_fields() {
        let r = restore_physics_from_cache(16, 4, 1024, true);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.body_count, 16);
        assert_eq!(r.joint_count, 4);
        assert_eq!(r.tick_number, 1024);
        assert!(r.simulation_active);
    }

    #[test]
    fn test_restore_physics_inactive_simulation() {
        let r = restore_physics_from_cache(3, 0, 0, false);
        assert!(!r.simulation_active);
        assert_eq!(r.body_count, 3);
        assert_eq!(r.joint_count, 0);
    }

    #[test]
    fn test_restore_physics_deterministic() {
        let a = restore_physics_from_cache(5, 2, 9999, true);
        let b = restore_physics_from_cache(5, 2, 9999, true);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_restore_physics_different_ticks_differ() {
        let a = restore_physics_from_cache(5, 2, 100, true);
        let b = restore_physics_from_cache(5, 2, 200, true);
        assert_ne!(a.content_hash, b.content_hash);
    }

    // ── Bridge 5: DbRecord → CryptoRestore ──────────────────────────────

    #[test]
    fn test_restore_crypto_fields() {
        let fp = [0xABu8; 32];
        let r = restore_crypto_from_db(fp, 5, 1_700_000_000_000_000_000, false);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.key_fingerprint, fp);
        assert_eq!(r.shard_count, 5);
        assert_eq!(r.created_at_ns, 1_700_000_000_000_000_000);
        assert!(!r.expired);
    }

    #[test]
    fn test_restore_crypto_expired_flag() {
        let fp = [0x00u8; 32];
        let r = restore_crypto_from_db(fp, 3, 0, true);
        assert!(r.expired);
        assert_eq!(r.shard_count, 3);
    }

    #[test]
    fn test_restore_crypto_deterministic() {
        let fp = [0x55u8; 32];
        let a = restore_crypto_from_db(fp, 7, 42_000, false);
        let b = restore_crypto_from_db(fp, 7, 42_000, false);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_restore_crypto_different_fingerprints_differ() {
        let fp_a = [0x11u8; 32];
        let fp_b = [0x22u8; 32];
        let a = restore_crypto_from_db(fp_a, 3, 0, false);
        let b = restore_crypto_from_db(fp_b, 3, 0, false);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
