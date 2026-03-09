//! Agri bridges — ALICE-Agri ↔ DB, Cache, Analytics, ML, Notify
//!
//! 5 bridges connecting agricultural sensor data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Agri → DB (crop data persistence) ──────────────────────────

/// Crop data record for ALICE-DB persistence.
pub struct AgriDbRecord {
    /// Content hash over field + timestamp.
    pub content_hash: u64,
    /// Field area in square metres (fixed-point, cm² precision: value * 100).
    pub field_area_cm2: u64,
    /// Soil moisture percentage (0–100, fixed-point × 100).
    pub moisture_pct_x100: u32,
    /// Ambient temperature in millidegrees Celsius.
    pub temperature_mdeg: i32,
    /// FNV-1a hash of the growth stage label.
    pub growth_stage_hash: u64,
    /// Estimated yield in grams per square metre (fixed-point × 100).
    pub yield_estimate_gpm2_x100: u32,
    /// Observation timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
}

/// Serialize a crop sensor reading for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn agri_to_db_record(
    field_area_cm2: u64,
    moisture_pct_x100: u32,
    temperature_mdeg: i32,
    growth_stage: &[u8],
    yield_estimate_gpm2_x100: u32,
    timestamp_ns: u64,
) -> AgriDbRecord {
    let growth_stage_hash = fnv1a(growth_stage);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&field_area_cm2.to_le_bytes());
    key[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    key[16..24].copy_from_slice(&growth_stage_hash.to_le_bytes());
    AgriDbRecord {
        content_hash: fnv1a(&key),
        field_area_cm2,
        moisture_pct_x100,
        temperature_mdeg,
        growth_stage_hash,
        yield_estimate_gpm2_x100,
        timestamp_ns,
    }
}

// ── Bridge 2: Agri → Cache (weather data) ────────────────────────────────

/// Weather cache entry for ALICE-Cache.
pub struct AgriCacheEntry {
    /// Content hash over field + weather snapshot.
    pub content_hash: u64,
    /// Ambient temperature in millidegrees Celsius.
    pub temperature_mdeg: i32,
    /// Relative humidity percentage (fixed-point × 100).
    pub humidity_pct_x100: u32,
    /// Wind speed in millimetres per second.
    pub wind_speed_mm_s: u32,
    /// Cache TTL in seconds (shorter when frost risk is detected).
    pub ttl_secs: u32,
    /// Forecast valid-until timestamp in nanoseconds.
    pub valid_until_ns: u64,
}

/// Build a weather cache entry for ALICE-Cache.
///
/// TTL is branchlessly reduced to 300 s when frost risk is present
/// (temperature below 0 °C, i.e. `temperature_mdeg < 0`).
#[inline]
#[must_use]
pub fn agri_to_cache_entry(
    temperature_mdeg: i32,
    humidity_pct_x100: u32,
    wind_speed_mm_s: u32,
    valid_until_ns: u64,
) -> AgriCacheEntry {
    // Branchless frost-risk TTL: 3600 s normal, 300 s on frost risk.
    let frost_risk = (temperature_mdeg < 0) as u32;
    let ttl_secs = 3_600_u32 - frost_risk * 3_300_u32;
    let mut key = [0u8; 16];
    key[0..4].copy_from_slice(&temperature_mdeg.to_le_bytes());
    key[4..8].copy_from_slice(&humidity_pct_x100.to_le_bytes());
    key[8..16].copy_from_slice(&valid_until_ns.to_le_bytes());
    AgriCacheEntry {
        content_hash: fnv1a(&key),
        temperature_mdeg,
        humidity_pct_x100,
        wind_speed_mm_s,
        ttl_secs,
        valid_until_ns,
    }
}

// ── Bridge 3: Agri → Analytics (yield metrics) ───────────────────────────

/// Yield metrics for ALICE-Analytics ingestion.
pub struct AgriAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Number of field observations in the reporting window.
    pub observation_count: u64,
    /// Total field area surveyed in cm².
    pub total_area_cm2: u64,
    /// Average soil moisture percentage (fixed-point × 100).
    pub avg_moisture_pct_x100: u32,
    /// Average yield estimate in grams per m² (fixed-point × 100).
    pub avg_yield_gpm2_x100: u32,
    /// Window start timestamp in nanoseconds.
    pub window_start_ns: u64,
}

/// Build yield metrics for ALICE-Analytics ingestion.
///
/// Averages use reciprocal multiply against `observation_count`.
#[inline]
#[must_use]
pub fn agri_to_analytics_metrics(
    observation_count: u64,
    total_area_cm2: u64,
    sum_moisture_pct_x100: u64,
    sum_yield_gpm2_x100: u64,
    window_start_ns: u64,
) -> AgriAnalyticsMetrics {
    let rcp = 1.0 / observation_count.max(1) as f64;
    let avg_moisture = (sum_moisture_pct_x100 as f64 * rcp) as u32;
    let avg_yield = (sum_yield_gpm2_x100 as f64 * rcp) as u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&observation_count.to_le_bytes());
    key[8..16].copy_from_slice(&total_area_cm2.to_le_bytes());
    key[16..24].copy_from_slice(&window_start_ns.to_le_bytes());
    AgriAnalyticsMetrics {
        content_hash: fnv1a(&key),
        observation_count,
        total_area_cm2,
        avg_moisture_pct_x100: avg_moisture,
        avg_yield_gpm2_x100: avg_yield,
        window_start_ns,
    }
}

// ── Bridge 4: Agri → ML (crop yield prediction input) ────────────────────

/// ML feature vector for ALICE-ML crop yield prediction.
pub struct AgriMlFeatures {
    /// Content hash over the feature values.
    pub content_hash: u64,
    /// Field area in cm² (raw feature).
    pub field_area_cm2: u64,
    /// Soil moisture percentage normalised to [0.0, 1.0].
    pub moisture_norm: f32,
    /// Temperature in degrees Celsius (f32).
    pub temperature_c: f32,
    /// FNV-1a hash of the growth stage (categorical embedding key).
    pub growth_stage_hash: u64,
    /// Current yield estimate in grams per m² (f32).
    pub yield_estimate_gpm2: f32,
    /// Number of days since last irrigation (feature for water stress).
    pub days_since_irrigation: u16,
}

/// Extract ML features for ALICE-ML crop yield prediction.
#[inline]
#[must_use]
pub fn agri_to_ml_features(
    field_area_cm2: u64,
    moisture_pct_x100: u32,
    temperature_mdeg: i32,
    growth_stage: &[u8],
    yield_estimate_gpm2_x100: u32,
    days_since_irrigation: u16,
) -> AgriMlFeatures {
    let growth_stage_hash = fnv1a(growth_stage);
    // Normalise moisture: divide by 10000 (pct_x100 max = 10000 for 100.00%).
    let moisture_norm = moisture_pct_x100 as f32 * (1.0 / 10_000.0_f32);
    let temperature_c = temperature_mdeg as f32 * 0.001_f32;
    let yield_estimate_gpm2 = yield_estimate_gpm2_x100 as f32 * 0.01_f32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&field_area_cm2.to_le_bytes());
    key[8..16].copy_from_slice(&growth_stage_hash.to_le_bytes());
    key[16..20].copy_from_slice(&moisture_pct_x100.to_le_bytes());
    key[20..24].copy_from_slice(&(temperature_mdeg as u32).to_le_bytes());
    AgriMlFeatures {
        content_hash: fnv1a(&key),
        field_area_cm2,
        moisture_norm,
        temperature_c,
        growth_stage_hash,
        yield_estimate_gpm2,
        days_since_irrigation,
    }
}

// ── Bridge 5: Agri → Notify (agricultural alerts) ────────────────────────

/// Agricultural alert payload for ALICE-Notify.
pub struct AgriNotifyAlert {
    /// Content hash over field + alert type.
    pub content_hash: u64,
    /// FNV-1a hash of the field identifier.
    pub field_id_hash: u64,
    /// Alert type code: 0=drought, 1=frost, 2=pest, 3=flood, 4=harvest_ready.
    pub alert_type: u8,
    /// Severity level: 0=info, 1=warning, 2=critical.
    pub severity: u8,
    /// Affected area in cm².
    pub affected_area_cm2: u64,
    /// Alert timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Build an agricultural alert for ALICE-Notify.
#[inline]
#[must_use]
pub fn agri_to_notify_alert(
    field_id: &[u8],
    alert_type: u8,
    severity: u8,
    affected_area_cm2: u64,
    timestamp_ns: u64,
) -> AgriNotifyAlert {
    let field_id_hash = fnv1a(field_id);
    let mut key = [0u8; 18];
    key[0..8].copy_from_slice(&field_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    key[16] = alert_type;
    key[17] = severity;
    AgriNotifyAlert {
        content_hash: fnv1a(&key),
        field_id_hash,
        alert_type,
        severity,
        affected_area_cm2,
        timestamp_ns,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const STAGE: &[u8] = b"vegetative";
    const FIELD: &[u8] = b"field:north-01";

    #[test]
    fn test_agri_to_db_record_hash_nonzero() {
        let rec = agri_to_db_record(100_000_000, 6_500, 20_000, STAGE, 50_000, 1_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.growth_stage_hash, 0);
    }

    #[test]
    fn test_agri_to_db_record_fields() {
        let rec = agri_to_db_record(200_000_000, 7_200, 22_500, STAGE, 48_000, 2_000_000_000);
        assert_eq!(rec.field_area_cm2, 200_000_000);
        assert_eq!(rec.moisture_pct_x100, 7_200);
        assert_eq!(rec.temperature_mdeg, 22_500);
        assert_eq!(rec.yield_estimate_gpm2_x100, 48_000);
    }

    #[test]
    fn test_agri_to_cache_entry_normal_ttl() {
        let entry = agri_to_cache_entry(20_000, 6_500, 5_000, 9_999_999);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_agri_to_cache_entry_frost_ttl() {
        // temperature_mdeg < 0 → frost risk → TTL = 300.
        let entry = agri_to_cache_entry(-500, 8_000, 2_000, 9_999_999);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_agri_to_analytics_metrics_avg() {
        // 4 observations, sum moisture = 28000 → avg 7000, sum yield = 200000 → avg 50000.
        let m = agri_to_analytics_metrics(4, 400_000_000, 28_000, 200_000, 0);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.avg_moisture_pct_x100, 7_000);
        assert_eq!(m.avg_yield_gpm2_x100, 50_000);
    }

    #[test]
    fn test_agri_to_analytics_metrics_zero_observations() {
        let m = agri_to_analytics_metrics(0, 0, 0, 0, 0);
        assert_eq!(m.avg_moisture_pct_x100, 0);
        assert_eq!(m.avg_yield_gpm2_x100, 0);
    }

    #[test]
    fn test_agri_to_ml_features_normalisation() {
        // moisture 5000 / 10000 = 0.5, temperature 25000 mdeg = 25.0 C.
        let f = agri_to_ml_features(100_000_000, 5_000, 25_000, STAGE, 40_000, 7);
        assert!((f.moisture_norm - 0.5).abs() < 0.0001);
        assert!((f.temperature_c - 25.0).abs() < 0.001);
        assert!((f.yield_estimate_gpm2 - 400.0).abs() < 0.1);
        assert_eq!(f.days_since_irrigation, 7);
    }

    #[test]
    fn test_agri_to_notify_alert_fields() {
        let alert = agri_to_notify_alert(FIELD, 1, 2, 50_000_000, 888_888_888);
        assert_ne!(alert.content_hash, 0);
        assert_ne!(alert.field_id_hash, 0);
        assert_eq!(alert.alert_type, 1);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.affected_area_cm2, 50_000_000);
    }
}
