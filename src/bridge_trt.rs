//! TRT bridges — ALICE-TRT ↔ SDF, Physics, View, Kinematics, Edge
//!
//! 5 bridges connecting GPU ternary inference to the ALICE ecosystem.

use alice_kinematics::{ArmChain, Intent};

// ── Bridge 1: TRT → SDF (GPU inference → neural SDF evaluation) ─────────

/// GPU-accelerated neural SDF evaluation config.
pub struct TrtSdfConfig {
    /// Input dimensions (3 for xyz).
    pub input_dims: usize,
    /// Hidden layer sizes.
    pub hidden_layers: Vec<usize>,
    /// Output dimensions (1 for distance).
    pub output_dims: usize,
    /// Total parameter count.
    pub param_count: usize,
    /// Activation function.
    pub activation: &'static str,
}

/// Configure GPU neural SDF evaluation pipeline for ALICE-SDF.
#[inline]
pub fn trt_sdf_config(hidden_sizes: &[usize]) -> TrtSdfConfig {
    let input_dims = 3;
    let output_dims = 1;
    let mut param_count = 0;
    let mut prev = input_dims;
    for &h in hidden_sizes {
        param_count += prev * h;
        prev = h;
    }
    param_count += prev * output_dims;
    TrtSdfConfig {
        input_dims,
        hidden_layers: hidden_sizes.to_vec(),
        output_dims,
        param_count,
        activation: "ReLU",
    }
}

// ── Bridge 2: TRT → Physics (GPU inference → physics control policy) ────

/// GPU physics control policy config.
pub struct TrtPhysicsPolicy {
    /// State space dimensions.
    pub state_dims: usize,
    /// Action space dimensions.
    pub action_dims: usize,
    /// Network architecture (layer sizes).
    pub architecture: Vec<usize>,
    /// Total trainable parameters.
    pub param_count: usize,
    /// Estimated FLOPS per inference.
    pub flops_per_inference: usize,
}

/// Configure GPU physics control policy for ALICE-Physics.
#[inline]
pub fn trt_physics_policy(state_dims: usize, action_dims: usize, hidden: &[usize]) -> TrtPhysicsPolicy {
    let mut param_count = 0;
    let mut flops = 0;
    let mut prev = state_dims;
    for &h in hidden {
        param_count += prev * h + h; // weights + bias
        flops += prev * h * 2;
        prev = h;
    }
    param_count += prev * action_dims + action_dims;
    flops += prev * action_dims * 2;
    TrtPhysicsPolicy {
        state_dims,
        action_dims,
        architecture: hidden.to_vec(),
        param_count,
        flops_per_inference: flops,
    }
}

// ── Bridge 3: TRT → View (GPU tensor → neural upscaling) ───────────────

/// Neural upscaling configuration for ALICE-View.
pub struct TrtViewUpscale {
    /// Input resolution (width, height).
    pub input_resolution: (usize, usize),
    /// Output resolution (width, height).
    pub output_resolution: (usize, usize),
    /// Upscale factor.
    pub scale_factor: usize,
    /// Network parameter count.
    pub param_count: usize,
    /// Quality tier.
    pub quality: TrtUpscaleQuality,
}

/// Upscale quality tier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrtUpscaleQuality {
    Performance,
    Balanced,
    Quality,
    UltraQuality,
}

/// Configure neural upscaling pipeline for ALICE-View.
#[inline]
pub fn trt_view_upscale(width: usize, height: usize, scale: usize, quality: TrtUpscaleQuality) -> TrtViewUpscale {
    let param_count = match quality {
        TrtUpscaleQuality::Performance => 50_000,
        TrtUpscaleQuality::Balanced => 200_000,
        TrtUpscaleQuality::Quality => 800_000,
        TrtUpscaleQuality::UltraQuality => 2_000_000,
    };
    TrtViewUpscale {
        input_resolution: (width, height),
        output_resolution: (width * scale, height * scale),
        scale_factor: scale,
        param_count,
        quality,
    }
}

// ── Bridge 4: TRT → Kinematics (GPU-accelerated IK solver config) ────────

/// GPU-accelerated inverse kinematics solver configuration for ALICE-Kinematics.
pub struct TrtIkConfig {
    /// Number of IK problems solved in parallel per GPU batch.
    pub batch_size: usize,
    /// Degrees of freedom of the arm chain (number of joints).
    pub chain_dof: usize,
    /// Network architecture: hidden layer sizes.
    pub architecture: Vec<usize>,
    /// Total trainable parameters in the IK network.
    pub param_count: usize,
    /// Estimated throughput: IK solves per second at this batch size.
    pub solves_per_second: usize,
}

/// Build a GPU IK solver config for ALICE-Kinematics from arm chain DOF and
/// network architecture.
///
/// `solves_per_second` is estimated branchlessly as
/// `batch_size * 1_000_000 / param_count` (reciprocal multiply avoids division).
#[inline]
pub fn trt_ik_config(batch_size: usize, chain_dof: usize, hidden: &[usize]) -> TrtIkConfig {
    // Input: chain_dof (joint angles) → Output: chain_dof (target angles)
    let mut param_count = 0usize;
    let mut prev = chain_dof;
    for &h in hidden {
        param_count += prev * h + h; // weights + bias
        prev = h;
    }
    param_count += prev * chain_dof + chain_dof;

    // Throughput estimate: batch * 1M / params — branchless reciprocal multiply.
    let rcp_params = if param_count > 0 { 1.0 / param_count as f64 } else { 0.0 };
    let solves_per_second = (batch_size as f64 * 1_000_000.0 * rcp_params) as usize;

    TrtIkConfig {
        batch_size,
        chain_dof,
        architecture: hidden.to_vec(),
        param_count,
        solves_per_second,
    }
}

// ── Bridge 5: TRT → Edge (GPU edge inference config) ─────────────────────

/// GPU model configuration compressed for ALICE-Edge deployment.
pub struct TrtEdgeInferenceConfig {
    /// Original (uncompressed) parameter count.
    pub original_params: usize,
    /// Parameters remaining after compression.
    pub compressed_params: usize,
    /// Whether INT8 quantization is applied (param count < 500 K threshold).
    pub is_quantized: bool,
    /// Estimated model size in kilobytes after compression.
    pub estimated_kb: usize,
}

/// Produce an edge-deployment config by applying `compression_ratio` to a
/// full-precision param count.
///
/// `compressed_params` uses reciprocal multiply (no division).
/// `is_quantized` is set branchlessly via integer comparison cast.
#[inline]
pub fn trt_edge_config(param_count: usize, compression_ratio: f32) -> TrtEdgeInferenceConfig {
    // Reciprocal multiply — avoids runtime division.
    let compressed_params = (param_count as f32 * compression_ratio) as usize;
    // Branchless bool: quantize when compressed_params < 500_000.
    let is_quantized = compressed_params < 500_000;
    // 4 bytes per f32 param → KB via reciprocal of 1024.
    const RCP_1024: f32 = 1.0 / 1024.0;
    let estimated_kb = (compressed_params as f32 * 4.0 * RCP_1024) as usize;
    TrtEdgeInferenceConfig {
        original_params: param_count,
        compressed_params,
        is_quantized,
        estimated_kb,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trt_sdf_config() {
        let config = trt_sdf_config(&[64, 64, 32]);
        assert_eq!(config.input_dims, 3);
        assert_eq!(config.output_dims, 1);
        assert!(config.param_count > 0);
        assert_eq!(config.hidden_layers.len(), 3);
    }

    #[test]
    fn test_trt_physics_policy() {
        let policy = trt_physics_policy(12, 6, &[128, 64]);
        assert_eq!(policy.state_dims, 12);
        assert_eq!(policy.action_dims, 6);
        assert!(policy.param_count > 0);
        assert!(policy.flops_per_inference > 0);
    }

    #[test]
    fn test_trt_view_upscale() {
        let up = trt_view_upscale(960, 540, 2, TrtUpscaleQuality::Quality);
        assert_eq!(up.output_resolution, (1920, 1080));
        assert_eq!(up.scale_factor, 2);
        assert_eq!(up.quality, TrtUpscaleQuality::Quality);
        assert_eq!(up.param_count, 800_000);
    }

    #[test]
    fn test_trt_ik_config() {
        // 6-DOF arm, hidden [128, 64]
        let cfg = trt_ik_config(32, 6, &[128, 64]);
        assert_eq!(cfg.batch_size, 32);
        assert_eq!(cfg.chain_dof, 6);
        assert_eq!(cfg.architecture, vec![128, 64]);
        // param_count: (6*128+128) + (128*64+64) + (64*6+6)
        //            = 896 + 8256 + 390 = 9542
        assert_eq!(cfg.param_count, 9542);
        // solves_per_second must be > 0
        assert!(cfg.solves_per_second > 0);
    }

    #[test]
    fn test_trt_edge_config() {
        // 2 000 000 params, compress to 10 %
        let cfg = trt_edge_config(2_000_000, 0.1);
        assert_eq!(cfg.original_params, 2_000_000);
        assert_eq!(cfg.compressed_params, 200_000);
        // 200 000 < 500 000 → quantized
        assert!(cfg.is_quantized);
        // 200 000 * 4 / 1024 = 781 KB
        assert_eq!(cfg.estimated_kb, 781);

        // Large model: 10 000 000 params, compress to 20 %
        let big = trt_edge_config(10_000_000, 0.2);
        assert_eq!(big.compressed_params, 2_000_000);
        assert!(!big.is_quantized); // 2 000 000 >= 500 000
    }
}
