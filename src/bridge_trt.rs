//! TRT bridges — ALICE-TRT ↔ DB, Analytics, Cache, Edge, View
//!
//! 5 bridges connecting GPU ternary inference to the ALICE ecosystem.
//! All structs carry only plain data derived from inference configuration
//! so they are usable without an active GPU device.

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
}
