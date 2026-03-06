//! Train bridges — ALICE-Train ↔ DB, Cache, Analytics, Edge, ML
//!
//! 5 bridges connecting backpropagation training framework to the ALICE ecosystem.

use alice_train::{EpochResult, TrainConfig};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Train → DB (学習結果の永続化) ────

/// 学習エポック結果の DB 永続化レコード。
pub struct TrainEpochDbRecord {
    /// Content hash of the epoch result (FNV-1a).
    pub content_hash: u64,
    /// エポック番号。
    pub epoch: u32,
    /// 平均損失値。
    pub avg_loss: f32,
}

/// `EpochResult` を DB 永続化レコードに変換。
#[inline]
#[must_use]
pub fn train_epoch_to_db(result: &EpochResult) -> TrainEpochDbRecord {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&(result.epoch as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&result.avg_loss.to_le_bytes());
    TrainEpochDbRecord {
        content_hash: fnv1a(&buf),
        epoch: result.epoch as u32,
        avg_loss: result.avg_loss,
    }
}

// ── Bridge 2: Train → Cache (学習設定キャッシュ) ────

/// 学習設定のキャッシュエントリ。
pub struct TrainConfigCacheEntry {
    /// Content hash of the config (FNV-1a).
    pub content_hash: u64,
    /// 学習率。
    pub learning_rate: f32,
    /// エポック数。
    pub epochs: u32,
    /// バッチサイズ。
    pub batch_size: u32,
    /// キャッシュ TTL (秒)。
    pub ttl_secs: u32,
}

/// `TrainConfig` をキャッシュエントリに変換。
#[inline]
#[must_use]
pub fn train_config_to_cache(config: &TrainConfig) -> TrainConfigCacheEntry {
    let mut buf = [0u8; 12];
    buf[..4].copy_from_slice(&config.learning_rate.to_le_bytes());
    buf[4..8].copy_from_slice(&(config.epochs as u32).to_le_bytes());
    buf[8..12].copy_from_slice(&(config.batch_size as u32).to_le_bytes());
    // 学習中は頻繁に参照 → 短い TTL
    let ttl_secs = 300;
    TrainConfigCacheEntry {
        content_hash: fnv1a(&buf),
        learning_rate: config.learning_rate,
        epochs: config.epochs as u32,
        batch_size: config.batch_size as u32,
        ttl_secs,
    }
}

// ── Bridge 3: Train → Analytics (学習メトリクス) ────

/// 学習メトリクスの Analytics エントリ。
pub struct TrainAnalyticsMetric {
    /// Content hash of the metric (FNV-1a).
    pub content_hash: u64,
    /// エポック番号。
    pub epoch: u32,
    /// 平均損失値。
    pub avg_loss: f32,
    /// 損失改善率 (前エポック比)。
    pub loss_improvement: f32,
}

/// `EpochResult` を Analytics メトリクスに変換。
#[inline]
#[must_use]
pub fn train_epoch_to_analytics(
    result: &EpochResult,
    prev_loss: Option<f32>,
) -> TrainAnalyticsMetric {
    let loss_improvement = prev_loss.map_or(0.0, |prev| prev - result.avg_loss);
    let mut buf = [0u8; 12];
    buf[..4].copy_from_slice(&(result.epoch as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&result.avg_loss.to_le_bytes());
    buf[8..12].copy_from_slice(&loss_improvement.to_le_bytes());
    TrainAnalyticsMetric {
        content_hash: fnv1a(&buf),
        epoch: result.epoch as u32,
        avg_loss: result.avg_loss,
        loss_improvement,
    }
}

// ── Bridge 4: Train → Edge (エッジデバイス学習テレメトリ) ────

/// エッジデバイス向け学習テレメトリパケット。
pub struct TrainEdgeTelemetry {
    /// Content hash of the telemetry (FNV-1a).
    pub content_hash: u64,
    /// エポック番号。
    pub epoch: u32,
    /// 平均損失値。
    pub avg_loss: f32,
    /// 学習完了フラグ。
    pub completed: bool,
}

/// `EpochResult` をエッジテレメトリに変換。
#[inline]
#[must_use]
pub fn train_epoch_to_edge(result: &EpochResult, total_epochs: u32) -> TrainEdgeTelemetry {
    let completed = result.epoch as u32 >= total_epochs.saturating_sub(1);
    let mut buf = [0u8; 9];
    buf[..4].copy_from_slice(&(result.epoch as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&result.avg_loss.to_le_bytes());
    buf[8] = u8::from(completed);
    TrainEdgeTelemetry {
        content_hash: fnv1a(&buf),
        epoch: result.epoch as u32,
        avg_loss: result.avg_loss,
        completed,
    }
}

// ── Bridge 5: Train → ML (推論エンジンへのモデル配信メタデータ) ────

/// 学習済みモデルの ML 配信メタデータ。
pub struct TrainModelDelivery {
    /// Content hash of the model metadata (FNV-1a).
    pub content_hash: u64,
    /// 最終エポックの平均損失。
    pub final_loss: f32,
    /// 総エポック数。
    pub total_epochs: u32,
    /// モデル配信準備完了フラグ (loss < 1.0)。
    pub ready: bool,
}

/// 学習完了時の最終結果を ML 配信メタデータに変換。
#[inline]
#[must_use]
pub fn train_result_to_ml_delivery(result: &EpochResult) -> TrainModelDelivery {
    let ready = result.avg_loss < 1.0;
    let mut buf = [0u8; 9];
    buf[..4].copy_from_slice(&result.avg_loss.to_le_bytes());
    buf[4..8].copy_from_slice(&(result.epoch as u32).to_le_bytes());
    buf[8] = u8::from(ready);
    TrainModelDelivery {
        content_hash: fnv1a(&buf),
        final_loss: result.avg_loss,
        total_epochs: result.epoch as u32 + 1,
        ready,
    }
}

// ============================================================================
// テスト
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn train_epoch_to_db_basic() {
        let result = EpochResult::new(0, 0.5);
        let record = train_epoch_to_db(&result);
        assert_ne!(record.content_hash, 0);
        assert_eq!(record.epoch, 0);
        assert!((record.avg_loss - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn train_epoch_to_db_hash_determinism() {
        let result = EpochResult::new(3, 0.25);
        let r1 = train_epoch_to_db(&result);
        let r2 = train_epoch_to_db(&result);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn train_config_to_cache_basic() {
        let config = TrainConfig {
            learning_rate: 0.001,
            epochs: 10,
            batch_size: 32,
            log_interval: 1,
        };
        let entry = train_config_to_cache(&config);
        assert_ne!(entry.content_hash, 0);
        assert!((entry.learning_rate - 0.001).abs() < f32::EPSILON);
        assert_eq!(entry.epochs, 10);
        assert_eq!(entry.batch_size, 32);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn train_config_to_cache_hash_determinism() {
        let config = TrainConfig {
            learning_rate: 0.001,
            epochs: 10,
            batch_size: 32,
            log_interval: 1,
        };
        let e1 = train_config_to_cache(&config);
        let e2 = train_config_to_cache(&config);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn train_epoch_to_analytics_no_prev() {
        let result = EpochResult::new(5, 0.3);
        let metric = train_epoch_to_analytics(&result, None);
        assert_ne!(metric.content_hash, 0);
        assert_eq!(metric.epoch, 5);
        assert!((metric.loss_improvement - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn train_epoch_to_analytics_with_prev() {
        let result = EpochResult::new(5, 0.3);
        let metric = train_epoch_to_analytics(&result, Some(0.5));
        assert!((metric.loss_improvement - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn train_epoch_to_edge_not_completed() {
        let result = EpochResult::new(3, 0.5);
        let telemetry = train_epoch_to_edge(&result, 10);
        assert!(!telemetry.completed);
        assert_eq!(telemetry.epoch, 3);
    }

    #[test]
    fn train_epoch_to_edge_completed() {
        let result = EpochResult::new(9, 0.1);
        let telemetry = train_epoch_to_edge(&result, 10);
        assert!(telemetry.completed);
    }

    #[test]
    fn train_result_to_ml_delivery_ready() {
        let result = EpochResult::new(9, 0.05);
        let delivery = train_result_to_ml_delivery(&result);
        assert!(delivery.ready);
        assert_eq!(delivery.total_epochs, 10);
    }

    #[test]
    fn train_result_to_ml_delivery_not_ready() {
        let result = EpochResult::new(9, 2.5);
        let delivery = train_result_to_ml_delivery(&result);
        assert!(!delivery.ready);
    }

    #[test]
    fn train_result_to_ml_delivery_hash_determinism() {
        let result = EpochResult::new(9, 0.05);
        let d1 = train_result_to_ml_delivery(&result);
        let d2 = train_result_to_ml_delivery(&result);
        assert_eq!(d1.content_hash, d2.content_hash);
    }
}
