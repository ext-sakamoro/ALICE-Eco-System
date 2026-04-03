//! Train-ML-Weight bridges — ALICE-Train export layer ↔ DB, Cache, Analytics, Edge, ML
//!
//! 5 bridges connecting the ternary export pipeline (AliceModelMeta, ExportStats,
//! EpochResult, TrainConfig) to the ALICE ecosystem.

use alice_train::{AliceModelMeta, EpochResult, ExportStats, TrainConfig};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: AliceModelMeta → DB (モデルメタデータ永続化) ──────────────

/// AliceModelMeta の DB 永続化レコード。
pub struct AliceModelMetaDbRecord {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// 量子化方式ハッシュ。
    pub quantization_hash: u64,
    /// フォーマットバージョン。
    pub format_version: u32,
    /// 総パラメータ数。
    pub total_params: u64,
    /// 量子化パラメータ数。
    pub quantized_params: u64,
    /// 非量子化パラメータ数。
    pub non_quantized_params: u64,
    /// 学習元ステップ数。
    pub source_step: u32,
    /// 学習元 loss。
    pub source_loss: f32,
}

/// `AliceModelMeta` を DB 永続化レコードに変換。
#[inline]
#[must_use]
pub fn alice_model_meta_to_db(meta: &AliceModelMeta) -> AliceModelMetaDbRecord {
    let quant_hash = fnv1a(meta.quantization.as_bytes());

    let mut buf = [0u8; 40];
    buf[..4].copy_from_slice(&meta.version.to_le_bytes());
    buf[4..12].copy_from_slice(&(meta.total_params as u64).to_le_bytes());
    buf[12..20].copy_from_slice(&(meta.quantized_params as u64).to_le_bytes());
    buf[20..28].copy_from_slice(&(meta.non_quantized_params as u64).to_le_bytes());
    buf[28..36].copy_from_slice(&quant_hash.to_le_bytes());
    buf[36..40].copy_from_slice(&(meta.source_step as u32).to_le_bytes());
    let content_hash = fnv1a(&[&buf[..], &meta.source_loss.to_le_bytes()].concat());

    AliceModelMetaDbRecord {
        content_hash,
        quantization_hash: quant_hash,
        format_version: meta.version,
        total_params: meta.total_params as u64,
        quantized_params: meta.quantized_params as u64,
        non_quantized_params: meta.non_quantized_params as u64,
        source_step: meta.source_step as u32,
        source_loss: meta.source_loss,
    }
}

// ── Bridge 2: ExportStats → Analytics (エクスポート統計メトリクス) ─────────

/// ExportStats の Analytics エントリ。
pub struct ExportStatsAnalyticsEntry {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// 総ファイルサイズ (bytes)。
    pub total_bytes: u64,
    /// Ternary パックセクションサイズ (bytes)。
    pub ternary_bytes: u64,
    /// Embedding セクションサイズ (bytes)。
    pub embed_bytes: u64,
    /// 量子化済みパラメータ数。
    pub quantized_params: u64,
}

/// `ExportStats` を Analytics エントリに変換。
#[inline]
#[must_use]
pub fn export_stats_to_analytics(stats: &ExportStats) -> ExportStatsAnalyticsEntry {
    let mut buf = [0u8; 32];
    buf[..8].copy_from_slice(&(stats.total_bytes as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(stats.ternary_bytes as u64).to_le_bytes());
    buf[16..24].copy_from_slice(&(stats.embed_bytes as u64).to_le_bytes());
    buf[24..32].copy_from_slice(&(stats.quantized_params as u64).to_le_bytes());
    let content_hash = fnv1a(&buf);

    ExportStatsAnalyticsEntry {
        content_hash,
        total_bytes: stats.total_bytes as u64,
        ternary_bytes: stats.ternary_bytes as u64,
        embed_bytes: stats.embed_bytes as u64,
        quantized_params: stats.quantized_params as u64,
    }
}

// ── Bridge 3: EpochResult → Cache (学習進捗キャッシュ) ──────────────────

/// EpochResult のキャッシュエントリ。
pub struct EpochResultCacheEntry {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// エポック番号。
    pub epoch: u32,
    /// 平均損失値。
    pub avg_loss: f32,
    /// キャッシュ TTL (秒)。損失が低いほど長めにキャッシュ。
    pub ttl_secs: u32,
}

/// `EpochResult` をキャッシュエントリに変換。
///
/// TTL はブランチレスで計算: 損失が 1.0 未満なら 3600 秒、以上なら 600 秒。
#[inline]
#[must_use]
pub fn epoch_result_to_cache(result: &EpochResult) -> EpochResultCacheEntry {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&(result.epoch as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&result.avg_loss.to_le_bytes());
    let content_hash = fnv1a(&buf);

    // 損失が低い(収束済み)なら長め、収束前は短め
    let converged = (result.avg_loss < 1.0) as u32;
    let ttl_secs = 600 + converged * 3000;

    EpochResultCacheEntry {
        content_hash,
        epoch: result.epoch as u32,
        avg_loss: result.avg_loss,
        ttl_secs,
    }
}

// ── Bridge 4: TrainConfig → Edge (学習設定のエッジ配信) ─────────────────

/// TrainConfig のエッジ配信レコード。
pub struct TrainConfigEdgeRecord {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// 学習率。
    pub learning_rate: f32,
    /// エポック数。
    pub epochs: u32,
    /// バッチサイズ。
    pub batch_size: u32,
    /// 勾配累積ステップ数。
    pub gradient_accumulation_steps: u32,
    /// ログ出力間隔。
    pub log_interval: u32,
}

/// `TrainConfig` をエッジ配信レコードに変換。
#[inline]
#[must_use]
pub fn train_config_to_edge(config: &TrainConfig) -> TrainConfigEdgeRecord {
    let mut buf = [0u8; 20];
    buf[..4].copy_from_slice(&config.learning_rate.to_le_bytes());
    buf[4..8].copy_from_slice(&(config.epochs as u32).to_le_bytes());
    buf[8..12].copy_from_slice(&(config.batch_size as u32).to_le_bytes());
    buf[12..16].copy_from_slice(&(config.gradient_accumulation_steps as u32).to_le_bytes());
    buf[16..20].copy_from_slice(&(config.log_interval as u32).to_le_bytes());
    let content_hash = fnv1a(&buf);

    TrainConfigEdgeRecord {
        content_hash,
        learning_rate: config.learning_rate,
        epochs: config.epochs as u32,
        batch_size: config.batch_size as u32,
        gradient_accumulation_steps: config.gradient_accumulation_steps as u32,
        log_interval: config.log_interval as u32,
    }
}

// ── Bridge 5: AliceModelMeta → ML (モデル特徴量ベクトル) ────────────────

/// AliceModelMeta の ML 特徴量ベクトル。
pub struct AliceModelMlFeatures {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// 量子化率 (0.0〜1.0)。
    pub quantization_ratio: f32,
    /// レイヤースケール数。
    pub layer_count: u32,
    /// 隠れ層サイズ (hidden_size)。
    pub hidden_size: u32,
    /// 語彙サイズ (vocab_size)。
    pub vocab_size: u32,
    /// tied embeddings フラグ (0/1)。
    pub tied_embeddings: u8,
}

/// `AliceModelMeta` から ML 特徴量を抽出。
#[inline]
#[must_use]
pub fn alice_model_meta_to_ml(meta: &AliceModelMeta) -> AliceModelMlFeatures {
    let quant_ratio = if meta.total_params > 0 {
        meta.quantized_params as f32 / meta.total_params as f32
    } else {
        0.0
    };
    let layer_count = meta.layer_scales.len() as u32;
    let hidden_size = meta.config.hidden_size as u32;
    let vocab_size = meta.config.vocab_size as u32;
    let tied_embeddings = meta.tied_embeddings as u8;

    let mut buf = [0u8; 18];
    buf[..4].copy_from_slice(&quant_ratio.to_le_bytes());
    buf[4..8].copy_from_slice(&layer_count.to_le_bytes());
    buf[8..12].copy_from_slice(&hidden_size.to_le_bytes());
    buf[12..16].copy_from_slice(&vocab_size.to_le_bytes());
    buf[16] = tied_embeddings;
    buf[17] = 0;
    let content_hash = fnv1a(&buf);

    AliceModelMlFeatures {
        content_hash,
        quantization_ratio: quant_ratio,
        layer_count,
        hidden_size,
        vocab_size,
        tied_embeddings,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_train::{AliceModelMeta, ExportStats, LayerScales};
    use alice_train::qwen35::Qwen35Config;

    fn make_qwen35_config() -> Qwen35Config {
        Qwen35Config::qwen35_9b()
    }

    fn make_model_meta(step: usize, loss: f32) -> AliceModelMeta {
        AliceModelMeta {
            version: 1,
            config: make_qwen35_config(),
            quantization: "ternary-1.58bit".to_owned(),
            tied_embeddings: false,
            quantized_params: 8_000_000_000,
            non_quantized_params: 200_000_000,
            total_params: 8_200_000_000,
            source_step: step,
            source_loss: loss,
            layer_scales: Vec::new(),
        }
    }

    fn make_export_stats(meta: AliceModelMeta) -> ExportStats {
        ExportStats {
            total_bytes: 1_500_000_000,
            embed_bytes: 100_000_000,
            ternary_bytes: 1_200_000_000,
            layer_fp32_bytes: 180_000_000,
            lm_head_bytes: 20_000_000,
            quantized_params: 8_000_000_000,
            meta,
        }
    }

    fn make_epoch_result(epoch: usize, loss: f32) -> EpochResult {
        EpochResult::new(epoch, loss)
    }

    // Bridge 1 tests

    #[test]
    fn test_model_meta_to_db_basic() {
        let meta = make_model_meta(5000, 1.23);
        let r = alice_model_meta_to_db(&meta);
        assert_ne!(r.content_hash, 0);
        assert_eq!(r.format_version, 1);
        assert_eq!(r.source_step, 5000);
        assert!((r.source_loss - 1.23).abs() < 1e-5);
        assert_eq!(r.total_params, 8_200_000_000);
    }

    #[test]
    fn test_model_meta_to_db_hash_deterministic() {
        let meta = make_model_meta(1000, 2.5);
        let r1 = alice_model_meta_to_db(&meta);
        let r2 = alice_model_meta_to_db(&meta);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.quantization_hash, r2.quantization_hash);
    }

    #[test]
    fn test_model_meta_to_db_quantization_hash() {
        let meta = make_model_meta(100, 3.0);
        let r = alice_model_meta_to_db(&meta);
        assert_eq!(r.quantization_hash, fnv1a(b"ternary-1.58bit"));
    }

    // Bridge 2 tests

    #[test]
    fn test_export_stats_to_analytics() {
        let meta = make_model_meta(5000, 1.0);
        let stats = make_export_stats(meta);
        let e = export_stats_to_analytics(&stats);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.total_bytes, 1_500_000_000);
        assert_eq!(e.ternary_bytes, 1_200_000_000);
        assert_eq!(e.embed_bytes, 100_000_000);
        assert_eq!(e.quantized_params, 8_000_000_000);
    }

    #[test]
    fn test_export_stats_hash_deterministic() {
        let meta1 = make_model_meta(5000, 1.0);
        let meta2 = make_model_meta(5000, 1.0);
        let stats1 = make_export_stats(meta1);
        let stats2 = make_export_stats(meta2);
        let e1 = export_stats_to_analytics(&stats1);
        let e2 = export_stats_to_analytics(&stats2);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // Bridge 3 tests

    #[test]
    fn test_epoch_result_cache_ttl_high_loss() {
        let result = make_epoch_result(1, 5.0);
        let c = epoch_result_to_cache(&result);
        assert_ne!(c.content_hash, 0);
        assert_eq!(c.epoch, 1);
        assert_eq!(c.ttl_secs, 600); // 損失が高い → 短い TTL
    }

    #[test]
    fn test_epoch_result_cache_ttl_low_loss() {
        let result = make_epoch_result(50, 0.5);
        let c = epoch_result_to_cache(&result);
        assert_eq!(c.ttl_secs, 3600); // 損失が低い → 長い TTL
    }

    #[test]
    fn test_epoch_result_cache_boundary() {
        let result = make_epoch_result(10, 1.0); // 境界値: < 1.0 が条件
        let c = epoch_result_to_cache(&result);
        assert_eq!(c.ttl_secs, 600); // 1.0 は収束未達
    }

    // Bridge 4 tests

    #[test]
    fn test_train_config_to_edge() {
        let config = TrainConfig::new()
            .with_epochs(200)
            .with_learning_rate(0.001)
            .with_gradient_accumulation(4);
        let e = train_config_to_edge(&config);
        assert_ne!(e.content_hash, 0);
        assert_eq!(e.epochs, 200);
        assert!((e.learning_rate - 0.001).abs() < 1e-7);
        assert_eq!(e.gradient_accumulation_steps, 4);
    }

    #[test]
    fn test_train_config_to_edge_hash_deterministic() {
        let config = TrainConfig::new();
        let e1 = train_config_to_edge(&config);
        let e2 = train_config_to_edge(&config);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // Bridge 5 tests

    #[test]
    fn test_model_meta_to_ml_features() {
        let meta = make_model_meta(5000, 1.0);
        let f = alice_model_meta_to_ml(&meta);
        assert_ne!(f.content_hash, 0);
        // 8B / 8.2B ≈ 0.976
        assert!(f.quantization_ratio > 0.9 && f.quantization_ratio <= 1.0);
        assert_eq!(f.tied_embeddings, 0);
    }

    #[test]
    fn test_model_meta_to_ml_zero_params() {
        let mut meta = make_model_meta(0, 0.0);
        meta.total_params = 0;
        meta.quantized_params = 0;
        let f = alice_model_meta_to_ml(&meta);
        assert_eq!(f.quantization_ratio, 0.0);
    }

    #[test]
    fn test_model_meta_to_ml_layer_count() {
        let mut meta = make_model_meta(100, 2.0);
        meta.layer_scales = vec![
            LayerScales { layer_idx: 0, layer_type: "deltanet".to_owned(), scales: Vec::new() },
            LayerScales { layer_idx: 1, layer_type: "attention".to_owned(), scales: Vec::new() },
        ];
        let f = alice_model_meta_to_ml(&meta);
        assert_eq!(f.layer_count, 2);
    }
}
