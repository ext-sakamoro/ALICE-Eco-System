//! TRT bridges — ALICE-TRT ↔ SDF, Physics, View
//!
//! 3 bridges connecting GPU ternary inference to the ALICE ecosystem.

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
}
