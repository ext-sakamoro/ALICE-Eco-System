//! ML bridges — ALICE-ML ↔ Physics, SDF, Animation, DB, Cache, Analytics
//!
//! 6 bridges connecting 1.58-bit ternary inference to the ALICE ecosystem.

use alice_ml::{ternary_matvec, TernaryWeight};

// ── Bridge 1: ML → Physics (ternary → neural ragdoll controller) ────────

/// Neural physics controller output from ternary inference.
pub struct MlPhysicsControl {
    /// Joint torques (N·m) for ragdoll.
    pub torques: Vec<f32>,
    /// Number of joints controlled.
    pub joint_count: usize,
    /// Inference time (for scheduling).
    pub inference_ops: usize,
}

/// Run ternary inference for ALICE-Physics ragdoll controller.
#[inline]
#[must_use]
pub fn ml_physics_ragdoll(weights: &TernaryWeight, state: &[f32]) -> MlPhysicsControl {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let mut output_buf = vec![0.0f32; rows];
    ternary_matvec(state, weights, &mut output_buf);
    // Apply tanh activation for bounded torques
    let torques: Vec<f32> = output_buf.iter().map(|&x| x.tanh()).collect();
    MlPhysicsControl {
        joint_count: rows,
        inference_ops: rows * cols,
        torques,
    }
}

// ── Bridge 2: ML → SDF (ternary → neural SDF field) ────────────────────

/// Neural SDF evaluation result.
pub struct MlSdfField {
    /// Distance values at query points.
    pub distances: Vec<f32>,
    /// Number of points evaluated.
    pub point_count: usize,
    /// Model parameters (weight count).
    pub param_count: usize,
}

/// Evaluate neural SDF field via ternary inference for ALICE-SDF.
#[inline]
#[must_use]
pub fn ml_sdf_evaluate(weights: &TernaryWeight, points: &[f32]) -> MlSdfField {
    let rows = weights.out_features();
    let point_count = points.len() / 3;
    let mut distances = Vec::with_capacity(point_count);
    // Pre-allocate output buffer once (amortize allocation across all points)
    let mut output = vec![0.0f32; rows];

    for xyz in points.chunks_exact(3).take(point_count) {
        output.fill(0.0); // reset without realloc
        ternary_matvec(xyz, weights, &mut output);
        // Take first output as distance (branchless via get)
        distances.push(*output.first().unwrap_or(&0.0));
    }

    MlSdfField {
        distances,
        point_count,
        param_count: rows,
    }
}

// ── Bridge 3: ML → Animation (inference → scene direction) ──────────────

/// AI-directed animation parameters from ternary inference.
pub struct MlAnimDirection {
    /// Camera movement vector (dx, dy, dz).
    pub camera_delta: (f32, f32, f32),
    /// Scene mood value (-1.0 to 1.0).
    pub mood: f32,
    /// Cut probability (0.0-1.0).
    pub cut_probability: f32,
    /// Character expression index.
    pub expression_idx: u8,
}

/// Run ternary inference for AI-driven anime direction in ALICE-Animation.
#[inline]
#[must_use]
pub fn ml_animation_direction(weights: &TernaryWeight, scene_features: &[f32]) -> MlAnimDirection {
    let rows = weights.out_features();
    let mut output = vec![0.0f32; rows];
    ternary_matvec(scene_features, weights, &mut output);

    // Map outputs to animation parameters (branchless via get+map_or)
    let dx = output.first().map_or(0.0, |x| x.tanh());
    let dy = output.get(1).map_or(0.0, |x| x.tanh());
    let dz = output.get(2).map_or(0.0, |x| x.tanh());
    let mood = output.get(3).map_or(0.0, |x| x.tanh());
    let cut_prob = output
        .get(4)
        .map_or(0.0, |x| (x * 0.5 + 0.5).clamp(0.0, 1.0));
    let expr = output.get(5).map_or(0u8, |x| (x.abs() * 8.0) as u8);

    MlAnimDirection {
        camera_delta: (dx, dy, dz),
        mood,
        cut_probability: cut_prob,
        expression_idx: expr,
    }
}

// ── Bridge 4: ML → DB (model metadata persistence) ──────────────────────

/// Model metadata record for ALICE-DB persistence.
pub struct MlDbRecord {
    /// Model parameter count.
    pub param_count: usize,
    /// Input features.
    pub in_features: usize,
    /// Output features.
    pub out_features: usize,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Sparsity ratio (fraction of zero weights).
    pub sparsity: f32,
}

/// Serialize ML model metadata for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn ml_to_db_record(weights: &TernaryWeight) -> MlDbRecord {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let total = rows * cols;
    let mut hash: u64 = 0xcbf29ce484222325;
    let bytes = (total as u64).to_le_bytes();
    for &b in &bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let bytes2 = (rows as u64).to_le_bytes();
    for &b in &bytes2 {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    MlDbRecord {
        param_count: total,
        in_features: cols,
        out_features: rows,
        content_hash: hash,
        sparsity: 1.0 / 3.0, // ternary: ~33% zeros
    }
}

// ── Bridge 5: ML → Cache (inference result caching) ─────────────────────

/// Cached inference result for ALICE-Cache.
pub struct MlCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Input dimension.
    pub input_dim: usize,
    /// Output dimension.
    pub output_dim: usize,
    /// Inference ops count (for eviction priority).
    pub inference_ops: usize,
}

/// Prepare ML model metadata for ALICE-Cache keying.
#[inline]
#[must_use]
pub fn ml_to_cache_entry(weights: &TernaryWeight) -> MlCacheEntry {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &(rows as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in &(cols as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    MlCacheEntry {
        content_hash: hash,
        input_dim: cols,
        output_dim: rows,
        inference_ops: rows * cols,
    }
}

// ── Bridge 6: ML → Analytics (training/inference metrics) ───────────────

/// ML inference metrics for ALICE-Analytics.
pub struct MlAnalyticsMetrics {
    /// Total multiply-accumulate operations.
    pub mac_ops: usize,
    /// Parameter count.
    pub param_count: usize,
    /// Memory footprint estimate (bytes, ternary = 2 bits/param).
    pub memory_bytes: usize,
    /// Compression vs f32 (32 bits → 2 bits).
    pub compression_ratio: f32,
}

/// Extract inference metrics for ALICE-Analytics monitoring.
#[inline]
#[must_use]
pub fn ml_to_analytics_metrics(weights: &TernaryWeight) -> MlAnalyticsMetrics {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let total = rows * cols;
    let memory_bytes = total.div_ceil(4); // 2 bits per ternary weight
    MlAnalyticsMetrics {
        mac_ops: total,
        param_count: total,
        memory_bytes,
        compression_ratio: (total * 4) as f32 / memory_bytes.max(1) as f32,
    }
}

// ── Bridge 7: ML model → TRT engine descriptor ───────────────────────────

/// TRT engine descriptor derived from an ML model for ALICE-TRT deployment.
pub struct MlTrtDescriptor {
    /// Content hash for deduplication (FNV-1a over layer geometry).
    pub content_hash: u64,
    /// Number of layers inferred from the model's in/out feature dimensions.
    pub layer_count: u32,
    /// Estimated total parameter count.
    pub estimated_params: u64,
    /// Whether FP16 mode is recommended (always true — TRT always benefits from fp16).
    pub requires_fp16: bool,
    /// Engine scheduling priority (1 = high).
    pub engine_priority: u8,
}

/// Build a TRT engine descriptor from an ML ternary weight model.
///
/// `layer_count` is derived from the model's out-feature count (each output
/// feature corresponds to one logical processing layer in the descriptor).
/// `requires_fp16` is always `true` because TRT fp16 mode reduces bandwidth
/// regardless of model size.
#[inline]
#[must_use]
pub fn ml_to_trt_descriptor(weights: &TernaryWeight) -> MlTrtDescriptor {
    let rows = weights.out_features();
    let cols = weights.in_features();
    let estimated_params = (rows * cols) as u64;
    // FNV-1a hash over (rows, cols) pair for stable deduplication key.
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &(rows as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for &b in &(cols as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    MlTrtDescriptor {
        content_hash: hash,
        layer_count: rows as u32,
        estimated_params,
        requires_fp16: true,
        engine_priority: 1,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_weights(rows: usize, cols: usize) -> TernaryWeight {
        let data: Vec<i8> = (0..rows * cols)
            .map(|i| match i % 3 {
                0 => 1,
                1 => -1,
                _ => 0,
            })
            .collect();
        TernaryWeight::from_ternary(&data, rows, cols)
    }

    #[test]
    fn test_ml_physics_ragdoll() {
        let weights = test_weights(6, 12);
        let state = vec![0.1f32; 12];
        let ctrl = ml_physics_ragdoll(&weights, &state);
        assert_eq!(ctrl.joint_count, 6);
        assert_eq!(ctrl.torques.len(), 6);
        for t in &ctrl.torques {
            assert!(*t >= -1.0 && *t <= 1.0);
        }
    }

    #[test]
    fn test_ml_sdf_evaluate() {
        let weights = test_weights(1, 3);
        let points = vec![0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let result = ml_sdf_evaluate(&weights, &points);
        assert_eq!(result.point_count, 3);
        assert_eq!(result.distances.len(), 3);
    }

    #[test]
    fn test_ml_animation_direction() {
        let weights = test_weights(6, 8);
        let features = vec![0.5f32; 8];
        let dir = ml_animation_direction(&weights, &features);
        assert!(dir.mood >= -1.0 && dir.mood <= 1.0);
        assert!(dir.cut_probability >= 0.0 && dir.cut_probability <= 1.0);
    }

    #[test]
    fn test_ml_to_db_record() {
        let weights = test_weights(6, 12);
        let rec = ml_to_db_record(&weights);
        assert_eq!(rec.param_count, 72);
        assert_eq!(rec.in_features, 12);
        assert_eq!(rec.out_features, 6);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_ml_to_cache_entry() {
        let weights = test_weights(4, 8);
        let entry = ml_to_cache_entry(&weights);
        assert_eq!(entry.input_dim, 8);
        assert_eq!(entry.output_dim, 4);
        assert_eq!(entry.inference_ops, 32);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_ml_to_analytics_metrics() {
        let weights = test_weights(6, 12);
        let m = ml_to_analytics_metrics(&weights);
        assert_eq!(m.mac_ops, 72);
        assert!(m.compression_ratio > 1.0);
        assert!(m.memory_bytes < m.param_count);
    }

    #[test]
    fn test_ml_to_trt_descriptor() {
        let weights = test_weights(8, 16);
        let desc = ml_to_trt_descriptor(&weights);
        assert_eq!(desc.layer_count, 8);
        assert_eq!(desc.estimated_params, 128);
        assert!(desc.requires_fp16);
        assert_eq!(desc.engine_priority, 1);
        assert_ne!(desc.content_hash, 0);
        // Same weights → same hash (deterministic).
        let desc2 = ml_to_trt_descriptor(&weights);
        assert_eq!(desc.content_hash, desc2.content_hash);
    }
}
