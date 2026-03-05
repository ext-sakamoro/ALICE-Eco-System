//! Bridge bridges — ALICE-Bridge ↔ Analytics, DB, Edge, Cache
//!
//! 5 bridges connecting the universal hardware bridge to the ALICE ecosystem.
//! Device actions, mappings, and safety events → telemetry, storage, edge control.

use alice_bridge::{ActuatorType, DeviceMapping, SafetyLimits};
use alice_bridge::bridge::BridgeAction;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: BridgeAction → Analytics (デバイスアクション計測) ───────

/// デバイスアクションのAnalyticsイベント。
pub struct BridgeActionAnalyticsEvent {
    /// コンテンツハッシュ（position + duration + timestamp）。
    pub content_hash: u64,
    /// 出力位置 [0.0, 1.0]。
    pub position: f64,
    /// アクション持続時間（ミリ秒）。
    pub duration_ms: u32,
    /// タイムスタンプ（秒）。
    pub timestamp: f64,
    /// 高強度フラグ（position > 0.8）。
    pub high_intensity: bool,
}

/// BridgeAction を Analytics イベントに変換。
#[inline]
#[must_use]
pub fn bridge_action_to_analytics(action: &BridgeAction) -> BridgeActionAnalyticsEvent {
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&action.position.to_bits().to_le_bytes());
    key[8..12].copy_from_slice(&action.duration_ms.to_le_bytes());
    key[12..20].copy_from_slice(&action.timestamp.to_bits().to_le_bytes());

    BridgeActionAnalyticsEvent {
        content_hash: fnv1a(&key),
        position: action.position,
        duration_ms: action.duration_ms,
        timestamp: action.timestamp,
        high_intensity: action.position > 0.8,
    }
}

// ── Bridge 2: DeviceMapping → DB (デバイス設定永続化) ─────────────────

/// デバイスマッピングのDB永続化レコード。
pub struct BridgeMappingDbRecord {
    /// コンテンツハッシュ（device_id + group + source_filter）。
    pub content_hash: u64,
    /// デバイスID。
    pub device_id: String,
    /// ラベル。
    pub label: String,
    /// スケール係数。
    pub scale: f64,
    /// オフセット。
    pub offset: f64,
    /// 反転フラグ。
    pub invert: bool,
    /// 遅延（ミリ秒）。
    pub delay_ms: u32,
    /// 入力ソースフィルター。
    pub source_filter: String,
    /// グループ名。
    pub group: String,
}

/// DeviceMapping を DB レコードに変換。
#[inline]
#[must_use]
pub fn bridge_mapping_to_db(mapping: &DeviceMapping) -> BridgeMappingDbRecord {
    let mut key_data = Vec::with_capacity(mapping.device_id.len() + mapping.group.len() + mapping.source_filter.len());
    key_data.extend_from_slice(mapping.device_id.as_bytes());
    key_data.extend_from_slice(mapping.group.as_bytes());
    key_data.extend_from_slice(mapping.source_filter.as_bytes());

    BridgeMappingDbRecord {
        content_hash: fnv1a(&key_data),
        device_id: mapping.device_id.clone(),
        label: mapping.label.clone(),
        scale: mapping.scale,
        offset: mapping.offset,
        invert: mapping.invert,
        delay_ms: mapping.delay_ms,
        source_filter: mapping.source_filter.clone(),
        group: mapping.group.clone(),
    }
}

// ── Bridge 3: SafetyLimits → Edge (安全制限テレメトリ) ───────────────

/// 安全制限のEdge通知レコード。
pub struct BridgeSafetyEdgeReport {
    /// コンテンツハッシュ（actuator_type + max_intensity + ramp_rate）。
    pub content_hash: u64,
    /// アクチュエータ種別（0=Vibrate..9=Custom）。
    pub actuator_type: u8,
    /// 最大強度 [0.0, 1.0]。
    pub max_intensity: f64,
    /// ランプレート（/秒）。
    pub ramp_rate: f64,
    /// クールダウン（ミリ秒）。
    pub cooldown_ms: u64,
    /// 自動停止時間（ミリ秒、0=無効）。
    pub auto_shutoff_ms: u64,
    /// 安全クリティカルフラグ。
    pub safety_critical: bool,
}

/// ActuatorType の u8 コード変換。
fn actuator_type_to_u8(atype: ActuatorType) -> u8 {
    match atype {
        ActuatorType::Vibrate => 0,
        ActuatorType::Rotate => 1,
        ActuatorType::Oscillate => 2,
        ActuatorType::Constrict => 3,
        ActuatorType::Inflate => 4,
        ActuatorType::Heat => 5,
        ActuatorType::Electrostimulate => 6,
        ActuatorType::Linear => 7,
        ActuatorType::Position => 8,
        ActuatorType::Custom => 9,
    }
}

/// SafetyLimits + ActuatorType を Edge レポートに変換。
#[inline]
#[must_use]
pub fn bridge_safety_to_edge(limits: &SafetyLimits, atype: ActuatorType) -> BridgeSafetyEdgeReport {
    let type_byte = actuator_type_to_u8(atype);
    let mut key = [0u8; 25];
    key[0] = type_byte;
    key[1..9].copy_from_slice(&limits.max_intensity.to_bits().to_le_bytes());
    key[9..17].copy_from_slice(&limits.ramp_rate.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&limits.cooldown_ms.to_le_bytes());

    BridgeSafetyEdgeReport {
        content_hash: fnv1a(&key),
        actuator_type: type_byte,
        max_intensity: limits.max_intensity,
        ramp_rate: limits.ramp_rate,
        cooldown_ms: limits.cooldown_ms,
        auto_shutoff_ms: limits.auto_shutoff_ms,
        safety_critical: atype.is_safety_critical(),
    }
}

// ── Bridge 4: BridgeAction → Cache (最新アクションキャッシュ) ─────────

/// 最新デバイスアクションのCacheエントリ。
pub struct BridgeActionCacheEntry {
    /// コンテンツハッシュ（device_id + position）。
    pub content_hash: u64,
    /// デバイスID。
    pub device_id: String,
    /// 出力位置 [0.0, 1.0]。
    pub position: f64,
    /// アクション持続時間（ミリ秒）。
    pub duration_ms: u32,
    /// タイムスタンプ（秒）。
    pub timestamp: f64,
    /// Cache TTL（秒）。高強度アクションは短いTTL。
    pub ttl_secs: u32,
}

/// BridgeAction を Cache エントリに変換。
#[inline]
#[must_use]
pub fn bridge_action_to_cache(action: &BridgeAction, device_id: &str) -> BridgeActionCacheEntry {
    let mut key_data = Vec::with_capacity(device_id.len() + 8);
    key_data.extend_from_slice(device_id.as_bytes());
    key_data.extend_from_slice(&action.position.to_bits().to_le_bytes());

    // Branchless TTL: 高強度時は短いTTL（30秒）、通常は60秒
    let high_intensity = (action.position > 0.8) as u32;
    let ttl_secs = 60 - high_intensity * 30;

    BridgeActionCacheEntry {
        content_hash: fnv1a(&key_data),
        device_id: device_id.to_string(),
        position: action.position,
        duration_ms: action.duration_ms,
        timestamp: action.timestamp,
        ttl_secs,
    }
}

// ── Bridge 5: DeviceMapping → Analytics (マッピング統計) ─────────────

/// デバイスマッピングのAnalytics統計イベント。
pub struct BridgeMappingAnalyticsEvent {
    /// コンテンツハッシュ（device_id + scale + offset + invert）。
    pub content_hash: u64,
    /// デバイスID。
    pub device_id: String,
    /// グループ名。
    pub group: String,
    /// スケール係数。
    pub scale: f64,
    /// オフセット。
    pub offset: f64,
    /// 反転フラグ。
    pub invert: bool,
    /// カスタムマッピングフラグ（デフォルトと異なるか）。
    pub is_custom: bool,
}

/// DeviceMapping を Analytics 統計イベントに変換。
#[inline]
#[must_use]
pub fn bridge_mapping_to_analytics(mapping: &DeviceMapping) -> BridgeMappingAnalyticsEvent {
    let mut key = [0u8; 25];
    let id_hash = fnv1a(mapping.device_id.as_bytes());
    key[0..8].copy_from_slice(&id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&mapping.scale.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&mapping.offset.to_bits().to_le_bytes());
    key[24] = mapping.invert as u8;

    // デフォルトと異なる設定があればカスタムとみなす
    let is_custom = (mapping.scale - 1.0).abs() > 1e-6
        || mapping.offset.abs() > 1e-6
        || mapping.invert
        || mapping.delay_ms > 0;

    BridgeMappingAnalyticsEvent {
        content_hash: fnv1a(&key),
        device_id: mapping.device_id.clone(),
        group: mapping.group.clone(),
        scale: mapping.scale,
        offset: mapping.offset,
        invert: mapping.invert,
        is_custom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_action(position: f64, duration_ms: u32, timestamp: f64) -> BridgeAction {
        BridgeAction {
            position,
            duration_ms,
            timestamp,
        }
    }

    fn make_mapping(device_id: &str, scale: f64, offset: f64, invert: bool) -> DeviceMapping {
        DeviceMapping {
            device_id: device_id.into(),
            label: format!("label_{device_id}"),
            scale,
            offset,
            invert,
            delay_ms: 0,
            source_filter: "all".into(),
            group: "default".into(),
        }
    }

    // ── Bridge 1 テスト ──

    #[test]
    fn action_analytics_hash_nonzero() {
        let action = make_action(0.5, 50, 1.0);
        let event = bridge_action_to_analytics(&action);
        assert_ne!(event.content_hash, 0);
        assert!((event.position - 0.5).abs() < 1e-6);
        assert_eq!(event.duration_ms, 50);
    }

    #[test]
    fn action_analytics_high_intensity() {
        let high = make_action(0.9, 50, 1.0);
        let low = make_action(0.5, 50, 1.0);
        assert!(bridge_action_to_analytics(&high).high_intensity);
        assert!(!bridge_action_to_analytics(&low).high_intensity);
    }

    #[test]
    fn action_analytics_deterministic() {
        let action = make_action(0.7, 33, 2.0);
        let e1 = bridge_action_to_analytics(&action);
        let e2 = bridge_action_to_analytics(&action);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 2 テスト ──

    #[test]
    fn mapping_db_fields() {
        let mapping = make_mapping("dev:0", 0.5, 0.1, true);
        let record = bridge_mapping_to_db(&mapping);
        assert_ne!(record.content_hash, 0);
        assert_eq!(record.device_id, "dev:0");
        assert!((record.scale - 0.5).abs() < 1e-6);
        assert!(record.invert);
    }

    #[test]
    fn mapping_db_different_ids() {
        let m1 = make_mapping("dev:0", 1.0, 0.0, false);
        let m2 = make_mapping("dev:1", 1.0, 0.0, false);
        let r1 = bridge_mapping_to_db(&m1);
        let r2 = bridge_mapping_to_db(&m2);
        assert_ne!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 3 テスト ──

    #[test]
    fn safety_edge_vibrate() {
        let limits = SafetyLimits {
            max_intensity: 1.0,
            ramp_rate: 10.0,
            cooldown_ms: 0,
            auto_shutoff_ms: 0,
        };
        let report = bridge_safety_to_edge(&limits, ActuatorType::Vibrate);
        assert_ne!(report.content_hash, 0);
        assert_eq!(report.actuator_type, 0);
        assert!(!report.safety_critical);
    }

    #[test]
    fn safety_edge_heat_critical() {
        let limits = SafetyLimits {
            max_intensity: 0.7,
            ramp_rate: 0.5,
            cooldown_ms: 1000,
            auto_shutoff_ms: 300_000,
        };
        let report = bridge_safety_to_edge(&limits, ActuatorType::Heat);
        assert!(report.safety_critical);
        assert_eq!(report.actuator_type, 5);
        assert!((report.max_intensity - 0.7).abs() < 1e-6);
    }

    // ── Bridge 4 テスト ──

    #[test]
    fn action_cache_normal_ttl() {
        let action = make_action(0.5, 50, 1.0);
        let entry = bridge_action_to_cache(&action, "dev:0");
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.device_id, "dev:0");
    }

    #[test]
    fn action_cache_high_intensity_ttl() {
        let action = make_action(0.9, 50, 1.0);
        let entry = bridge_action_to_cache(&action, "dev:0");
        assert_eq!(entry.ttl_secs, 30);
    }

    // ── Bridge 5 テスト ──

    #[test]
    fn mapping_analytics_default() {
        let mapping = make_mapping("dev:0", 1.0, 0.0, false);
        let event = bridge_mapping_to_analytics(&mapping);
        assert_ne!(event.content_hash, 0);
        assert!(!event.is_custom);
    }

    #[test]
    fn mapping_analytics_custom() {
        let mapping = make_mapping("dev:0", 0.5, 0.1, true);
        let event = bridge_mapping_to_analytics(&mapping);
        assert!(event.is_custom);
        assert!(event.invert);
    }

    #[test]
    fn fnv1a_deterministic() {
        let h1 = fnv1a(b"alice_bridge");
        let h2 = fnv1a(b"alice_bridge");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
    }

    #[test]
    fn fnv1a_different_inputs() {
        let h1 = fnv1a(b"bridge_a");
        let h2 = fnv1a(b"bridge_b");
        assert_ne!(h1, h2);
    }
}
