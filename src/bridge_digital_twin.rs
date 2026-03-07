//! DigitalTwin bridges — ALICE-Digital-Twin ↔ DB, Analytics, Physics, Cache, Edge
//!
//! 5 bridges connecting the digital twin layer to the ALICE ecosystem.
//! Covers twin state persistence in DB, metrics in Analytics, simulation
//! data for Physics, state caching, and IoT event delivery via Edge.

use alice_digital_twin::{StateDiff, TwinState};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: DigitalTwin → DB (twin state persistence) ──────────────────

/// Twin state record for ALICE-DB persistence.
///
/// Written on every state snapshot so the database layer can store and
/// query historical twin states by twin ID, timestamp, or property delta.
pub struct DigitalTwinDbStateRecord {
    /// FNV-1a hash over twin ID and timestamp bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the twin ID string.
    pub twin_id_hash: u64,
    /// Number of properties in the twin state snapshot.
    pub property_count: u32,
    /// Snapshot timestamp in milliseconds since Unix epoch.
    pub timestamp_ms: u64,
}

/// Convert a twin state snapshot into a DB record for ALICE-DB.
#[inline]
#[must_use]
pub fn digital_twin_state_to_db_record(state: &TwinState) -> DigitalTwinDbStateRecord {
    let twin_id_hash = fnv1a(state.id.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state.timestamp.to_le_bytes());
    DigitalTwinDbStateRecord {
        content_hash: fnv1a(&key),
        twin_id_hash,
        property_count: state.properties.len() as u32,
        timestamp_ms: state.timestamp,
    }
}

// ── Bridge 2: DigitalTwin → Analytics (twin metrics event) ───────────────

/// Twin metrics event for ALICE-Analytics.
///
/// Emitted on state diff so the analytics layer can compute change rates,
/// property velocity distributions, and anomaly frequency aggregates.
pub struct DigitalTwinAnalyticsMetricsEvent {
    /// FNV-1a hash over twin ID hash, changed count, and timestamp bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the twin ID — analytics stream key.
    pub twin_id_hash: u64,
    /// Number of changed properties in this diff.
    pub changed_count: u32,
    /// Number of added properties in this diff.
    pub added_count: u32,
    /// Number of removed properties in this diff.
    pub removed_count: u32,
    /// Diff timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Convert a state diff into a metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn digital_twin_diff_to_analytics_event(
    twin_id: &str,
    diff: &StateDiff,
    timestamp_ms: u64,
) -> DigitalTwinAnalyticsMetricsEvent {
    let twin_id_hash = fnv1a(twin_id.as_bytes());
    let changed_count = diff.changed.len() as u32;
    let added_count = diff.added.len() as u32;
    let removed_count = diff.removed.len() as u32;
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&changed_count.to_le_bytes());
    key[12..20].copy_from_slice(&timestamp_ms.to_le_bytes());
    key[20..24].copy_from_slice(&(added_count + removed_count).to_le_bytes());
    DigitalTwinAnalyticsMetricsEvent {
        content_hash: fnv1a(&key),
        twin_id_hash,
        changed_count,
        added_count,
        removed_count,
        timestamp_ms,
    }
}

// ── Bridge 3: DigitalTwin → Physics (simulation data) ────────────────────

/// Physics simulation input derived from a twin state for ALICE-Physics.
///
/// Extracts property counts from a twin state snapshot so the physics engine
/// can use real-world sensor data as initial conditions.  All properties are
/// f64-valued; the numeric count equals the total property count.
pub struct DigitalTwinPhysicsSimInput {
    /// FNV-1a hash over twin ID and timestamp bytes.
    pub content_hash: u64,
    /// FNV-1a hash of the twin ID — physics object identifier.
    pub twin_id_hash: u64,
    /// Simulation timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Number of f64-valued properties available for simulation.
    pub numeric_property_count: u32,
}

/// Convert a twin state into a physics simulation input descriptor for ALICE-Physics.
#[inline]
#[must_use]
pub fn digital_twin_state_to_physics_input(state: &TwinState) -> DigitalTwinPhysicsSimInput {
    let twin_id_hash = fnv1a(state.id.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state.timestamp.to_le_bytes());
    let numeric_property_count = state.properties.len() as u32;
    DigitalTwinPhysicsSimInput {
        content_hash: fnv1a(&key),
        twin_id_hash,
        timestamp_ms: state.timestamp,
        numeric_property_count,
    }
}

// ── Bridge 4: DigitalTwin → Cache (twin state cache) ─────────────────────

/// Twin state cache entry for ALICE-Cache.
///
/// Caches the latest twin state so real-time read paths avoid hitting DB
/// on every request.  Active twins (many properties) receive a shorter TTL
/// to keep the cache fresh relative to their update frequency.
pub struct DigitalTwinCacheEntry {
    /// FNV-1a hash over twin ID and timestamp bytes — cache key.
    pub content_hash: u64,
    /// FNV-1a hash of the twin ID used as the cache lookup key.
    pub twin_id_hash: u64,
    /// Number of properties in the cached state.
    pub property_count: u32,
    /// State timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Cache TTL in seconds: 5 for active twins (> 20 properties), else 30.
    pub ttl_secs: u32,
}

/// Build a twin state cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: active twins (> 20 properties) → 5 s;
/// simple twins (<= 20 properties) → 30 s.
#[inline]
#[must_use]
pub fn digital_twin_state_to_cache_entry(state: &TwinState) -> DigitalTwinCacheEntry {
    let twin_id_hash = fnv1a(state.id.as_bytes());
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state.timestamp.to_le_bytes());
    let property_count = state.properties.len() as u32;
    // Branchless TTL: active=1 → 30-25=5, simple=0 → 30.
    let active = (property_count > 20) as u32;
    let ttl_secs = 30 - active * 25;
    DigitalTwinCacheEntry {
        content_hash: fnv1a(&key),
        twin_id_hash,
        property_count,
        timestamp_ms: state.timestamp,
        ttl_secs,
    }
}

// ── Bridge 5: DigitalTwin → Edge (IoT event delivery) ────────────────────

/// IoT event payload for ALICE-Edge delivery.
///
/// Packages an anomaly detection result as a compact edge event so the
/// edge layer can route anomaly alerts to IoT devices or control systems.
pub struct DigitalTwinEdgeEvent {
    /// FNV-1a hash over twin ID hash, anomaly index, and z-score bits.
    pub content_hash: u64,
    /// FNV-1a hash of the twin ID — edge routing key.
    pub twin_id_hash: u64,
    /// Index of the anomalous sample in the time series.
    pub sample_index: usize,
    /// Z-score of the anomalous value scaled to permille.
    pub z_score_permille: u32,
    /// Event severity: 0=info, 1=warning, 2=critical (z_score > 3000 permille).
    pub severity: u8,
}

/// Convert a time series anomaly into an edge IoT event for ALICE-Edge.
///
/// `sample_index` is the index returned by `TimeSeries::detect_anomalies`.
/// `z_score` is the caller-computed z-score for that sample
/// (|value - mean| / std_dev).
///
/// Severity is computed branchlessly:
/// z-score > 3.0 (3000 permille) → critical (2),
/// z-score > 2.0 (2000 permille) → warning (1),
/// else → info (0).
#[inline]
#[must_use]
pub fn digital_twin_anomaly_to_edge_event(
    twin_id: &str,
    sample_index: usize,
    z_score: f64,
) -> DigitalTwinEdgeEvent {
    let twin_id_hash = fnv1a(twin_id.as_bytes());
    let z_score_permille = (z_score * 1000.0).abs() as u32;
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(sample_index as u64).to_le_bytes());
    key[16..20].copy_from_slice(&z_score_permille.to_le_bytes());
    // Branchless severity: each level adds 1 if threshold exceeded.
    let above_warning = (z_score_permille > 2000) as u8;
    let above_critical = (z_score_permille > 3000) as u8;
    let severity = above_warning + above_critical;
    DigitalTwinEdgeEvent {
        content_hash: fnv1a(&key),
        twin_id_hash,
        sample_index,
        z_score_permille,
        severity,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(id: &str, ts: u64, props: Vec<(&str, f64)>) -> TwinState {
        let mut state = TwinState::new(id, ts);
        for (k, v) in props {
            state.set(k, v);
        }
        state
    }

    fn make_diff(changed: usize, added: usize, removed: usize) -> StateDiff {
        StateDiff {
            changed: (0..changed)
                .map(|i| (format!("k{i}"), i as f64, i as f64 + 1.0))
                .collect(),
            added: (0..added).map(|i| (format!("a{i}"), i as f64)).collect(),
            removed: (0..removed).map(|i| format!("r{i}")).collect(),
        }
    }

    #[test]
    fn test_state_to_db_record() {
        let state = make_state(
            "twin-A",
            1_700_000_000_000,
            vec![("temperature", 25.5), ("active", 1.0)],
        );
        let rec = digital_twin_state_to_db_record(&state);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.twin_id_hash, 0);
        assert_eq!(rec.property_count, 2);
        assert_eq!(rec.timestamp_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_diff_to_analytics_event() {
        let diff = make_diff(3, 1, 2);
        let ev = digital_twin_diff_to_analytics_event("twin-B", &diff, 1_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.twin_id_hash, 0);
        assert_eq!(ev.changed_count, 3);
        assert_eq!(ev.added_count, 1);
        assert_eq!(ev.removed_count, 2);
        assert_eq!(ev.timestamp_ms, 1_000_000);
    }

    #[test]
    fn test_state_to_physics_input_numeric_count() {
        let state = make_state(
            "twin-C",
            500,
            vec![
                ("pos_x", 1.0),
                ("pos_y", 2.0),
                ("vel_x", 3.0),
                ("enabled", 0.0),
                ("scale", 1.5),
            ],
        );
        let inp = digital_twin_state_to_physics_input(&state);
        assert_ne!(inp.content_hash, 0);
        assert_eq!(inp.numeric_property_count, 5);
    }

    #[test]
    fn test_state_to_cache_entry_simple_ttl() {
        // <= 20 properties → ttl = 30
        let props: Vec<_> = (0..5)
            .map(|i| {
                (
                    Box::leak(format!("p{i}").into_boxed_str()) as &str,
                    i as f64,
                )
            })
            .collect();
        let state = make_state("twin-D", 100, props);
        let entry = digital_twin_state_to_cache_entry(&state);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 30);
    }

    #[test]
    fn test_state_to_cache_entry_active_ttl() {
        // > 20 properties → ttl = 5
        let props: Vec<_> = (0..25)
            .map(|i| {
                (
                    Box::leak(format!("p{i}").into_boxed_str()) as &str,
                    i as f64,
                )
            })
            .collect();
        let state = make_state("twin-E", 200, props);
        let entry = digital_twin_state_to_cache_entry(&state);
        assert_eq!(entry.ttl_secs, 5);
        assert_eq!(entry.property_count, 25);
    }

    #[test]
    fn test_anomaly_to_edge_event_info() {
        // z_score = 1.5 → severity = 0 (info)
        let ev = digital_twin_anomaly_to_edge_event("twin-F", 10, 1.5);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.sample_index, 10);
        assert_eq!(ev.severity, 0);
    }

    #[test]
    fn test_anomaly_to_edge_event_warning() {
        // z_score = 2.5 → severity = 1 (warning)
        let ev = digital_twin_anomaly_to_edge_event("twin-G", 5, 2.5);
        assert_eq!(ev.severity, 1);
        assert_eq!(ev.z_score_permille, 2500);
    }

    #[test]
    fn test_anomaly_to_edge_event_critical() {
        // z_score = 4.2 → severity = 2 (critical)
        let ev = digital_twin_anomaly_to_edge_event("twin-H", 0, 4.2);
        assert_eq!(ev.severity, 2);
    }

    #[test]
    fn test_hash_determinism() {
        let state = make_state("twin-Z", 999, vec![("x", 7.0)]);
        let r1 = digital_twin_state_to_db_record(&state);
        let r2 = digital_twin_state_to_db_record(&state);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.twin_id_hash, r2.twin_id_hash);
    }
}
