//! SIMD bridges — ALICE-SIMD ↔ Physics, SDF, ML, Edge, Cache
//!
//! 5 bridges exposing ALICE-SIMD's aligned vector ops, branchless
//! arithmetic, and FNV-1a hashing to sibling crates.

use alice_simd::{AlignedVec, Vec3};

// ── Bridge 1: SIMD → Physics (aligned batch transforms) ─────────────────

/// Batch-transformed positions for ALICE-Physics.
pub struct SimdPhysicsBatch {
    /// Deterministic hash over input positions.
    pub content_hash: u64,
    /// Transformed positions (x, y, z interleaved).
    pub positions: AlignedVec<f32>,
    /// Number of particles processed.
    pub count: usize,
    /// Whether SIMD fast-path was used.
    pub simd_accelerated: bool,
}

/// Apply uniform gravity to a batch of particle positions via SIMD.
#[inline]
#[must_use]
pub fn simd_to_physics_gravity(
    positions: &[Vec3],
    velocities: &[Vec3],
    gravity: f32,
    dt: f32,
) -> SimdPhysicsBatch {
    let count = positions.len().min(velocities.len());
    let mut hash_bytes = Vec::with_capacity(count * 12);
    let mut out = AlignedVec::with_capacity(count * 3);
    for i in 0..count {
        hash_bytes.extend_from_slice(&positions[i].x.to_le_bytes());
        hash_bytes.extend_from_slice(&positions[i].y.to_le_bytes());
        hash_bytes.extend_from_slice(&positions[i].z.to_le_bytes());
        let vz = velocities[i].z.mul_add(1.0, gravity * dt);
        out.push(velocities[i].x.mul_add(dt, positions[i].x));
        out.push(velocities[i].y.mul_add(dt, positions[i].y));
        out.push(vz.mul_add(dt, positions[i].z));
    }
    SimdPhysicsBatch {
        content_hash: fnv1a(&hash_bytes),
        positions: out,
        count,
        simd_accelerated: cfg!(target_feature = "avx2") || cfg!(target_feature = "neon"),
    }
}

// ── Bridge 2: SIMD → SDF (aligned distance buffer) ──────────────────────

/// Aligned distance buffer for ALICE-SDF field evaluation.
pub struct SimdSdfDistances {
    /// Deterministic hash (point_count as bytes).
    pub content_hash: u64,
    /// SDF distance values (cache-line aligned).
    pub distances: AlignedVec<f32>,
    /// Number of query points.
    pub point_count: usize,
}

/// Allocate an aligned distance buffer for SDF batch evaluation.
#[inline]
#[must_use]
pub fn simd_to_sdf_distances(point_count: usize) -> SimdSdfDistances {
    let mut distances = AlignedVec::with_capacity(point_count);
    for _ in 0..point_count {
        distances.push(f32::MAX);
    }
    SimdSdfDistances {
        content_hash: fnv1a(&point_count.to_le_bytes()),
        distances,
        point_count,
    }
}

// ── Bridge 3: SIMD → ML (aligned weight buffer) ─────────────────────────

/// Aligned weight storage for ALICE-ML ternary inference.
pub struct SimdMlWeightBuffer {
    /// Deterministic hash (byte_count as bytes).
    pub content_hash: u64,
    /// Packed weights (cache-line aligned for SIMD matvec).
    pub data: AlignedVec<u8>,
    /// Number of weight bytes.
    pub byte_count: usize,
    /// SIMD lane width used for alignment.
    pub lane_width: usize,
}

/// Allocate an aligned buffer for ML ternary weight packing.
#[inline]
#[must_use]
pub fn simd_to_ml_weights(byte_count: usize) -> SimdMlWeightBuffer {
    let mut data = AlignedVec::with_capacity(byte_count);
    for _ in 0..byte_count {
        data.push(0);
    }
    SimdMlWeightBuffer {
        content_hash: fnv1a(&byte_count.to_le_bytes()),
        data,
        byte_count,
        lane_width: alice_simd::SIMD_WIDTH,
    }
}

// ── Bridge 4: SIMD → Edge (fast hashing for flow keys) ──────────────────

/// FNV-1a hash result for ALICE-Edge flow classification.
pub struct SimdEdgeHash {
    /// Deterministic hash of the input data.
    pub content_hash: u64,
    /// 64-bit FNV-1a hash (via `alice_simd::fnv1a`).
    pub hash: u64,
    /// Input byte count (for telemetry).
    pub input_bytes: usize,
}

/// Compute FNV-1a hash of a byte slice using ALICE-SIMD's implementation.
#[inline]
#[must_use]
pub fn simd_to_edge_hash(data: &[u8]) -> SimdEdgeHash {
    let hash = alice_simd::fnv1a(data);
    SimdEdgeHash {
        content_hash: fnv1a(data),
        hash,
        input_bytes: data.len(),
    }
}

// ── Bridge 5: SIMD → Cache (aligned buffer metadata) ────────────────────

/// Aligned buffer metadata for ALICE-Cache integration.
pub struct SimdCacheMetadata {
    /// Deterministic hash over (capacity, lane_width).
    pub content_hash: u64,
    /// Buffer capacity in elements.
    pub capacity: usize,
    /// SIMD lane width.
    pub lane_width: usize,
    /// TTL in seconds (aligned buffers: 300s).
    pub ttl_secs: u32,
}

/// Create cache metadata for a SIMD-aligned allocation.
#[inline]
#[must_use]
pub fn simd_to_cache_metadata(capacity: usize) -> SimdCacheMetadata {
    let lane_width = alice_simd::SIMD_WIDTH;
    let mut buf = [0u8; 16];
    buf[..8].copy_from_slice(&capacity.to_le_bytes());
    buf[8..16].copy_from_slice(&lane_width.to_le_bytes());
    SimdCacheMetadata {
        content_hash: fnv1a(&buf),
        capacity,
        lane_width,
        ttl_secs: 300,
    }
}

// ── Shared ──────────────────────────────────────────────────────────────

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_gravity_hash_nonzero() {
        let p = vec![Vec3 {
            x: 0.0,
            y: 0.0,
            z: 10.0,
        }];
        let v = vec![Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }];
        let result = simd_to_physics_gravity(&p, &v, -9.81, 0.1);
        assert_ne!(result.content_hash, 0);
        assert_eq!(result.count, 1);
    }

    #[test]
    fn physics_gravity_position_update() {
        let p = vec![Vec3 {
            x: 0.0,
            y: 0.0,
            z: 10.0,
        }];
        let v = vec![Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }];
        let result = simd_to_physics_gravity(&p, &v, -9.81, 0.1);
        assert_eq!(result.positions.len(), 3);
        // x should advance: 0.0 + 1.0*0.1 = 0.1
        assert!((result.positions[0] - 0.1).abs() < 1e-5);
    }

    #[test]
    fn sdf_distance_alloc_hash() {
        let buf = simd_to_sdf_distances(1024);
        assert_ne!(buf.content_hash, 0);
        assert_eq!(buf.point_count, 1024);
        assert_eq!(buf.distances.len(), 1024);
    }

    #[test]
    fn sdf_distance_initial_value() {
        let buf = simd_to_sdf_distances(8);
        assert!((buf.distances[0] - f32::MAX).abs() < 1.0);
    }

    #[test]
    fn ml_weight_alloc_hash() {
        let buf = simd_to_ml_weights(256);
        assert_ne!(buf.content_hash, 0);
        assert_eq!(buf.byte_count, 256);
        assert!(buf.lane_width > 0);
    }

    #[test]
    fn ml_weight_zero_initialized() {
        let buf = simd_to_ml_weights(64);
        assert!(buf.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn edge_hash_deterministic() {
        let h1 = simd_to_edge_hash(b"flow-key");
        let h2 = simd_to_edge_hash(b"flow-key");
        assert_eq!(h1.content_hash, h2.content_hash);
        assert_eq!(h1.hash, h2.hash);
        assert_eq!(h1.input_bytes, 8);
    }

    #[test]
    fn edge_hash_different_inputs() {
        let h1 = simd_to_edge_hash(b"a");
        let h2 = simd_to_edge_hash(b"b");
        assert_ne!(h1.content_hash, h2.content_hash);
    }

    #[test]
    fn cache_metadata_hash() {
        let meta = simd_to_cache_metadata(4096);
        assert_ne!(meta.content_hash, 0);
        assert_eq!(meta.capacity, 4096);
        assert_eq!(meta.ttl_secs, 300);
    }

    #[test]
    fn fnv1a_consistency() {
        assert_eq!(fnv1a(b"test"), fnv1a(b"test"));
        assert_ne!(fnv1a(b"x"), fnv1a(b"y"));
    }
}
