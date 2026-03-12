//! Train QAT bridges — ALICE-Train QAT ↔ LLM, Edge, ML, Analytics, Monitor
//!
//! 5 bridges connecting quantization-aware training to inference, edge deployment,
//! and observability pipelines.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Train-QAT → LLM (量子化モデル配信メタデータ) ──────────────

/// QAT 完了モデルの LLM 推論エンジン向けメタデータ。
pub struct QatLlmDelivery {
    /// Content hash over the delivery descriptor.
    pub content_hash: u64,
    /// 量子化ビット数 (0=ternary, 1=INT4, 2=INT8).
    pub quant_bits: u8,
    /// 最終学習 epoch の平均損失。
    pub final_loss: f32,
    /// 量子化前後のコサイン類似度。
    pub cosine_similarity: f32,
    /// 量子化前後の MAE。
    pub quantization_mae: f32,
    /// Scale factor。
    pub scale: f32,
    /// GGUF 出力推奨フォーマット (0=Q4_K, 1=Q6_K, 2=Q8_0, 3=F16).
    pub recommended_gguf: u8,
}

/// QAT 結果を LLM 推論エンジン向けに変換。
///
/// `recommended_gguf`: cosine_similarity >= 0.99 → Q4_K, >= 0.97 → Q6_K, >= 0.95 → Q8_0, else F16
#[inline]
#[must_use]
pub fn qat_to_llm_delivery(
    quant_bits: u8,
    final_loss: f32,
    cosine_similarity: f32,
    quantization_mae: f32,
    scale: f32,
) -> QatLlmDelivery {
    let mut buf = [0u8; 17];
    buf[0] = quant_bits;
    buf[1..5].copy_from_slice(&final_loss.to_bits().to_le_bytes());
    buf[5..9].copy_from_slice(&cosine_similarity.to_bits().to_le_bytes());
    buf[9..13].copy_from_slice(&quantization_mae.to_bits().to_le_bytes());
    buf[13..17].copy_from_slice(&scale.to_bits().to_le_bytes());
    // Branchless GGUF recommendation
    let r0 = (cosine_similarity >= 0.99) as u8; // Q4_K
    let r1 = (cosine_similarity >= 0.97) as u8; // Q6_K
    let r2 = (cosine_similarity >= 0.95) as u8; // Q8_0
    // 3 - r0 - r1 - r2: 3=F16, 2=Q8_0, 1=Q6_K, 0=Q4_K
    let recommended_gguf = 3 - r0 - r1 - r2;
    QatLlmDelivery {
        content_hash: fnv1a(&buf),
        quant_bits,
        final_loss,
        cosine_similarity,
        quantization_mae,
        scale,
        recommended_gguf,
    }
}

// ── Bridge 2: Train-QAT → Edge (エッジデプロイ適合性判定) ────────────────

/// QAT モデルのエッジデプロイ適合性。
pub struct QatEdgeFitness {
    /// Content hash over the fitness descriptor.
    pub content_hash: u64,
    /// 量子化ビット数。
    pub quant_bits: u8,
    /// 量子化後の推定モデルサイズ (bytes)。
    pub estimated_model_bytes: u64,
    /// デバイスメモリに収まるか。
    pub fits_in_memory: bool,
    /// 推定 tokens/sec (bandwidth-limited)。
    pub estimated_tps: f32,
    /// 量子化品質スコア (cosine_similarity × (1 - MAE))。
    pub quality_score: f32,
}

/// QAT 結果のエッジデプロイ適合性を判定。
///
/// `estimated_model_bytes`: original_bytes × (quant_bits / 32)
#[inline]
#[must_use]
pub fn qat_to_edge_fitness(
    quant_bits: u8,
    original_model_bytes: u64,
    device_memory_bytes: u64,
    device_bandwidth_gbps: f32,
    cosine_similarity: f32,
    quantization_mae: f32,
) -> QatEdgeFitness {
    let mut buf = [0u8; 22];
    buf[0] = quant_bits;
    buf[1..9].copy_from_slice(&original_model_bytes.to_le_bytes());
    buf[9..17].copy_from_slice(&device_memory_bytes.to_le_bytes());
    buf[17..21].copy_from_slice(&device_bandwidth_gbps.to_bits().to_le_bytes());
    buf[21] = (cosine_similarity * 100.0) as u8;
    // Effective bits for size estimation
    let effective_bits: f64 = match quant_bits {
        0 => 1.58, // ternary
        1 => 4.0,  // INT4
        2 => 8.0,  // INT8
        _ => 16.0, // F16
    };
    let estimated_model_bytes = (original_model_bytes as f64 * effective_bits / 32.0) as u64;
    let fits_in_memory = estimated_model_bytes < device_memory_bytes;
    let secs_per_token = estimated_model_bytes as f64 / (device_bandwidth_gbps as f64 * 1e9);
    let estimated_tps = if secs_per_token > 0.0 {
        (1.0 / secs_per_token) as f32
    } else {
        0.0
    };
    let quality_score = cosine_similarity * (1.0 - quantization_mae.min(1.0));
    QatEdgeFitness {
        content_hash: fnv1a(&buf),
        quant_bits,
        estimated_model_bytes,
        fits_in_memory,
        estimated_tps,
        quality_score,
    }
}

// ── Bridge 3: Train-QAT → ML (ternary 量子化パラメータ同期) ─────────────

/// QAT と ML 推論エンジンの量子化パラメータ同期。
pub struct QatMlSync {
    /// Content hash over the sync descriptor.
    pub content_hash: u64,
    /// レイヤー数。
    pub num_layers: u32,
    /// 合計パラメータ数 (millions)。
    pub total_params_m: f32,
    /// 平均 sparsity (ゼロ重みの割合)。
    pub avg_sparsity: f32,
    /// 平均 effective bits per weight。
    pub avg_effective_bits: f32,
    /// 推論準備完了（cosine_sim >= 0.95 かつ loss < 1.0）。
    pub inference_ready: bool,
}

/// QAT 結果を ML 推論エンジンに同期。
#[inline]
#[must_use]
pub fn qat_to_ml_sync(
    num_layers: u32,
    total_params_m: f32,
    avg_sparsity: f32,
    avg_effective_bits: f32,
    cosine_similarity: f32,
    final_loss: f32,
) -> QatMlSync {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&num_layers.to_le_bytes());
    buf[4..8].copy_from_slice(&total_params_m.to_bits().to_le_bytes());
    buf[8..12].copy_from_slice(&avg_sparsity.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&avg_effective_bits.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&cosine_similarity.to_bits().to_le_bytes());
    let inference_ready = cosine_similarity >= 0.95 && final_loss < 1.0;
    QatMlSync {
        content_hash: fnv1a(&buf),
        num_layers,
        total_params_m,
        avg_sparsity,
        avg_effective_bits,
        inference_ready,
    }
}

// ── Bridge 4: Train-QAT → Analytics (QAT 進捗メトリクス) ────────────────

/// QAT 学習進捗の Analytics メトリクス。
pub struct QatAnalyticsMetric {
    /// Content hash over the metric.
    pub content_hash: u64,
    /// エポック番号。
    pub epoch: u32,
    /// 平均損失。
    pub avg_loss: f32,
    /// 量子化 MAE。
    pub quant_mae: f32,
    /// コサイン類似度。
    pub cosine_sim: f32,
    /// Temperature (annealing 進捗)。
    pub temperature: f32,
}

/// QAT epoch 結果を Analytics メトリクスに変換。
#[inline]
#[must_use]
pub fn qat_to_analytics(
    epoch: u32,
    avg_loss: f32,
    quant_mae: f32,
    cosine_sim: f32,
    temperature: f32,
) -> QatAnalyticsMetric {
    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&epoch.to_le_bytes());
    buf[4..8].copy_from_slice(&avg_loss.to_bits().to_le_bytes());
    buf[8..12].copy_from_slice(&quant_mae.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&cosine_sim.to_bits().to_le_bytes());
    buf[16..20].copy_from_slice(&temperature.to_bits().to_le_bytes());
    QatAnalyticsMetric {
        content_hash: fnv1a(&buf),
        epoch,
        avg_loss,
        quant_mae,
        cosine_sim,
        temperature,
    }
}

// ── Bridge 5: Train-QAT → Monitor (QAT ジョブヘルスチェック) ────────────

/// QAT 学習ジョブのヘルスチェック。
pub struct QatMonitorHealth {
    /// Content hash over the health snapshot.
    pub content_hash: u64,
    /// 現在のエポック。
    pub current_epoch: u32,
    /// 合計エポック数。
    pub total_epochs: u32,
    /// 進捗率 (0.0–1.0)。
    pub progress: f32,
    /// 損失が収束しているか（直近 5 epoch で改善 < 1%）。
    pub converged: bool,
    /// ジョブが健全か（loss が finite かつ NaN でない）。
    pub is_healthy: bool,
}

/// QAT ジョブのヘルスチェックを生成。
#[inline]
#[must_use]
pub fn qat_to_monitor_health(
    current_epoch: u32,
    total_epochs: u32,
    current_loss: f32,
    prev_loss: f32,
) -> QatMonitorHealth {
    let mut buf = [0u8; 16];
    buf[0..4].copy_from_slice(&current_epoch.to_le_bytes());
    buf[4..8].copy_from_slice(&total_epochs.to_le_bytes());
    buf[8..12].copy_from_slice(&current_loss.to_bits().to_le_bytes());
    buf[12..16].copy_from_slice(&prev_loss.to_bits().to_le_bytes());
    let progress = if total_epochs > 0 {
        (current_epoch + 1) as f32 / total_epochs as f32
    } else {
        0.0
    };
    let improvement_ratio = if prev_loss.abs() > 1e-10 {
        (prev_loss - current_loss).abs() / prev_loss.abs()
    } else {
        1.0
    };
    let converged = improvement_ratio < 0.01 && current_epoch > 4;
    let is_healthy = current_loss.is_finite();
    QatMonitorHealth {
        content_hash: fnv1a(&buf),
        current_epoch,
        total_epochs,
        progress,
        converged,
        is_healthy,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qat_llm_delivery_q4k() {
        let d = qat_to_llm_delivery(0, 0.05, 0.995, 0.02, 0.75);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.recommended_gguf, 0); // Q4_K (cosine >= 0.99)
        assert!((d.scale - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_qat_llm_delivery_q6k() {
        let d = qat_to_llm_delivery(1, 0.1, 0.975, 0.05, 0.5);
        assert_eq!(d.recommended_gguf, 1); // Q6_K (0.97 <= cosine < 0.99)
    }

    #[test]
    fn test_qat_llm_delivery_f16() {
        let d = qat_to_llm_delivery(2, 0.5, 0.90, 0.15, 0.3);
        assert_eq!(d.recommended_gguf, 3); // F16 (cosine < 0.95)
    }

    #[test]
    fn test_qat_llm_delivery_hash_determinism() {
        let a = qat_to_llm_delivery(0, 0.05, 0.995, 0.02, 0.75);
        let b = qat_to_llm_delivery(0, 0.05, 0.995, 0.02, 0.75);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_qat_edge_fitness_ternary_fits() {
        // 8B model (16GB FP32) → ternary (1.58 bit) ≈ 790MB
        let f = qat_to_edge_fitness(0, 16_000_000_000, 8_000_000_000, 34.0, 0.99, 0.02);
        assert_ne!(f.content_hash, 0);
        assert!(f.fits_in_memory);
        assert!(f.estimated_tps > 0.0);
        assert!(f.quality_score > 0.95);
    }

    #[test]
    fn test_qat_edge_fitness_no_fit() {
        let f = qat_to_edge_fitness(2, 16_000_000_000, 2_000_000_000, 10.0, 0.95, 0.05);
        assert!(!f.fits_in_memory); // INT8: 16GB * 8/32 = 4GB > 2GB
    }

    #[test]
    fn test_qat_ml_sync_ready() {
        let s = qat_to_ml_sync(32, 7000.0, 0.35, 1.58, 0.99, 0.05);
        assert_ne!(s.content_hash, 0);
        assert!(s.inference_ready);
        assert_eq!(s.num_layers, 32);
    }

    #[test]
    fn test_qat_ml_sync_not_ready() {
        let s = qat_to_ml_sync(16, 1000.0, 0.20, 1.58, 0.90, 2.0);
        assert!(!s.inference_ready); // cosine < 0.95
    }

    #[test]
    fn test_qat_analytics_metric() {
        let m = qat_to_analytics(5, 0.15, 0.03, 0.98, 0.85);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.epoch, 5);
        assert!((m.temperature - 0.85).abs() < 1e-6);
    }

    #[test]
    fn test_qat_monitor_healthy() {
        let h = qat_to_monitor_health(5, 100, 0.15, 0.20);
        assert_ne!(h.content_hash, 0);
        assert!(h.is_healthy);
        assert!(!h.converged); // epoch 5, improvement = 25% > 1%
        assert!((h.progress - 0.06).abs() < 0.01);
    }

    #[test]
    fn test_qat_monitor_converged() {
        let h = qat_to_monitor_health(50, 100, 0.100, 0.101);
        assert!(h.converged); // improvement = 0.99% < 1%, epoch > 4
    }

    #[test]
    fn test_qat_monitor_unhealthy() {
        let h = qat_to_monitor_health(10, 100, f32::NAN, 0.5);
        assert!(!h.is_healthy);
    }
}
