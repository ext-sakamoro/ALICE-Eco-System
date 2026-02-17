//! ML bridges — ALICE-ML ↔ Physics, SDF, Animation
//!
//! 3 bridges connecting 1.58-bit ternary inference to the ALICE ecosystem.

use alice_ml::{TernaryWeight, ternary_matvec};

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
pub fn ml_sdf_evaluate(weights: &TernaryWeight, points: &[f32]) -> MlSdfField {
    let rows = weights.out_features();
    let point_count = points.len() / 3;
    let mut distances = Vec::with_capacity(point_count);

    for i in 0..point_count {
        let xyz = &points[i * 3..(i * 3 + 3).min(points.len())];
        if xyz.len() < 3 { break; }
        let mut output = vec![0.0f32; rows];
        ternary_matvec(xyz, weights, &mut output);
        // Take first output as distance
        distances.push(if output.is_empty() { 0.0 } else { output[0] });
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
pub fn ml_animation_direction(weights: &TernaryWeight, scene_features: &[f32]) -> MlAnimDirection {
    let rows = weights.out_features();
    let mut output = vec![0.0f32; rows];
    ternary_matvec(scene_features, weights, &mut output);

    // Map outputs to animation parameters
    let dx = if !output.is_empty() { output[0].tanh() } else { 0.0 };
    let dy = if output.len() > 1 { output[1].tanh() } else { 0.0 };
    let dz = if output.len() > 2 { output[2].tanh() } else { 0.0 };
    let mood = if output.len() > 3 { output[3].tanh() } else { 0.0 };
    let cut_prob = if output.len() > 4 { (output[4] * 0.5 + 0.5).clamp(0.0, 1.0) } else { 0.0 };
    let expr = if output.len() > 5 { (output[5].abs() * 8.0) as u8 } else { 0 };

    MlAnimDirection {
        camera_delta: (dx, dy, dz),
        mood,
        cut_probability: cut_prob,
        expression_idx: expr,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_weights(rows: usize, cols: usize) -> TernaryWeight {
        let data: Vec<i8> = (0..rows * cols).map(|i| match i % 3 { 0 => 1, 1 => -1, _ => 0 }).collect();
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
}
