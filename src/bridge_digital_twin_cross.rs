//! Cross-domain bridges — ALICE-Digital-Twin ↔ Physics, Sync, Kinematics
//!
//! 5 bridges connecting digital twin state to physics simulation,
//! P2P synchronization, and kinematic motion intent.

use alice_digital_twin::{StateDiff, TimeSeries, TwinState};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: TwinState → Physics RigidBody init data ─────────────────

/// Physics rigid body initialization data derived from a digital twin state.
///
/// Extracts position (x, y, z) and mass from a `TwinState`'s property map
/// so the Physics layer can create a `RigidBody` without accessing the
/// full twin state.
pub struct TwinPhysicsRigidBody {
    /// FNV-1a hash over twin id, timestamp, position, mass bytes.
    pub content_hash: u64,
    /// Twin identifier.
    pub twin_id_hash: u64,
    /// Timestamp of the twin state snapshot.
    pub timestamp: u64,
    /// Position x (from property "pos_x", default 0.0).
    pub pos_x: f64,
    /// Position y (from property "pos_y", default 0.0).
    pub pos_y: f64,
    /// Position z (from property "pos_z", default 0.0).
    pub pos_z: f64,
    /// Mass (from property "mass", default 1.0).
    pub mass: f64,
    /// Whether the body is dynamic (mass > 0).
    pub is_dynamic: bool,
}

/// Convert a `TwinState` into physics rigid body init data.
#[inline]
#[must_use]
pub fn digital_twin_state_to_physics_rigidbody(state: &TwinState) -> TwinPhysicsRigidBody {
    let pos_x = state.get("pos_x").unwrap_or(0.0);
    let pos_y = state.get("pos_y").unwrap_or(0.0);
    let pos_z = state.get("pos_z").unwrap_or(0.0);
    let mass = state.get("mass").unwrap_or(1.0);
    let twin_id_hash = fnv1a(state.id.as_bytes());

    let mut key = [0u8; 48];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state.timestamp.to_le_bytes());
    key[16..24].copy_from_slice(&pos_x.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&pos_y.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&pos_z.to_bits().to_le_bytes());
    key[40..48].copy_from_slice(&mass.to_bits().to_le_bytes());

    TwinPhysicsRigidBody {
        content_hash: fnv1a(&key),
        twin_id_hash,
        timestamp: state.timestamp,
        pos_x,
        pos_y,
        pos_z,
        mass,
        is_dynamic: mass > 0.0,
    }
}

// ── Bridge 2: TwinState → Sync InputFrame ─────────────────────────────

/// Sync input frame derived from a digital twin state.
///
/// Maps a `TwinState` into ALICE-Sync `InputFrame` metadata so the Sync
/// layer can replicate twin state changes via event diffing.
pub struct TwinSyncInputFrame {
    /// FNV-1a hash over twin id, timestamp, property count bytes.
    pub content_hash: u64,
    /// Twin identifier hash.
    pub twin_id_hash: u64,
    /// Sync frame number (mapped from twin timestamp).
    pub frame: u32,
    /// Number of properties in the twin state.
    pub property_count: usize,
    /// Entity id for Sync (derived from twin id hash).
    pub entity_id: u32,
    /// Delta position x (from property "vel_x", default 0.0) scaled to Q16.16.
    pub delta_x: i32,
    /// Delta position y (from property "vel_y", default 0.0) scaled to Q16.16.
    pub delta_y: i32,
    /// Delta position z (from property "vel_z", default 0.0) scaled to Q16.16.
    pub delta_z: i32,
}

/// Convert a `TwinState` into a sync input frame.
#[inline]
#[must_use]
pub fn digital_twin_state_to_sync_input_frame(state: &TwinState) -> TwinSyncInputFrame {
    let twin_id_hash = fnv1a(state.id.as_bytes());
    let frame = (state.timestamp & 0xFFFF_FFFF) as u32;
    let property_count = state.properties.len();
    let entity_id = (twin_id_hash & 0xFFFF_FFFF) as u32;

    // 速度をQ16.16固定小数点に変換
    let vel_x = state.get("vel_x").unwrap_or(0.0);
    let vel_y = state.get("vel_y").unwrap_or(0.0);
    let vel_z = state.get("vel_z").unwrap_or(0.0);
    let delta_x = (vel_x * 65536.0) as i32;
    let delta_y = (vel_y * 65536.0) as i32;
    let delta_z = (vel_z * 65536.0) as i32;

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..16].copy_from_slice(&state.timestamp.to_le_bytes());
    key[16..24].copy_from_slice(&(property_count as u64).to_le_bytes());
    key[24..28].copy_from_slice(&delta_x.to_le_bytes());
    key[28..32].copy_from_slice(&delta_y.to_le_bytes());

    TwinSyncInputFrame {
        content_hash: fnv1a(&key),
        twin_id_hash,
        frame,
        property_count,
        entity_id,
        delta_x,
        delta_y,
        delta_z,
    }
}

// ── Bridge 3: TwinState → Kinematics Intent ───────────────────────────

/// Kinematics intent derived from a digital twin state.
///
/// Extracts target position and motion parameters from a `TwinState`
/// so the Kinematics layer can generate motion trajectories.
pub struct TwinKinematicsIntent {
    /// FNV-1a hash over twin id, target position, duration bytes.
    pub content_hash: u64,
    /// Twin identifier hash.
    pub twin_id_hash: u64,
    /// Target x (from property "target_x", default 0.0).
    pub target_x: f32,
    /// Target y (from property "target_y", default 0.0).
    pub target_y: f32,
    /// Target z (from property "target_z", default 0.0).
    pub target_z: f32,
    /// Duration in milliseconds (from property "duration_ms", default 100, clamped to 255).
    pub duration_ms: u8,
    /// Intent type discriminant: 0=Reach, 1=Point, 2=Grasp, 3=Release.
    pub intent_type: u8,
}

/// Convert a `TwinState` into a kinematics intent.
#[inline]
#[must_use]
pub fn digital_twin_state_to_kinematics_intent(state: &TwinState) -> TwinKinematicsIntent {
    let twin_id_hash = fnv1a(state.id.as_bytes());
    let target_x = state.get("target_x").unwrap_or(0.0) as f32;
    let target_y = state.get("target_y").unwrap_or(0.0) as f32;
    let target_z = state.get("target_z").unwrap_or(0.0) as f32;
    let duration_raw = state.get("duration_ms").unwrap_or(100.0) as u32;
    let duration_ms = if duration_raw > 255 { 255 } else { duration_raw as u8 };
    let intent_raw = state.get("intent_type").unwrap_or(0.0) as u8;
    let intent_type = match intent_raw {
        1 => 1, // Point
        2 => 2, // Grasp
        3 => 3, // Release
        _ => 0, // Reach
    };

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&twin_id_hash.to_le_bytes());
    key[8..12].copy_from_slice(&target_x.to_bits().to_le_bytes());
    key[12..16].copy_from_slice(&target_y.to_bits().to_le_bytes());
    key[16..20].copy_from_slice(&target_z.to_bits().to_le_bytes());
    key[20] = duration_ms;
    key[21] = intent_type;
    key[22..24].copy_from_slice(&[0, 0]);

    TwinKinematicsIntent {
        content_hash: fnv1a(&key),
        twin_id_hash,
        target_x,
        target_y,
        target_z,
        duration_ms,
        intent_type,
    }
}

// ── Bridge 4: StateDiff → Sync delta packet ───────────────────────────

/// Sync delta packet derived from a twin state diff.
///
/// Maps changed, added, and removed property counts into Sync-compatible
/// delta metadata so the Sync layer can decide whether to replicate a
/// full state snapshot or incremental events.
pub struct TwinSyncDelta {
    /// FNV-1a hash over changed/added/removed counts.
    pub content_hash: u64,
    /// Number of changed properties.
    pub changed_count: usize,
    /// Number of added properties.
    pub added_count: usize,
    /// Number of removed properties.
    pub removed_count: usize,
    /// Total delta entries.
    pub total_entries: usize,
    /// Estimated serialized size in bytes (24 per changed + 16 per added + 8 per removed).
    pub estimated_bytes: usize,
    /// Whether a full sync is recommended (total_entries > 64).
    pub recommend_full_sync: bool,
}

/// Convert a `StateDiff` into a sync delta packet.
#[inline]
#[must_use]
pub fn digital_twin_diff_to_sync_delta(diff: &StateDiff) -> TwinSyncDelta {
    let changed_count = diff.changed.len();
    let added_count = diff.added.len();
    let removed_count = diff.removed.len();
    let total_entries = changed_count + added_count + removed_count;
    let estimated_bytes = changed_count * 24 + added_count * 16 + removed_count * 8;
    let recommend_full_sync = total_entries > 64;

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&(changed_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(added_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&(removed_count as u64).to_le_bytes());

    TwinSyncDelta {
        content_hash: fnv1a(&key),
        changed_count,
        added_count,
        removed_count,
        total_entries,
        estimated_bytes,
        recommend_full_sync,
    }
}

// ── Bridge 5: TimeSeries anomaly → physics replay data ────────────────

/// Physics replay data derived from a time series anomaly detection.
///
/// When a `TimeSeries` detects anomalous values, this bridge extracts
/// the anomaly window so the Physics layer can replay the simulation
/// around the anomaly timestamp for root cause analysis.
pub struct TwinPhysicsReplay {
    /// FNV-1a hash over anomaly count, series length, threshold bytes.
    pub content_hash: u64,
    /// Number of anomalous points detected.
    pub anomaly_count: usize,
    /// Total number of points in the time series.
    pub series_length: usize,
    /// First anomaly index (or 0 if none).
    pub first_anomaly_index: usize,
    /// Anomaly threshold used for detection.
    pub threshold: f64,
    /// Whether a physics replay is recommended (anomaly_count > 0).
    pub replay_recommended: bool,
    /// Anomaly ratio (anomaly_count / series_length).
    pub anomaly_ratio: f64,
}

/// Extract physics replay data from a time series anomaly detection.
#[inline]
#[must_use]
pub fn digital_twin_timeseries_to_physics_replay(
    ts: &TimeSeries,
    threshold: f64,
) -> TwinPhysicsReplay {
    let anomalies = ts.detect_anomalies(threshold);
    let anomaly_count = anomalies.len();
    let series_length = ts.len();
    let first_anomaly_index = anomalies.first().copied().unwrap_or(0);
    let anomaly_ratio = if series_length > 0 {
        anomaly_count as f64 / series_length as f64
    } else {
        0.0
    };

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&(anomaly_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(series_length as u64).to_le_bytes());
    key[16..24].copy_from_slice(&threshold.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&(first_anomaly_index as u64).to_le_bytes());

    TwinPhysicsReplay {
        content_hash: fnv1a(&key),
        anomaly_count,
        series_length,
        first_anomaly_index,
        threshold,
        replay_recommended: anomaly_count > 0,
        anomaly_ratio,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_digital_twin::{compute_diff, TwinState};

    fn make_twin() -> TwinState {
        let mut s = TwinState::new("motor_1", 1000);
        s.set("pos_x", 1.0);
        s.set("pos_y", 2.0);
        s.set("pos_z", 3.0);
        s.set("mass", 5.0);
        s.set("vel_x", 0.5);
        s.set("vel_y", -0.3);
        s.set("vel_z", 0.1);
        s.set("target_x", 10.0);
        s.set("target_y", 5.0);
        s.set("target_z", 0.0);
        s.set("duration_ms", 200.0);
        s.set("intent_type", 2.0);
        s
    }

    // ── Bridge 1: TwinState → Physics RigidBody ─────────────────────

    #[test]
    fn test_twin_to_physics_rigidbody() {
        let s = make_twin();
        let rb = digital_twin_state_to_physics_rigidbody(&s);
        assert_ne!(rb.content_hash, 0);
        assert!((rb.pos_x - 1.0).abs() < 1e-10);
        assert!((rb.pos_y - 2.0).abs() < 1e-10);
        assert!((rb.pos_z - 3.0).abs() < 1e-10);
        assert!((rb.mass - 5.0).abs() < 1e-10);
        assert!(rb.is_dynamic);
    }

    #[test]
    fn test_twin_to_physics_rigidbody_deterministic() {
        let s = make_twin();
        let a = digital_twin_state_to_physics_rigidbody(&s);
        let b = digital_twin_state_to_physics_rigidbody(&s);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 2: TwinState → Sync InputFrame ───────────────────────

    #[test]
    fn test_twin_to_sync_input_frame() {
        let s = make_twin();
        let frame = digital_twin_state_to_sync_input_frame(&s);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.frame, 1000);
        assert!(frame.property_count > 0);
        assert_ne!(frame.entity_id, 0);
        // vel_x = 0.5 → delta_x = 0.5 * 65536 = 32768
        assert_eq!(frame.delta_x, 32768);
    }

    #[test]
    fn test_twin_to_sync_input_frame_deterministic() {
        let s = make_twin();
        let a = digital_twin_state_to_sync_input_frame(&s);
        let b = digital_twin_state_to_sync_input_frame(&s);
        assert_eq!(a.content_hash, b.content_hash);
    }

    // ── Bridge 3: TwinState → Kinematics Intent ─────────────────────

    #[test]
    fn test_twin_to_kinematics_intent() {
        let s = make_twin();
        let intent = digital_twin_state_to_kinematics_intent(&s);
        assert_ne!(intent.content_hash, 0);
        assert!((intent.target_x - 10.0).abs() < 0.01);
        assert!((intent.target_y - 5.0).abs() < 0.01);
        assert!((intent.target_z - 0.0).abs() < 0.01);
        assert_eq!(intent.duration_ms, 200);
        assert_eq!(intent.intent_type, 2); // Grasp
    }

    #[test]
    fn test_twin_to_kinematics_intent_clamp_duration() {
        let mut s = TwinState::new("arm", 0);
        s.set("duration_ms", 999.0);
        let intent = digital_twin_state_to_kinematics_intent(&s);
        assert_eq!(intent.duration_ms, 255);
    }

    // ── Bridge 4: StateDiff → Sync delta ────────────────────────────

    #[test]
    fn test_diff_to_sync_delta() {
        let mut old = TwinState::new("s1", 0);
        old.set("temp", 80.0);
        old.set("rpm", 3000.0);
        let mut new = TwinState::new("s1", 1);
        new.set("temp", 85.0);
        new.set("pressure", 1.0);
        let diff = compute_diff(&old, &new);
        let delta = digital_twin_diff_to_sync_delta(&diff);
        assert_ne!(delta.content_hash, 0);
        assert_eq!(delta.changed_count, 1); // temp changed
        assert_eq!(delta.added_count, 1); // pressure added
        assert_eq!(delta.removed_count, 1); // rpm removed
        assert_eq!(delta.total_entries, 3);
        assert!(!delta.recommend_full_sync);
    }

    #[test]
    fn test_diff_to_sync_delta_empty() {
        let s = TwinState::new("s1", 0);
        let diff = compute_diff(&s, &s);
        let delta = digital_twin_diff_to_sync_delta(&diff);
        assert_eq!(delta.total_entries, 0);
        assert_eq!(delta.estimated_bytes, 0);
        assert!(!delta.recommend_full_sync);
    }

    // ── Bridge 5: TimeSeries → Physics replay ───────────────────────

    #[test]
    fn test_timeseries_to_physics_replay_with_anomaly() {
        let mut ts = TimeSeries::new(200);
        for i in 0..100 {
            ts.push(i, 10.0);
        }
        ts.push(100, 1000.0); // 異常値
        let replay = digital_twin_timeseries_to_physics_replay(&ts, 2.0);
        assert_ne!(replay.content_hash, 0);
        assert!(replay.anomaly_count > 0);
        assert!(replay.replay_recommended);
        assert!(replay.anomaly_ratio > 0.0);
    }

    #[test]
    fn test_timeseries_to_physics_replay_no_anomaly() {
        let mut ts = TimeSeries::new(100);
        for i in 0..50 {
            ts.push(i, 10.0);
        }
        let replay = digital_twin_timeseries_to_physics_replay(&ts, 2.0);
        assert_eq!(replay.anomaly_count, 0);
        assert!(!replay.replay_recommended);
        assert!((replay.anomaly_ratio - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_timeseries_to_physics_replay_empty() {
        let ts = TimeSeries::new(100);
        let replay = digital_twin_timeseries_to_physics_replay(&ts, 2.0);
        assert_eq!(replay.anomaly_count, 0);
        assert_eq!(replay.series_length, 0);
        assert!(!replay.replay_recommended);
    }
}
