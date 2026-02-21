//! TRT bridges — ALICE-TRT ↔ DB, Analytics, Cache, Edge, View, Animation
//!
//! 7 bridges connecting GPU ternary inference to the ALICE ecosystem.
//! All structs carry only plain data derived from inference configuration
//! so they are usable without an active GPU device.

#[inline(always)]
fn fnv1a_trt(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: TRT → DB (inference result persistence) ───────────────────

/// GPU inference result record for ALICE-DB persistence.
pub struct TrtDbRecord {
    /// Content hash for deduplication (FNV-1a over model geometry).
    pub content_hash: u64,
    /// Number of output features produced by the final layer.
    pub out_features: usize,
    /// Total parameter count across all layers.
    pub param_count: usize,
    /// Estimated VRAM usage in bytes (2 bits per ternary weight).
    pub vram_bytes: usize,
}

/// Build a DB persistence record from GPU inference layer geometry.
///
/// `layer_shapes` is a slice of `(out, in)` pairs — one entry per layer.
/// Hash is computed with FNV-1a over the encoded shape data.
#[inline]
pub fn trt_to_db_record(layer_shapes: &[(usize, usize)]) -> TrtDbRecord {
    let mut param_count: usize = 0;
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis

    for &(out, inp) in layer_shapes {
        param_count += out * inp;
        // Hash (out, in) pair with FNV-1a.
        for &b in &(out as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &b in &(inp as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    // 2 bits per ternary weight → divide by 4 (packed into bytes).
    let vram_bytes = (param_count + 3) / 4;
    let out_features = layer_shapes.last().map_or(0, |&(out, _)| out);

    TrtDbRecord {
        content_hash: hash,
        out_features,
        param_count,
        vram_bytes,
    }
}

// ── Bridge 2: TRT → Analytics (inference latency / throughput metrics) ──

/// GPU inference performance metrics for ALICE-Analytics monitoring.
pub struct TrtAnalyticsMetrics {
    /// Total multiply-accumulate operations across all layers.
    pub mac_ops: usize,
    /// Total parameter count.
    pub param_count: usize,
    /// Memory footprint in bytes (ternary: 2 bits/weight).
    pub vram_bytes: usize,
    /// Bandwidth compression ratio vs FP16 (16 bits → 2 bits = 8x).
    pub bandwidth_compression: f32,
    /// Estimated throughput multiplier from bandwidth reduction.
    pub throughput_factor: f32,
}

/// Extract performance metrics from GPU inference layer geometry for ALICE-Analytics.
///
/// `layer_shapes` is a slice of `(out, in)` pairs.
#[inline]
pub fn trt_to_analytics_metrics(layer_shapes: &[(usize, usize)]) -> TrtAnalyticsMetrics {
    let mut mac_ops: usize = 0;
    let mut param_count: usize = 0;

    for &(out, inp) in layer_shapes {
        let layer_params = out * inp;
        param_count += layer_params;
        // Each layer: 2 MACs per weight (multiply + accumulate).
        mac_ops += layer_params * 2;
    }

    // Ternary: 2 bits per weight packed into bytes.
    let vram_bytes = (param_count + 3) / 4;
    // FP16 reference: 2 bytes per weight.
    let fp16_bytes = param_count * 2;
    // Reciprocal multiply avoids division in the hot path.
    let rcp_vram = if vram_bytes > 0 { 1.0 / vram_bytes as f32 } else { 0.0 };
    let bandwidth_compression = fp16_bytes as f32 * rcp_vram;
    // Bandwidth reduction translates directly to throughput on memory-bound workloads.
    let throughput_factor = bandwidth_compression;

    TrtAnalyticsMetrics {
        mac_ops,
        param_count,
        vram_bytes,
        bandwidth_compression,
        throughput_factor,
    }
}

// ── Bridge 3: TRT → Cache (model weight caching) ────────────────────────

/// GPU model weight cache entry for ALICE-Cache.
pub struct TrtCacheEntry {
    /// Content hash for cache key lookup.
    pub content_hash: u64,
    /// Number of layers in the cached model.
    pub layer_count: usize,
    /// Total VRAM bytes required to restore the model.
    pub vram_bytes: usize,
    /// Eviction priority score (lower = evict first).
    /// Derived from mac_ops: heavier models are worth keeping longer.
    pub eviction_priority: usize,
}

/// Prepare a GPU model weight cache entry from layer geometry for ALICE-Cache.
///
/// `layer_shapes` is a slice of `(out, in)` pairs.
#[inline]
pub fn trt_to_cache_entry(layer_shapes: &[(usize, usize)]) -> TrtCacheEntry {
    let mut param_count: usize = 0;
    let mut hash: u64 = 0xcbf29ce484222325;

    for &(out, inp) in layer_shapes {
        param_count += out * inp;
        for &b in &(out as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &b in &(inp as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let vram_bytes = (param_count + 3) / 4;
    // Eviction priority = total MACs (heavier model → higher priority to retain).
    let eviction_priority = param_count * 2;

    TrtCacheEntry {
        content_hash: hash,
        layer_count: layer_shapes.len(),
        vram_bytes,
        eviction_priority,
    }
}

// ── Bridge 4: TRT → Edge (edge inference deployment config) ─────────────

/// GPU model configuration compressed for ALICE-Edge deployment.
pub struct TrtEdgeDeployConfig {
    /// Original parameter count (full-precision reference).
    pub original_params: usize,
    /// Parameters retained after compression via `compression_ratio`.
    pub compressed_params: usize,
    /// Whether INT8 quantization is applied (params < 500 K threshold).
    pub is_quantized: bool,
    /// Estimated on-device model size in kilobytes.
    pub estimated_kb: usize,
    /// Number of layers in the deployed model.
    pub layer_count: usize,
}

/// Produce an ALICE-Edge deployment config by applying `compression_ratio` to
/// the full-precision parameter count derived from `layer_shapes`.
///
/// `compression_ratio` must be in (0.0, 1.0].
/// `compressed_params` uses reciprocal multiply — no runtime division.
/// `is_quantized` is set branchlessly via integer comparison.
#[inline]
pub fn trt_to_edge_deploy(layer_shapes: &[(usize, usize)], compression_ratio: f32) -> TrtEdgeDeployConfig {
    let original_params: usize = layer_shapes.iter().map(|&(o, i)| o * i).sum();
    // Reciprocal multiply avoids runtime division.
    let compressed_params = (original_params as f32 * compression_ratio) as usize;
    // Branchless: quantize when model fits in 500 K params.
    let is_quantized = compressed_params < 500_000;
    // 4 bytes per f32 → convert to KB via reciprocal of 1024.
    const RCP_1024: f32 = 1.0 / 1024.0;
    let estimated_kb = (compressed_params as f32 * 4.0 * RCP_1024) as usize;

    TrtEdgeDeployConfig {
        original_params,
        compressed_params,
        is_quantized,
        estimated_kb,
        layer_count: layer_shapes.len(),
    }
}

// ── Bridge 5: TRT → View (inference result visualization config) ─────────

/// Neural upscaling / visualization config derived from TRT output dimensions.
pub struct TrtViewConfig {
    /// Input spatial resolution (width, height) fed to the renderer.
    pub input_resolution: (usize, usize),
    /// Output resolution after neural upscaling.
    pub output_resolution: (usize, usize),
    /// Integer upscale factor applied by ALICE-View.
    pub scale_factor: usize,
    /// Number of feature channels produced by the final inference layer.
    pub feature_channels: usize,
    /// Estimated GPU memory for the output tensor in bytes (f32 per value).
    pub output_tensor_bytes: usize,
}

/// Configure ALICE-View neural upscaling from TRT output geometry.
///
/// `out_features` is the channel count from the last TRT layer.
/// `scale` must be >= 1.
#[inline]
pub fn trt_to_view_config(width: usize, height: usize, out_features: usize, scale: usize) -> TrtViewConfig {
    let out_w = width * scale;
    let out_h = height * scale;
    // f32 = 4 bytes per value in the output tensor.
    let output_tensor_bytes = out_w * out_h * out_features * 4;

    TrtViewConfig {
        input_resolution: (width, height),
        output_resolution: (out_w, out_h),
        scale_factor: scale,
        feature_channels: out_features,
        output_tensor_bytes,
    }
}

// ── Bridge 6: TRT ↔ Animation (AI-driven motion prediction) ─────────────

/// AI motion prediction result produced by a TRT model over animation frames.
///
/// Encodes the model identity and prediction statistics so the Animation layer
/// can consume predicted joint poses without holding a reference to TRT internals.
pub struct TrtAnimationPrediction {
    /// FNV-1a hash over model name bytes (model identity / cache key).
    pub content_hash: u64,
    /// FNV-1a hash over model name + input/output frame counts.
    pub model_hash: u64,
    /// Number of input animation frames fed to the model.
    pub input_frames: usize,
    /// Number of future frames predicted by the model.
    pub predicted_frames: usize,
    /// Number of skeleton joints covered by the prediction.
    pub joint_count: usize,
    /// Model confidence score in [0.0, 1.0] (higher = more certain).
    pub confidence: f32,
    /// GPU inference latency in microseconds for this prediction pass.
    pub latency_us: u64,
}

/// Build a TRT motion prediction descriptor from inference metadata.
///
/// `model_name` identifies the TRT model used for prediction.
/// `confidence` is clamped to [0.0, 1.0] so callers do not need to pre-validate.
#[inline]
pub fn trt_to_animation_prediction(
    model_name: &str,
    input_frames: usize,
    predicted_frames: usize,
    joint_count: usize,
    confidence: f32,
    latency_us: u64,
) -> TrtAnimationPrediction {
    let content_hash = fnv1a_trt(model_name.as_bytes());
    // model_hash mixes name, frame counts, and joint count for a richer identity.
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&(input_frames as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(predicted_frames as u64).to_le_bytes());
    buf[16..24].copy_from_slice(&(joint_count as u64).to_le_bytes());
    let model_hash = fnv1a_trt(&[model_name.as_bytes(), &buf].concat());

    TrtAnimationPrediction {
        content_hash,
        model_hash,
        input_frames,
        predicted_frames,
        joint_count,
        confidence: confidence.clamp(0.0, 1.0),
        latency_us,
    }
}

/// Animation sequence record packaged for TRT training data ingestion.
///
/// Converts a sequence descriptor (frame count, joint count, keyframe positions)
/// into a compact record the TRT training pipeline can use to build motion
/// prediction datasets without access to the full animation asset.
pub struct AnimationTrtTrainingData {
    /// FNV-1a hash of the sequence name bytes (dataset dedup key).
    pub content_hash: u64,
    /// FNV-1a hash of the sequence name combined with frame / joint metadata.
    pub sequence_hash: u64,
    /// Total number of frames in the animation sequence.
    pub frame_count: usize,
    /// Number of skeleton joints recorded per frame.
    pub joint_count: usize,
    /// Total number of keyframe entries across all joints.
    pub total_keyframes: usize,
}

/// Build a TRT training data record from an animation sequence descriptor.
///
/// `sequence_name` labels the dataset entry.
/// `keyframes` is the total number of keyframe entries across all joints in
/// the sequence (typically `frame_count × joint_count` for dense captures,
/// or fewer for sparse keyframed animation).
#[inline]
pub fn animation_to_trt_training_data(
    sequence_name: &str,
    frame_count: usize,
    joint_count: usize,
    keyframes: usize,
) -> AnimationTrtTrainingData {
    let content_hash = fnv1a_trt(sequence_name.as_bytes());
    // sequence_hash mixes the name with numeric metadata for a stable unique key.
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&(frame_count as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(joint_count as u64).to_le_bytes());
    buf[16..24].copy_from_slice(&(keyframes as u64).to_le_bytes());
    let sequence_hash = fnv1a_trt(&[sequence_name.as_bytes(), &buf].concat());

    AnimationTrtTrainingData {
        content_hash,
        sequence_hash,
        frame_count,
        joint_count,
        total_keyframes: keyframes,
    }
}

// ── Bridge 8: TRT inference result → SDF field parameters ───────────────

/// SDF field parameters derived from a TRT inference result for ALICE-SDF.
pub struct TrtSdfField {
    /// Content hash for cache keying (FNV-1a over layer geometry).
    pub content_hash: u64,
    /// Voxel grid resolution for the SDF evaluation pass.
    pub field_resolution: u32,
    /// Iso-surface threshold value for marching-cubes extraction.
    pub iso_value: f32,
    /// Estimated number of active SDF tree nodes given the resolution.
    pub estimated_nodes: usize,
}

/// Build an SDF field descriptor from TRT layer geometry.
///
/// `field_resolution` is derived from the total parameter count: larger models
/// warrant finer SDF grids.  `iso_value` is fixed at 0.0 (zero-level set).
/// `estimated_nodes` approximates the octree node count at the given resolution.
#[inline]
pub fn trt_to_sdf_field(layer_shapes: &[(usize, usize)]) -> TrtSdfField {
    let mut param_count: usize = 0;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &(out, inp) in layer_shapes {
        param_count += out * inp;
        for &b in &(out as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &b in &(inp as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    // Resolution scales with sqrt(param_count), clamped to [16, 512].
    let field_resolution = ((param_count as f64).sqrt() as u32).clamp(16, 512);
    // Estimated octree nodes: resolution^3 / 8 (one node per 2×2×2 cell).
    let res = field_resolution as usize;
    let estimated_nodes = (res * res * res) / 8;
    TrtSdfField {
        content_hash: hash,
        field_resolution,
        iso_value: 0.0,
        estimated_nodes,
    }
}

// ── Bridge 9: TRT inference result → Physics simulation parameters ───────

/// Physics simulation parameters derived from a TRT inference result.
pub struct TrtPhysicsParams {
    /// Content hash for deduplication (FNV-1a over layer geometry).
    pub content_hash: u64,
    /// Number of rigid bodies to simulate (one per output feature).
    pub body_count: u32,
    /// Fixed simulation time step in microseconds.
    pub time_step_us: u64,
    /// Maximum force magnitude in Newtons applied by the inference controller.
    pub force_magnitude: f64,
    /// Whether the simulation runs at real-time rate (time_step_us ≤ 16 667 µs = 60 fps).
    pub is_realtime: bool,
}

/// Build physics simulation parameters from TRT layer geometry.
///
/// `body_count` equals the output feature count of the last layer.
/// `time_step_us` is fixed at 8 333 µs (120 fps physics tick).
/// `is_realtime` is set branchlessly: `time_step_us <= 16_667`.
#[inline]
pub fn trt_to_physics_params(layer_shapes: &[(usize, usize)]) -> TrtPhysicsParams {
    let mut param_count: usize = 0;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &(out, inp) in layer_shapes {
        param_count += out * inp;
        for &b in &(out as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &b in &(inp as u64).to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let body_count = layer_shapes.last().map_or(0, |&(out, _)| out) as u32;
    // 120 Hz physics tick: 1_000_000 µs / 120 = 8_333 µs.
    let time_step_us: u64 = 8_333;
    // Branchless: real-time when time_step_us fits within one 60 fps frame.
    let is_realtime = time_step_us <= 16_667;
    // Force magnitude scales with parameter count to reflect model influence.
    let force_magnitude = 1.0 + param_count as f64 * 0.001;
    TrtPhysicsParams {
        content_hash: hash,
        body_count,
        time_step_us,
        force_magnitude,
        is_realtime,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 3-layer network: 3→64, 64→32, 32→8
    fn test_shapes() -> Vec<(usize, usize)> {
        vec![(64, 3), (32, 64), (8, 32)]
    }

    #[test]
    fn test_trt_to_db_record() {
        let shapes = test_shapes();
        let rec = trt_to_db_record(&shapes);
        // Last layer out = 8.
        assert_eq!(rec.out_features, 8);
        // param_count: 64*3 + 32*64 + 8*32 = 192 + 2048 + 256 = 2496
        assert_eq!(rec.param_count, 2496);
        // vram_bytes: ceil(2496 / 4) = 624
        assert_eq!(rec.vram_bytes, 624);
        // Hash must be non-zero and stable.
        assert_ne!(rec.content_hash, 0);
        // Same shapes → same hash.
        let rec2 = trt_to_db_record(&shapes);
        assert_eq!(rec.content_hash, rec2.content_hash);
    }

    #[test]
    fn test_trt_to_analytics_metrics() {
        let shapes = test_shapes();
        let m = trt_to_analytics_metrics(&shapes);
        assert_eq!(m.param_count, 2496);
        assert_eq!(m.mac_ops, 4992); // 2496 * 2
        assert_eq!(m.vram_bytes, 624);
        // FP16 = 2496 * 2 = 4992 bytes; bandwidth_compression = 4992 / 624 = 8.0.
        assert!((m.bandwidth_compression - 8.0).abs() < 0.01);
        assert!(m.throughput_factor > 1.0);
    }

    #[test]
    fn test_trt_to_cache_entry() {
        let shapes = test_shapes();
        let entry = trt_to_cache_entry(&shapes);
        assert_eq!(entry.layer_count, 3);
        assert_eq!(entry.vram_bytes, 624);
        assert_eq!(entry.eviction_priority, 4992); // 2496 * 2
        assert_ne!(entry.content_hash, 0);
        // Two calls with same shapes must return same hash.
        let entry2 = trt_to_cache_entry(&shapes);
        assert_eq!(entry.content_hash, entry2.content_hash);
    }

    #[test]
    fn test_trt_to_edge_deploy_quantized() {
        // Small model — compressed_params < 500 K → quantized.
        let shapes = test_shapes(); // 2496 params
        let cfg = trt_to_edge_deploy(&shapes, 1.0);
        assert_eq!(cfg.original_params, 2496);
        assert_eq!(cfg.compressed_params, 2496);
        assert!(cfg.is_quantized);
        assert_eq!(cfg.layer_count, 3);
        // estimated_kb: 2496 * 4 / 1024 = 9
        assert_eq!(cfg.estimated_kb, 9);
    }

    #[test]
    fn test_trt_to_edge_deploy_not_quantized() {
        // Large model: 1000×1000 = 1 M params, compress to 60 % → 600 K >= 500 K → not quantized.
        let shapes = vec![(1000, 1000)];
        let cfg = trt_to_edge_deploy(&shapes, 0.6);
        assert_eq!(cfg.original_params, 1_000_000);
        assert_eq!(cfg.compressed_params, 600_000);
        assert!(!cfg.is_quantized);
        // estimated_kb: 600000 * 4 / 1024 = 2343
        assert_eq!(cfg.estimated_kb, 2343);
    }

    #[test]
    fn test_trt_to_view_config() {
        let cfg = trt_to_view_config(960, 540, 8, 2);
        assert_eq!(cfg.input_resolution, (960, 540));
        assert_eq!(cfg.output_resolution, (1920, 1080));
        assert_eq!(cfg.scale_factor, 2);
        assert_eq!(cfg.feature_channels, 8);
        // output_tensor_bytes: 1920 * 1080 * 8 * 4 = 66 355 200
        assert_eq!(cfg.output_tensor_bytes, 1920 * 1080 * 8 * 4);
    }

    // ── Bridge 6 tests ────────────────────────────────────────────────────

    #[test]
    fn test_trt_to_animation_prediction_basic() {
        let pred = trt_to_animation_prediction("motion_v2", 30, 10, 24, 0.92, 3500);
        assert_ne!(pred.content_hash, 0);
        assert_ne!(pred.model_hash, 0);
        assert_eq!(pred.input_frames, 30);
        assert_eq!(pred.predicted_frames, 10);
        assert_eq!(pred.joint_count, 24);
        assert!((pred.confidence - 0.92).abs() < 1e-5);
        assert_eq!(pred.latency_us, 3500);
    }

    #[test]
    fn test_trt_to_animation_prediction_confidence_clamped() {
        // Confidence values outside [0, 1] must be clamped.
        let pred_high = trt_to_animation_prediction("m", 1, 1, 1, 1.5, 0);
        let pred_low  = trt_to_animation_prediction("m", 1, 1, 1, -0.3, 0);
        assert!((pred_high.confidence - 1.0).abs() < f32::EPSILON,
            "confidence > 1.0 must clamp to 1.0");
        assert!((pred_low.confidence).abs() < f32::EPSILON,
            "negative confidence must clamp to 0.0");
    }

    #[test]
    fn test_trt_to_animation_prediction_different_models_differ() {
        let a = trt_to_animation_prediction("model_a", 30, 10, 24, 0.9, 1000);
        let b = trt_to_animation_prediction("model_b", 30, 10, 24, 0.9, 1000);
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(a.model_hash, b.model_hash);
    }

    #[test]
    fn test_trt_to_animation_prediction_same_model_same_hash() {
        let a = trt_to_animation_prediction("stable_model", 60, 20, 32, 0.85, 5000);
        let b = trt_to_animation_prediction("stable_model", 60, 20, 32, 0.85, 5000);
        assert_eq!(a.content_hash, b.content_hash, "identical inputs must yield identical hashes");
        assert_eq!(a.model_hash, b.model_hash);
    }

    // ── Bridge 7 tests ────────────────────────────────────────────────────

    #[test]
    fn test_animation_to_trt_training_data_basic() {
        let data = animation_to_trt_training_data("run_cycle", 120, 24, 2880);
        assert_ne!(data.content_hash, 0);
        assert_ne!(data.sequence_hash, 0);
        assert_eq!(data.frame_count, 120);
        assert_eq!(data.joint_count, 24);
        assert_eq!(data.total_keyframes, 2880);
    }

    #[test]
    fn test_animation_to_trt_training_data_different_names_differ() {
        let a = animation_to_trt_training_data("walk", 60, 24, 1440);
        let b = animation_to_trt_training_data("sprint", 60, 24, 1440);
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(a.sequence_hash, b.sequence_hash);
    }

    #[test]
    fn test_animation_to_trt_training_data_same_name_same_hash() {
        let a = animation_to_trt_training_data("idle_loop", 30, 16, 480);
        let b = animation_to_trt_training_data("idle_loop", 30, 16, 480);
        assert_eq!(a.content_hash, b.content_hash, "identical inputs must be deterministic");
        assert_eq!(a.sequence_hash, b.sequence_hash);
    }

    #[test]
    fn test_animation_to_trt_training_data_different_frame_counts_differ() {
        // Same name but different frame counts → different sequence_hash.
        let a = animation_to_trt_training_data("clip", 60, 24, 1440);
        let b = animation_to_trt_training_data("clip", 120, 24, 2880);
        // content_hash is name-only, so it should be equal.
        assert_eq!(a.content_hash, b.content_hash,
            "content_hash is derived from name only");
        // sequence_hash mixes in frame metadata, so it must differ.
        assert_ne!(a.sequence_hash, b.sequence_hash,
            "sequence_hash must change when frame_count changes");
    }

    // ── Bridge 8 tests ────────────────────────────────────────────────────

    #[test]
    fn test_trt_to_sdf_field_basic() {
        let shapes = test_shapes(); // 2496 params
        let field = trt_to_sdf_field(&shapes);
        assert_ne!(field.content_hash, 0);
        // sqrt(2496) ≈ 49 → clamped to 49, within [16, 512].
        assert!(field.field_resolution >= 16 && field.field_resolution <= 512);
        assert!((field.iso_value).abs() < f32::EPSILON);
        assert!(field.estimated_nodes > 0);
        // Deterministic: same shapes → same hash.
        let field2 = trt_to_sdf_field(&shapes);
        assert_eq!(field.content_hash, field2.content_hash);
    }

    #[test]
    fn test_trt_to_sdf_field_resolution_floor() {
        // Single tiny layer: 1×1 = 1 param → sqrt = 1.0 → clamped to 16.
        let shapes = vec![(1, 1)];
        let field = trt_to_sdf_field(&shapes);
        assert_eq!(field.field_resolution, 16);
    }

    #[test]
    fn test_trt_to_sdf_field_resolution_ceiling() {
        // Very large layer: 1_000_000 params → sqrt = 1000 → clamped to 512.
        let shapes = vec![(1000, 1000)];
        let field = trt_to_sdf_field(&shapes);
        assert_eq!(field.field_resolution, 512);
    }

    // ── Bridge 9 tests ────────────────────────────────────────────────────

    #[test]
    fn test_trt_to_physics_params_basic() {
        let shapes = test_shapes(); // last layer out = 8
        let params = trt_to_physics_params(&shapes);
        assert_ne!(params.content_hash, 0);
        assert_eq!(params.body_count, 8);
        assert_eq!(params.time_step_us, 8_333);
        assert!(params.is_realtime); // 8333 <= 16667
        assert!(params.force_magnitude > 1.0);
    }

    #[test]
    fn test_trt_to_physics_params_deterministic() {
        let shapes = test_shapes();
        let a = trt_to_physics_params(&shapes);
        let b = trt_to_physics_params(&shapes);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.body_count, b.body_count);
    }

    #[test]
    fn test_trt_to_physics_params_empty_shapes() {
        let params = trt_to_physics_params(&[]);
        assert_eq!(params.body_count, 0);
        assert!(params.is_realtime);
    }
}
