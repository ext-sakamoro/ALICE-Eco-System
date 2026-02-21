//! Climate bridges — ALICE-Climate ↔ Analytics, DB, Edge, Cache
//!
//! 5 bridges connecting planetary climate data to the ALICE ecosystem.

use alice_climate::{
    ClimateAnomaly, ClimateResponse, AnomalyKind, Observation, WeatherStation,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: WeatherStation Observation → Analytics (weather metrics) ──

/// Weather observation metrics for ALICE-Analytics ingestion.
pub struct ClimateAnalyticsObservationEvent {
    /// Content hash over station ID, temperature, and humidity bytes.
    pub content_hash: u64,
    /// Inner u64 of the station ID.
    pub station_id: u64,
    /// Temperature in Celsius.
    pub temperature_c: f64,
    /// Relative humidity as a percentage (0–100).
    pub humidity_pct: f64,
    /// Atmospheric pressure in hPa.
    pub pressure_hpa: f64,
    /// Wind speed in m/s.
    pub wind_speed_ms: f64,
    /// Observation timestamp (nanoseconds).
    pub timestamp_ns: u64,
}

/// Convert a weather observation into an analytics event.
#[inline]
pub fn climate_observation_to_analytics(station: &WeatherStation, obs: &Observation) -> ClimateAnalyticsObservationEvent {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&obs.station_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&obs.temperature_c.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&obs.humidity_pct.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&obs.pressure_hpa.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&obs.wind_speed_ms.to_bits().to_le_bytes());

    ClimateAnalyticsObservationEvent {
        content_hash: fnv1a(&key),
        station_id: station.id.0,
        temperature_c: obs.temperature_c,
        humidity_pct: obs.humidity_pct,
        pressure_hpa: obs.pressure_hpa,
        wind_speed_ms: obs.wind_speed_ms,
        timestamp_ns: obs.timestamp_ns,
    }
}

// ── Bridge 2: ClimateResponse → Analytics (climate field metrics) ───────

/// Climate field evaluation metrics for ALICE-Analytics ingestion.
pub struct ClimateAnalyticsFieldEvent {
    /// Content hash over temperature, pressure, and density bytes.
    pub content_hash: u64,
    /// Temperature at the query point (Celsius).
    pub temperature_c: f64,
    /// Pressure at the query point (hPa).
    pub pressure_hpa: f64,
    /// Density at the query point (kg/m3).
    pub density_kg_m3: f64,
    /// Wind velocity vector at the query point [u, v, w] (m/s).
    pub wind_velocity_ms: [f64; 3],
}

/// Convert a climate response into an analytics field event.
#[inline]
pub fn climate_response_to_analytics(resp: &ClimateResponse) -> ClimateAnalyticsFieldEvent {
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&resp.atmosphere.temperature_c.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&resp.atmosphere.pressure_hpa.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&resp.atmosphere.density_kg_m3.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&resp.atmosphere.wind_velocity_ms[0].to_bits().to_le_bytes());

    ClimateAnalyticsFieldEvent {
        content_hash: fnv1a(&key),
        temperature_c: resp.atmosphere.temperature_c,
        pressure_hpa: resp.atmosphere.pressure_hpa,
        density_kg_m3: resp.atmosphere.density_kg_m3,
        wind_velocity_ms: resp.atmosphere.wind_velocity_ms,
    }
}

// ── Bridge 3: ClimateResponse → DB (climate snapshot record) ────────────

/// Climate snapshot record for ALICE-DB persistence.
pub struct ClimateDbSnapshotRecord {
    /// Content hash over all climate response fields.
    pub content_hash: u64,
    /// Atmospheric temperature (Celsius).
    pub temperature_c: f64,
    /// Atmospheric pressure (hPa).
    pub pressure_hpa: f64,
    /// Atmospheric density (kg/m3).
    pub density_kg_m3: f64,
    /// Wind velocity vector [u, v, w] (m/s).
    pub wind_velocity_ms: [f64; 3],
    /// Ocean temperature (Celsius), if below sea level.
    pub ocean_temperature_c: Option<f64>,
    /// Ocean salinity (PSU), if below sea level.
    pub ocean_salinity_psu: Option<f64>,
    /// Ocean density (kg/m3), if below sea level.
    pub ocean_density_kg_m3: Option<f64>,
}

/// Convert a climate response into a DB snapshot record.
#[inline]
pub fn climate_response_to_db(resp: &ClimateResponse) -> ClimateDbSnapshotRecord {
    let ocean_temp = resp.ocean.as_ref().map(|o| o.temperature_c);
    let ocean_sal = resp.ocean.as_ref().map(|o| o.salinity_psu);
    let ocean_dens = resp.ocean.as_ref().map(|o| o.density_kg_m3);

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&resp.atmosphere.temperature_c.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&resp.atmosphere.pressure_hpa.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&resp.atmosphere.density_kg_m3.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&ocean_temp.unwrap_or(0.0).to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&ocean_sal.unwrap_or(0.0).to_bits().to_le_bytes());

    ClimateDbSnapshotRecord {
        content_hash: fnv1a(&key),
        temperature_c: resp.atmosphere.temperature_c,
        pressure_hpa: resp.atmosphere.pressure_hpa,
        density_kg_m3: resp.atmosphere.density_kg_m3,
        wind_velocity_ms: resp.atmosphere.wind_velocity_ms,
        ocean_temperature_c: ocean_temp,
        ocean_salinity_psu: ocean_sal,
        ocean_density_kg_m3: ocean_dens,
    }
}

// ── Bridge 4: Anomaly → Edge (real-time anomaly alert) ─────────────────

/// Climate anomaly alert for ALICE-Edge ingestion.
pub struct ClimateEdgeAnomalyAlert {
    /// Content hash over anomaly kind and atmospheric state bytes.
    pub content_hash: u64,
    /// Anomaly kind: 0=HeatWave, 1=ColdSnap, 2=Storm, 3=Drought, 4=Flood.
    pub anomaly_kind: u8,
    /// Temperature at the anomaly location (Celsius).
    pub temperature_c: f64,
    /// Wind speed magnitude (m/s), computed from wind_velocity_ms.
    pub wind_speed_ms: f64,
    /// Anomaly magnitude from detect_anomaly.
    pub magnitude: f64,
    /// Location latitude.
    pub location_lat: f64,
    /// Location longitude.
    pub location_lon: f64,
}

/// Convert a ClimateAnomaly and associated ClimateResponse into an edge alert.
#[inline]
pub fn climate_anomaly_to_edge(anomaly: &ClimateAnomaly, resp: &ClimateResponse) -> ClimateEdgeAnomalyAlert {
    let kind_byte = match anomaly.kind {
        AnomalyKind::HeatWave => 0u8,
        AnomalyKind::ColdSnap => 1,
        AnomalyKind::Storm => 2,
        AnomalyKind::Drought => 3,
        AnomalyKind::Flood => 4,
    };

    let wv = resp.atmosphere.wind_velocity_ms;
    let wind_speed = (wv[0].powi(2) + wv[1].powi(2) + wv[2].powi(2)).sqrt();

    let mut key = [0u8; 25];
    key[0] = kind_byte;
    key[1..9].copy_from_slice(&resp.atmosphere.temperature_c.to_bits().to_le_bytes());
    key[9..17].copy_from_slice(&wind_speed.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&anomaly.magnitude.to_bits().to_le_bytes());

    ClimateEdgeAnomalyAlert {
        content_hash: fnv1a(&key),
        anomaly_kind: kind_byte,
        temperature_c: resp.atmosphere.temperature_c,
        wind_speed_ms: wind_speed,
        magnitude: anomaly.magnitude,
        location_lat: anomaly.location_lat,
        location_lon: anomaly.location_lon,
    }
}

// ── Bridge 5: ClimateResponse → Cache (real-time climate lookup) ────────

/// Climate cache entry for ALICE-Cache real-time lookup.
pub struct ClimateCacheEntry {
    /// Content hash over temperature and pressure bytes.
    pub content_hash: u64,
    /// Temperature (Celsius).
    pub temperature_c: f64,
    /// Pressure (hPa).
    pub pressure_hpa: f64,
    /// Humidity (percentage, 0–100).
    pub humidity_pct: f64,
    /// Cache TTL in seconds. High wind or extreme temperature → shorter TTL.
    pub ttl_secs: u32,
}

/// Convert a climate response into a cache entry with adaptive TTL.
///
/// TTL heuristic: base 120 s, reduced by 55 s for wind > 20 m/s,
/// reduced by 55 s for extreme temperature (> 40 C or < -20 C).
/// Minimum TTL: 10 s.
#[inline]
pub fn climate_response_to_cache(resp: &ClimateResponse) -> ClimateCacheEntry {
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&resp.atmosphere.temperature_c.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&resp.atmosphere.pressure_hpa.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&resp.atmosphere.humidity_pct.to_bits().to_le_bytes());

    // Adaptive TTL based on atmospheric volatility
    let wv = resp.atmosphere.wind_velocity_ms;
    let wind_speed = (wv[0].powi(2) + wv[1].powi(2) + wv[2].powi(2)).sqrt();
    let temp = resp.atmosphere.temperature_c;

    let mut ttl: u32 = 120;
    // High wind → more volatile → shorter cache
    let high_wind = (wind_speed > 20.0) as u32;
    ttl -= high_wind * 55;
    // Extreme temperature → shorter cache
    let extreme_temp = (temp > 40.0 || temp < -20.0) as u32;
    ttl -= extreme_temp * 55;
    // Floor at 10 s
    if ttl < 10 { ttl = 10; }

    ClimateCacheEntry {
        content_hash: fnv1a(&key),
        temperature_c: temp,
        pressure_hpa: resp.atmosphere.pressure_hpa,
        humidity_pct: resp.atmosphere.humidity_pct,
        ttl_secs: ttl,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_climate::{
        AtmosphericLayer, AtmosphericState, ClimateQuery, ClimateResponse,
        OceanState, StationId, WeatherStation, Observation, AnomalyKind,
        detect_anomaly, evaluate_climate,
    };

    fn make_station() -> WeatherStation {
        WeatherStation::new(1, "TestStation", 35.6762, 139.6503, 40.0)
    }

    fn make_observation(temp: f64, humidity_pct: f64) -> Observation {
        Observation {
            station_id: StationId(1),
            timestamp_ns: 1_000_000_000,
            temperature_c: temp,
            pressure_hpa: 1013.25,
            humidity_pct,
            wind_speed_ms: 5.0,
            wind_direction_rad: std::f64::consts::PI,
        }
    }

    fn make_response_surface() -> ClimateResponse {
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

    fn make_response_ocean() -> ClimateResponse {
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

    #[test]
    fn test_observation_to_analytics() {
        let station = make_station();
        let obs = make_observation(25.0, 60.0);
        let ev = climate_observation_to_analytics(&station, &obs);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.station_id, 1);
        assert!((ev.temperature_c - 25.0).abs() < 1e-10);
        assert!((ev.humidity_pct - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_response_to_analytics() {
        let query = ClimateQuery {
            latitude: 35.0,
            longitude: 139.0,
            altitude_m: 0.0,
            timestamp_ns: 180u64 * 24 * 3600 * 1_000_000_000,
        };
        let resp = evaluate_climate(&query);
        let ev = climate_response_to_analytics(&resp);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.temperature_c > -100.0);
        assert!(ev.pressure_hpa > 0.0);
    }

    #[test]
    fn test_response_to_db_no_ocean() {
        let resp = make_response_surface();
        let rec = climate_response_to_db(&resp);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.ocean_temperature_c.is_none());
        assert!(rec.ocean_salinity_psu.is_none());
    }

    #[test]
    fn test_response_to_db_with_ocean() {
        let resp = make_response_ocean();
        let rec = climate_response_to_db(&resp);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.ocean_temperature_c.unwrap() - 15.0).abs() < 1e-10);
        assert!((rec.ocean_salinity_psu.unwrap() - 35.0).abs() < 1e-10);
        assert!(rec.ocean_density_kg_m3.unwrap() > 1000.0);
    }

    #[test]
    fn test_anomaly_to_edge() {
        let baseline = AtmosphericState {
            temperature_c: 20.0,
            pressure_hpa: 1013.25,
            humidity_pct: 60.0,
            wind_velocity_ms: [2.0, 1.0, 0.0],
            density_kg_m3: 1.2,
        };
        let current = AtmosphericState {
            temperature_c: 45.0, // +25 deviation → HeatWave
            ..baseline
        };
        let resp = ClimateResponse {
            atmosphere: current,
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let anomaly = detect_anomaly(&current, &baseline, 35.0, 139.0, 0).unwrap();
        assert_eq!(anomaly.kind, AnomalyKind::HeatWave);

        let alert = climate_anomaly_to_edge(&anomaly, &resp);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.anomaly_kind, 0); // HeatWave
        assert!((alert.temperature_c - 45.0).abs() < 1e-10);
        assert!(alert.magnitude > 0.0);
        assert!((alert.location_lat - 35.0).abs() < 1e-10);
    }

    #[test]
    fn test_response_to_cache_normal() {
        let resp = make_response_surface();
        let entry = climate_response_to_cache(&resp);
        assert_eq!(entry.ttl_secs, 120); // normal conditions
        assert!((entry.temperature_c - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_response_to_cache_extreme_temp() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: 50.0, // extreme heat
                pressure_hpa: 1013.25,
                humidity_pct: 20.0,
                wind_velocity_ms: [1.0, 0.0, 0.0],
                density_kg_m3: 1.1,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let entry = climate_response_to_cache(&resp);
        assert_eq!(entry.ttl_secs, 65); // 120 - 55
    }

    #[test]
    fn test_response_to_cache_high_wind() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: 20.0,
                pressure_hpa: 1013.25,
                humidity_pct: 50.0,
                wind_velocity_ms: [25.0, 10.0, 0.0], // ~27 m/s
                density_kg_m3: 1.2,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let entry = climate_response_to_cache(&resp);
        assert_eq!(entry.ttl_secs, 65); // 120 - 55
    }

    #[test]
    fn test_response_to_cache_extreme_both() {
        let resp = ClimateResponse {
            atmosphere: AtmosphericState {
                temperature_c: -30.0, // extreme cold
                pressure_hpa: 980.0,
                humidity_pct: 10.0,
                wind_velocity_ms: [30.0, 15.0, 0.0], // high wind
                density_kg_m3: 1.4,
            },
            ocean: None,
            layer: AtmosphericLayer::Troposphere,
        };
        let entry = climate_response_to_cache(&resp);
        assert_eq!(entry.ttl_secs, 10); // 120 - 55 - 55 = 10
    }

    #[test]
    fn test_hash_determinism() {
        let station = make_station();
        let obs = make_observation(20.0, 50.0);
        let e1 = climate_observation_to_analytics(&station, &obs);
        let e2 = climate_observation_to_analytics(&station, &obs);
        assert_eq!(e1.content_hash, e2.content_hash);
    }
}
