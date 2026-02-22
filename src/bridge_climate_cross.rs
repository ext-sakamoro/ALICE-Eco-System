//! Cross-domain bridges — ALICE-Climate ↔ SDF, ML, View
//!
//! 3 bridges connecting planetary climate data to SDF iso-surface fields,
//! ML feature vectors, and View render data for weather visualisation.

use alice_climate::{ClimateResponse, Observation};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: ClimateResponse → SDF (iso-surface field parameters) ──────

/// SDF field parameters derived from a climate response.
///
/// Maps atmospheric temperature and wind into SDF-compatible iso-surface
/// values and gradient strengths so the SDF engine can render weather
/// phenomena as implicit surfaces (e.g. cloud boundaries, temperature fronts).
pub struct ClimateSdfField {
    /// FNV-1a hash over temperature, pressure, wind speed, and density bytes.
    pub content_hash: u64,
    /// Atmospheric temperature (Celsius).
    pub temperature_c: f64,
    /// Atmospheric pressure (hPa).
    pub pressure_hpa: f64,
    /// Iso-surface value: temperature_c / 100.0 (normalised for SDF eval).
    pub iso_surface_value: f64,
    /// Gradient strength: wind speed magnitude * 0.01.
    pub gradient_strength: f64,
    /// Air density at the query point (kg/m3).
    pub density_kg_m3: f64,
}

/// Convert a climate response into SDF field parameters.
#[inline]
pub fn climate_response_to_sdf_field(resp: &ClimateResponse) -> ClimateSdfField {
    let wind_speed = (resp.atmosphere.wind_velocity_ms[0].powi(2)
        + resp.atmosphere.wind_velocity_ms[1].powi(2)
        + resp.atmosphere.wind_velocity_ms[2].powi(2))
    .sqrt();

    let iso_surface_value = resp.atmosphere.temperature_c / 100.0;
    let gradient_strength = wind_speed * 0.01;

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&resp.atmosphere.temperature_c.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&resp.atmosphere.pressure_hpa.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&wind_speed.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&resp.atmosphere.density_kg_m3.to_bits().to_le_bytes());

    ClimateSdfField {
        content_hash: fnv1a(&key),
        temperature_c: resp.atmosphere.temperature_c,
        pressure_hpa: resp.atmosphere.pressure_hpa,
        iso_surface_value,
        gradient_strength,
        density_kg_m3: resp.atmosphere.density_kg_m3,
    }
}

// ── Bridge 2: Observation → ML (feature vector) ─────────────────────────

/// ML feature vector derived from a weather station observation.
///
/// Extracts 7 features (temperature, pressure, humidity, wind speed,
/// wind direction, time-of-day normalised) for downstream classifiers
/// and regression models.
pub struct ClimateMlFeatures {
    /// FNV-1a hash over station_id, timestamp, temperature, and pressure bytes.
    pub content_hash: u64,
    /// Station ID.
    pub station_id: u64,
    /// Observation timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Feature count: always 7.
    pub feature_count: usize,
    /// Temperature (Celsius).
    pub temperature_c: f64,
    /// Pressure (hPa).
    pub pressure_hpa: f64,
    /// Humidity (percentage, 0-100).
    pub humidity_pct: f64,
    /// Normalised time-of-day: (timestamp_ns % (24*3600*1e9)) / (24*3600*1e9).
    pub normalized_time: f64,
}

/// Convert a weather observation into an ML feature vector.
#[inline]
pub fn climate_observation_to_ml_features(obs: &Observation) -> ClimateMlFeatures {
    let nanos_per_day: u64 = 24 * 3600 * 1_000_000_000;
    let time_of_day_ns = obs.timestamp_ns % nanos_per_day;
    let normalized_time = time_of_day_ns as f64 / nanos_per_day as f64;

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&obs.station_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&obs.timestamp_ns.to_le_bytes());
    key[16..24].copy_from_slice(&obs.temperature_c.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&obs.pressure_hpa.to_bits().to_le_bytes());

    ClimateMlFeatures {
        content_hash: fnv1a(&key),
        station_id: obs.station_id.0,
        timestamp_ns: obs.timestamp_ns,
        feature_count: 7,
        temperature_c: obs.temperature_c,
        pressure_hpa: obs.pressure_hpa,
        humidity_pct: obs.humidity_pct,
        normalized_time,
    }
}

// ── Bridge 3: ClimateResponse → View (render data) ──────────────────────

/// View render data derived from a climate response.
///
/// Pre-computes colour mapping (blue=cold to red=hot), wind arrow vectors,
/// and ocean opacity so the View renderer can display weather overlays
/// without recomputing per-frame.
pub struct ClimateViewData {
    /// FNV-1a hash over temperature, wind velocity, and ocean presence bytes.
    pub content_hash: u64,
    /// Temperature-mapped red channel: clamp((temp + 40) / 80, 0, 1).
    pub temperature_color_r: f32,
    /// Temperature-mapped blue channel: 1.0 - color_r.
    pub temperature_color_b: f32,
    /// Wind arrow X component: wind_u * 0.01.
    pub wind_arrow_dx: f32,
    /// Wind arrow Y component: wind_v * 0.01.
    pub wind_arrow_dy: f32,
    /// Ocean surface opacity: 0.8 if ocean is present, else 0.3.
    pub ocean_opacity: f32,
    /// Atmospheric temperature (Celsius) for tooltip display.
    pub temperature_c: f64,
}

/// Convert a climate response into View render data.
#[inline]
pub fn climate_response_to_view_data(resp: &ClimateResponse) -> ClimateViewData {
    let temp = resp.atmosphere.temperature_c;
    let color_r = ((temp + 40.0) / 80.0).clamp(0.0, 1.0) as f32;
    let color_b = 1.0 - color_r;

    let wind_u = resp.atmosphere.wind_velocity_ms[0];
    let wind_v = resp.atmosphere.wind_velocity_ms[1];
    let wind_arrow_dx = (wind_u * 0.01) as f32;
    let wind_arrow_dy = (wind_v * 0.01) as f32;

    // Ocean opacity: present → 0.8, absent → 0.3
    let ocean_opacity = if resp.ocean.is_some() { 0.8f32 } else { 0.3f32 };

    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&temp.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&wind_u.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&wind_v.to_bits().to_le_bytes());
    key[24] = resp.ocean.is_some() as u8;

    ClimateViewData {
        content_hash: fnv1a(&key),
        temperature_color_r: color_r,
        temperature_color_b: color_b,
        wind_arrow_dx,
        wind_arrow_dy,
        ocean_opacity,
        temperature_c: temp,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_climate::{
        AtmosphericLayer, AtmosphericState, ClimateQuery, ClimateResponse,
        OceanState, StationId, WeatherStation, Observation, evaluate_climate,
    };

    fn make_surface_response() -> ClimateResponse {
        ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: 25.0,
                pressure_hpa: 1013.25,
                humidity_pct: 60.0,
                wind_velocity_ms: [3.0, 1.0, 0.0],
                density_kg_m3: 1.18,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        }
    }

    fn make_ocean_response() -> ClimateResponse {
        ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: 20.0,
                pressure_hpa: 1013.25,
                humidity_pct: 75.0,
                wind_velocity_ms: [2.0, 1.0, 0.0],
                density_kg_m3: 1.2,
            },
            ocean: Some(OceanState {
                temperature_c: 15.0,
                salinity_psu: 35.0,
                current_velocity_ms: [0.3, 0.1, 0.0],
                pressure_bar: 51.0,
                density_kg_m3: 1027.0,
            }),
            layer: AtmosphericLayer::Troposphere,
        }
    }

    fn make_observation(temp: f64, ts_ns: u64) -> Observation {
        Observation {
            station_id: StationId(1),
            timestamp_ns: ts_ns,
            temperature_c: temp,
            pressure_hpa: 1013.25,
            humidity_pct: 60.0,
            wind_speed_ms: 5.0,
            wind_direction_rad: std::f64::consts::PI,
        }
    }

    // ── Bridge 1: climate response → SDF field ──────────────────────────

    #[test]
    fn test_response_to_sdf_field() {
        let resp = make_surface_response();
        let sdf = climate_response_to_sdf_field(&resp);
        assert_ne!(sdf.content_hash, 0);
        assert!((sdf.temperature_c - 25.0).abs() < 1e-10);
        assert!((sdf.pressure_hpa - 1013.25).abs() < 1e-10);
        assert!((sdf.iso_surface_value - 0.25).abs() < 1e-10); // 25/100
        // wind_speed = sqrt(9+1+0) = sqrt(10) ≈ 3.162, gradient = 3.162*0.01
        assert!(sdf.gradient_strength > 0.03 && sdf.gradient_strength < 0.04);
        assert!(sdf.density_kg_m3 > 0.0);
    }

    #[test]
    fn test_response_to_sdf_field_cold() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: -40.0,
                pressure_hpa: 500.0,
                humidity_pct: 10.0,
                wind_velocity_ms: [0.0, 0.0, 0.0],
                density_kg_m3: 0.7,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let sdf = climate_response_to_sdf_field(&resp);
        assert!((sdf.iso_surface_value - (-0.4)).abs() < 1e-10); // -40/100
        assert!((sdf.gradient_strength - 0.0).abs() < 1e-10); // no wind
    }

    #[test]
    fn test_response_to_sdf_field_deterministic() {
        let resp = make_surface_response();
        let s1 = climate_response_to_sdf_field(&resp);
        let s2 = climate_response_to_sdf_field(&resp);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    // ── Bridge 2: observation → ML features ─────────────────────────────

    #[test]
    fn test_observation_to_ml_features() {
        // Timestamp: exactly 6 hours into a day
        let six_hours_ns: u64 = 6 * 3600 * 1_000_000_000;
        let obs = make_observation(22.5, six_hours_ns);
        let features = climate_observation_to_ml_features(&obs);
        assert_ne!(features.content_hash, 0);
        assert_eq!(features.station_id, 1);
        assert_eq!(features.timestamp_ns, six_hours_ns);
        assert_eq!(features.feature_count, 7);
        assert!((features.temperature_c - 22.5).abs() < 1e-10);
        assert!((features.pressure_hpa - 1013.25).abs() < 1e-10);
        assert!((features.humidity_pct - 60.0).abs() < 1e-10);
        // 6 hours = 0.25 of a day
        assert!((features.normalized_time - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_observation_to_ml_features_midnight() {
        let nanos_per_day: u64 = 24 * 3600 * 1_000_000_000;
        let obs = make_observation(10.0, nanos_per_day); // exactly midnight next day
        let features = climate_observation_to_ml_features(&obs);
        // nanos_per_day % nanos_per_day = 0 → normalized_time = 0.0
        assert!((features.normalized_time - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_observation_to_ml_features_deterministic() {
        let obs = make_observation(20.0, 5_000_000_000);
        let f1 = climate_observation_to_ml_features(&obs);
        let f2 = climate_observation_to_ml_features(&obs);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 3: climate response → View data ──────────────────────────

    #[test]
    fn test_response_to_view_data_warm() {
        let resp = make_surface_response(); // 25 C
        let view = climate_response_to_view_data(&resp);
        assert_ne!(view.content_hash, 0);
        // color_r = (25 + 40) / 80 = 65/80 = 0.8125
        assert!((view.temperature_color_r - 0.8125).abs() < 0.001);
        assert!((view.temperature_color_b - (1.0 - 0.8125)).abs() < 0.001);
        // wind: u=3.0, v=1.0 → dx=0.03, dy=0.01
        assert!((view.wind_arrow_dx - 0.03).abs() < 0.001);
        assert!((view.wind_arrow_dy - 0.01).abs() < 0.001);
        // No ocean → opacity 0.3
        assert!((view.ocean_opacity - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_response_to_view_data_cold() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: -40.0,
                pressure_hpa: 1013.25,
                humidity_pct: 30.0,
                wind_velocity_ms: [0.0, 0.0, 0.0],
                density_kg_m3: 1.5,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let view = climate_response_to_view_data(&resp);
        // color_r = (-40 + 40) / 80 = 0.0 → pure blue
        assert!((view.temperature_color_r - 0.0).abs() < 0.001);
        assert!((view.temperature_color_b - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_response_to_view_data_hot() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: 50.0,
                pressure_hpa: 1013.25,
                humidity_pct: 10.0,
                wind_velocity_ms: [10.0, 5.0, 0.0],
                density_kg_m3: 1.0,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let view = climate_response_to_view_data(&resp);
        // color_r = (50 + 40) / 80 = 90/80 = 1.125 → clamped to 1.0
        assert!((view.temperature_color_r - 1.0).abs() < 0.001);
        assert!((view.temperature_color_b - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_response_to_view_data_with_ocean() {
        let resp = make_ocean_response();
        let view = climate_response_to_view_data(&resp);
        // Ocean present → opacity 0.8
        assert!((view.ocean_opacity - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_response_to_view_data_deterministic() {
        let resp = make_surface_response();
        let v1 = climate_response_to_view_data(&resp);
        let v2 = climate_response_to_view_data(&resp);
        assert_eq!(v1.content_hash, v2.content_hash);
    }

    // ── Integration: evaluate_climate → bridges ─────────────────────────

    #[test]
    fn test_full_pipeline_evaluate_to_sdf() {
        let query = ClimateQuery {
            latitude: 35.6,
            longitude: 139.7,
            altitude_m: 0.0,
            timestamp_ns: 180u64 * 24 * 3600 * 1_000_000_000,
        };
        let resp = evaluate_climate(&query);
        let sdf = climate_response_to_sdf_field(&resp);
        assert_ne!(sdf.content_hash, 0);
        assert!(sdf.pressure_hpa > 0.0);
    }

    #[test]
    fn test_full_pipeline_evaluate_to_view() {
        let query = ClimateQuery {
            latitude: 0.0,
            longitude: 0.0,
            altitude_m: -500.0, // ocean
            timestamp_ns: 90u64 * 24 * 3600 * 1_000_000_000,
        };
        let resp = evaluate_climate(&query);
        let view = climate_response_to_view_data(&resp);
        // Below sea level → ocean present → opacity 0.8
        assert!((view.ocean_opacity - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_full_pipeline_station_to_ml() {
        let station = WeatherStation::new(42, "Tokyo", 35.6762, 139.6503, 40.0);
        let obs = Observation {
            station_id: station.id,
            timestamp_ns: 12u64 * 3600 * 1_000_000_000, // noon
            temperature_c: 28.0,
            pressure_hpa: 1010.0,
            humidity_pct: 70.0,
            wind_speed_ms: 3.5,
            wind_direction_rad: 1.5,
        };
        let features = climate_observation_to_ml_features(&obs);
        assert_eq!(features.station_id, 42);
        assert_eq!(features.feature_count, 7);
        // noon = 0.5 of a day
        assert!((features.normalized_time - 0.5).abs() < 1e-6);
    }
}
