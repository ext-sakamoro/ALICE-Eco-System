//! Semantic Telemetry bridges — ALICE-Semantic-Telemetry ↔ Analytics, DB, Edge, ML, View,
//!                              Physics, Sync, Motion, RTOS
//!
//! 9 bridges connecting semantic telemetry to the ALICE ecosystem.

use alice_semantic_telemetry::{EventKind, SemanticEvent, SemanticRing, Severity};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Semantic Telemetry → Analytics (event aggregation) ──────────

/// Aggregated semantic telemetry snapshot for ALICE-Analytics.
///
/// Summarizes a batch of semantic events into counts and rates
/// suitable for HyperLogLog/DDSketch ingestion.
pub struct TelemetryAnalyticsSnapshot {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Total events in this snapshot window.
    pub total_events: u64,
    /// Count per event kind (6 kinds).
    pub kind_counts: [u64; 6],
    /// Events at Warn or above.
    pub warn_and_above: u64,
    /// Anomaly detection events.
    pub anomaly_count: u64,
    /// Snapshot window start timestamp (ns).
    pub window_start_ns: u64,
    /// Snapshot window end timestamp (ns).
    pub window_end_ns: u64,
}

/// Create an analytics snapshot from the semantic ring.
#[inline]
#[must_use]
pub fn telemetry_to_analytics_snapshot(
    ring: &SemanticRing,
    window_start_ns: u64,
    window_end_ns: u64,
) -> TelemetryAnalyticsSnapshot {
    let kind_counts = ring.count_by_kind();
    let total = kind_counts.iter().sum::<u64>();
    let warn_and_above = ring.iter().filter(|e| e.severity >= Severity::Warn).count() as u64;
    let anomaly_count = kind_counts[EventKind::AnomalyDetected as usize];
    let hash_data = [total.to_le_bytes(), window_start_ns.to_le_bytes()].concat();
    TelemetryAnalyticsSnapshot {
        content_hash: fnv1a(&hash_data),
        total_events: total,
        kind_counts,
        warn_and_above,
        anomaly_count,
        window_start_ns,
        window_end_ns,
    }
}

// ── Bridge 2: Semantic Telemetry → DB (event persistence) ─────────────────

/// Serialized semantic event for ALICE-DB persistence.
///
/// Flattened representation suitable for segment-based time-series storage.
pub struct TelemetryDbRecord {
    /// Content hash (primary key).
    pub content_hash: u64,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Source subsystem hash.
    pub source_id: u64,
    /// Event kind as u8.
    pub kind: u8,
    /// Severity as u8.
    pub severity: u8,
    /// Primary payload.
    pub payload: u64,
    /// Secondary payload.
    pub payload2: u64,
}

/// Convert a semantic event to a DB record.
#[inline]
#[must_use]
pub fn telemetry_event_to_db_record(event: &SemanticEvent) -> TelemetryDbRecord {
    let hash_data = [
        event.timestamp_ns.to_le_bytes(),
        event.source_id.to_le_bytes(),
    ]
    .concat();
    TelemetryDbRecord {
        content_hash: fnv1a(&hash_data),
        timestamp_ns: event.timestamp_ns,
        source_id: event.source_id,
        kind: event.kind as u8,
        severity: event.severity as u8,
        payload: event.payload,
        payload2: event.payload2,
    }
}

/// Batch convert: drain the ring and produce DB records.
#[inline]
pub fn telemetry_drain_to_db_records(ring: &mut SemanticRing) -> Vec<TelemetryDbRecord> {
    ring.drain()
        .iter()
        .map(telemetry_event_to_db_record)
        .collect()
}

// ── Bridge 3: Edge → Semantic Telemetry (sensor event injection) ──────────

/// Create a semantic event from an edge sensor reading.
///
/// Maps sensor state changes to `StateTransition` events
/// and threshold crossings to `ThresholdCrossing` events.
#[inline]
#[must_use]
pub fn edge_sensor_to_semantic_event(
    timestamp_ns: u64,
    sensor_id: u64,
    kind: EventKind,
    value: u64,
) -> SemanticEvent {
    SemanticEvent {
        timestamp_ns,
        source_id: sensor_id,
        kind,
        severity: Severity::Info,
        payload: value,
        payload2: 0,
    }
}

// ── Bridge 4: ML → Semantic Telemetry (anomaly event injection) ───────────

/// Create a semantic event from an ML anomaly detection result.
#[inline]
#[must_use]
pub fn ml_anomaly_to_semantic_event(
    timestamp_ns: u64,
    model_id: u64,
    anomaly_score: f32,
    severity: Severity,
) -> SemanticEvent {
    SemanticEvent {
        timestamp_ns,
        source_id: model_id,
        kind: EventKind::AnomalyDetected,
        severity,
        payload: anomaly_score.to_bits() as u64,
        payload2: 0,
    }
}

// ── Bridge 5: Semantic Telemetry → View (dashboard overlay) ───────────────

/// Dashboard-ready telemetry summary for ALICE-View overlay.
///
/// Pre-computed rates and counts suitable for immediate rendering
/// without further computation in the UI loop.
pub struct TelemetryViewSummary {
    /// Total events in the current window.
    pub total_events: u64,
    /// State transitions count.
    pub state_transitions: u64,
    /// Classification events count.
    pub classifications: u64,
    /// Anomalies detected count.
    pub anomalies: u64,
    /// Warnings and above count.
    pub warnings: u64,
    /// Event rate (events per second).
    pub events_per_sec: f32,
}

/// Create a view summary from the semantic ring.
#[inline]
#[must_use]
pub fn telemetry_to_view_summary(
    ring: &SemanticRing,
    window_duration_secs: f32,
) -> TelemetryViewSummary {
    let kind_counts = ring.count_by_kind();
    let total = kind_counts.iter().sum::<u64>();
    let warnings = ring.iter().filter(|e| e.severity >= Severity::Warn).count() as u64;
    let inv_duration = if window_duration_secs > 0.0 {
        1.0 / window_duration_secs
    } else {
        0.0
    };
    TelemetryViewSummary {
        total_events: total,
        state_transitions: kind_counts[EventKind::StateTransition as usize],
        classifications: kind_counts[EventKind::Classification as usize],
        anomalies: kind_counts[EventKind::AnomalyDetected as usize],
        warnings,
        events_per_sec: total as f32 * inv_duration,
    }
}

// ── Bridge 6: Physics → Semantic Telemetry (collision / state events) ─────

/// Create a semantic event from a rigid-body collision impulse.
///
/// Maps a collision occurrence to a `StateTransition` event.
/// `payload` encodes the body identity hash; `payload2` encodes the
/// collision impulse magnitude as f32 bits for downstream filtering.
///
/// Severity is `Warn` when impulse exceeds 100 simulation units
/// (potential structural event), `Info` otherwise.
#[inline]
#[must_use]
pub fn physics_collision_to_semantic_event(
    timestamp_ns: u64,
    body_hash: u64,
    collision_impulse: f32,
) -> SemanticEvent {
    // Threshold: impulse > 100.0 → Warn, otherwise Info.
    let severity = if collision_impulse > 100.0 {
        Severity::Warn
    } else {
        Severity::Info
    };
    SemanticEvent {
        timestamp_ns,
        source_id: body_hash,
        kind: EventKind::StateTransition,
        severity,
        payload: body_hash,
        payload2: collision_impulse.to_bits() as u64,
    }
}

/// Create a semantic event from a rigid-body discrete state change.
///
/// Encodes `(from_state << 32) | to_state` in `payload`, matching the
/// `StateTransition` payload convention documented in `SemanticEvent`.
/// `payload2` carries the body identity hash for cross-subsystem correlation.
#[inline]
#[must_use]
pub fn physics_state_change_to_semantic_event(
    timestamp_ns: u64,
    body_hash: u64,
    from_state: u32,
    to_state: u32,
) -> SemanticEvent {
    let payload = ((from_state as u64) << 32) | (to_state as u64);
    SemanticEvent {
        timestamp_ns,
        source_id: body_hash,
        kind: EventKind::StateTransition,
        severity: Severity::Info,
        payload,
        payload2: body_hash,
    }
}

// ── Bridge 7: Sync → Semantic Telemetry (lockstep frame events) ───────────

/// Create a semantic event from a lockstep sync frame.
///
/// Hash mismatches (desync) are emitted as `AnomalyDetected` at `Error`
/// severity. Clean frames are emitted as `StageComplete` at `Info`.
/// `payload` carries the frame number; `payload2` carries the session hash.
#[inline]
#[must_use]
pub fn sync_frame_to_semantic_event(
    timestamp_ns: u64,
    session_hash: u64,
    frame_number: u64,
    hash_mismatch: bool,
) -> SemanticEvent {
    // Desync: treat as anomaly at Error severity.
    // Clean frame: stage completion at Info.
    let (kind, severity) = if hash_mismatch {
        (EventKind::AnomalyDetected, Severity::Error)
    } else {
        (EventKind::StageComplete, Severity::Info)
    };
    SemanticEvent {
        timestamp_ns,
        source_id: session_hash,
        kind,
        severity,
        payload: frame_number,
        payload2: session_hash,
    }
}

// ── Bridge 8: Motion → Semantic Telemetry (trajectory events) ─────────────

/// Create a semantic event from a motion trajectory plan dispatch.
///
/// Emits a `DataFlow` event annotating that a trajectory has been handed
/// from the Motion subsystem to downstream consumers.
/// `payload` carries `plan_hash`; `payload2` packs `(segments << 32) | duration_ms`
/// for compact downstream decoding without heap allocation.
#[inline]
#[must_use]
pub fn motion_trajectory_to_semantic_event(
    timestamp_ns: u64,
    plan_hash: u64,
    segments: u32,
    duration_ms: u32,
) -> SemanticEvent {
    // Pack segments and duration into a single u64: high word = segments, low word = duration_ms.
    let payload2 = ((segments as u64) << 32) | (duration_ms as u64);
    SemanticEvent {
        timestamp_ns,
        source_id: plan_hash,
        kind: EventKind::DataFlow,
        severity: Severity::Info,
        payload: plan_hash,
        payload2,
    }
}

// ── Bridge 9: RTOS → Semantic Telemetry (task scheduling events) ──────────

/// Create a semantic event from an RTOS task execution result.
///
/// Missed deadlines are emitted as `ThresholdCrossing` at `Warn` severity.
/// Met deadlines are emitted as `StageComplete` at `Trace` severity
/// (high-frequency, intentionally low noise on the ring).
/// `payload` carries `task_hash`; `payload2` carries `slack_us` as u64.
#[inline]
#[must_use]
pub fn rtos_task_to_semantic_event(
    timestamp_ns: u64,
    task_hash: u64,
    deadline_met: bool,
    slack_us: i32,
) -> SemanticEvent {
    // Missed deadline → threshold crossing at Warn.
    // Met deadline   → stage complete at Trace (low-noise path).
    let (kind, severity) = if deadline_met {
        (EventKind::StageComplete, Severity::Trace)
    } else {
        (EventKind::ThresholdCrossing, Severity::Warn)
    };
    SemanticEvent {
        timestamp_ns,
        source_id: task_hash,
        kind,
        severity,
        payload: task_hash,
        payload2: slack_us as i64 as u64,
    }
}

// ── Tests for bridges 6–9 ─────────────────────────────────────────────────

#[cfg(test)]
mod bridge_physics_sync_motion_rtos_tests {
    use super::*;

    // ── Bridge 6: Physics collision event ───────────────────────────────

    #[test]
    fn physics_collision_low_impulse_is_info() {
        let ev = physics_collision_to_semantic_event(1_000, 0xDEAD_BEEF, 10.0);
        assert_eq!(ev.kind, EventKind::StateTransition);
        assert_eq!(ev.severity, Severity::Info);
        assert_eq!(ev.source_id, 0xDEAD_BEEF);
        assert_eq!(ev.payload, 0xDEAD_BEEF);
        // payload2 holds impulse as f32 bits.
        assert_eq!(ev.payload2, 10.0f32.to_bits() as u64);
        assert_eq!(ev.timestamp_ns, 1_000);
    }

    #[test]
    fn physics_collision_high_impulse_is_warn() {
        let ev = physics_collision_to_semantic_event(2_000, 0xCAFE, 500.0);
        assert_eq!(ev.severity, Severity::Warn);
        assert_eq!(ev.payload2, 500.0f32.to_bits() as u64);
    }

    #[test]
    fn physics_collision_boundary_impulse_exactly_100_is_info() {
        // 100.0 is not strictly greater than 100.0 → Info.
        let ev = physics_collision_to_semantic_event(0, 1, 100.0);
        assert_eq!(ev.severity, Severity::Info);
    }

    #[test]
    fn physics_state_change_encodes_states_correctly() {
        let from: u32 = 1; // e.g., Active
        let to: u32 = 3; // e.g., Sleeping
        let ev = physics_state_change_to_semantic_event(5_000, 0xABCD, from, to);
        assert_eq!(ev.kind, EventKind::StateTransition);
        assert_eq!(ev.severity, Severity::Info);
        // payload = (from << 32) | to.
        let expected_payload = ((from as u64) << 32) | (to as u64);
        assert_eq!(ev.payload, expected_payload);
        // payload2 = body_hash.
        assert_eq!(ev.payload2, 0xABCD);
        assert_eq!(ev.source_id, 0xABCD);
    }

    #[test]
    fn physics_state_change_different_states_differ() {
        let ev1 = physics_state_change_to_semantic_event(0, 1, 0, 1);
        let ev2 = physics_state_change_to_semantic_event(0, 1, 1, 2);
        assert_ne!(ev1.payload, ev2.payload);
    }

    // ── Bridge 7: Sync lockstep frame event ─────────────────────────────

    #[test]
    fn sync_frame_clean_is_stage_complete_info() {
        let session_hash = 0x1111_2222_3333_4444u64;
        let ev = sync_frame_to_semantic_event(10_000, session_hash, 42, false);
        assert_eq!(ev.kind, EventKind::StageComplete);
        assert_eq!(ev.severity, Severity::Info);
        assert_eq!(ev.payload, 42);
        assert_eq!(ev.payload2, session_hash);
        assert_eq!(ev.source_id, session_hash);
    }

    #[test]
    fn sync_frame_hash_mismatch_is_anomaly_error() {
        let session_hash = 0xDEAD_C0DE_CAFE_BABEu64;
        let ev = sync_frame_to_semantic_event(20_000, session_hash, 99, true);
        assert_eq!(ev.kind, EventKind::AnomalyDetected);
        assert_eq!(ev.severity, Severity::Error);
        assert_eq!(ev.payload, 99);
        assert_eq!(ev.payload2, session_hash);
    }

    #[test]
    fn sync_frame_zero_frame_number_clean() {
        let ev = sync_frame_to_semantic_event(0, 0, 0, false);
        assert_eq!(ev.kind, EventKind::StageComplete);
        assert_eq!(ev.payload, 0);
    }

    // ── Bridge 8: Motion trajectory event ───────────────────────────────

    #[test]
    fn motion_trajectory_event_encodes_correctly() {
        let plan_hash = 0xFEED_FACE_DEAD_BEEFu64;
        let ev = motion_trajectory_to_semantic_event(30_000, plan_hash, 8, 250);
        assert_eq!(ev.kind, EventKind::DataFlow);
        assert_eq!(ev.severity, Severity::Info);
        assert_eq!(ev.source_id, plan_hash);
        assert_eq!(ev.payload, plan_hash);
        // payload2 = (segments << 32) | duration_ms.
        let expected_payload2 = ((8u64) << 32) | 250u64;
        assert_eq!(ev.payload2, expected_payload2);
        assert_eq!(ev.timestamp_ns, 30_000);
    }

    #[test]
    fn motion_trajectory_segments_in_payload2_high_word() {
        let ev = motion_trajectory_to_semantic_event(0, 1, 16, 500);
        let segments_decoded = (ev.payload2 >> 32) as u32;
        let duration_decoded = ev.payload2 as u32;
        assert_eq!(segments_decoded, 16);
        assert_eq!(duration_decoded, 500);
    }

    #[test]
    fn motion_trajectory_different_plans_differ() {
        let ev1 = motion_trajectory_to_semantic_event(0, 0xAAAA, 4, 100);
        let ev2 = motion_trajectory_to_semantic_event(0, 0xBBBB, 4, 100);
        assert_ne!(ev1.source_id, ev2.source_id);
        assert_ne!(ev1.payload, ev2.payload);
    }

    // ── Bridge 9: RTOS task scheduling event ────────────────────────────

    #[test]
    fn rtos_task_deadline_met_is_stage_complete_trace() {
        let task_hash = 0x1234_5678_9ABC_DEF0u64;
        let ev = rtos_task_to_semantic_event(40_000, task_hash, true, 50);
        assert_eq!(ev.kind, EventKind::StageComplete);
        assert_eq!(ev.severity, Severity::Trace);
        assert_eq!(ev.source_id, task_hash);
        assert_eq!(ev.payload, task_hash);
        // slack_us = 50 stored as u64 (positive, no sign extension issues).
        assert_eq!(ev.payload2, 50u64);
        assert_eq!(ev.timestamp_ns, 40_000);
    }

    #[test]
    fn rtos_task_deadline_missed_is_threshold_crossing_warn() {
        let task_hash = 0xDEAD_C0DEu64;
        let ev = rtos_task_to_semantic_event(50_000, task_hash, false, -20);
        assert_eq!(ev.kind, EventKind::ThresholdCrossing);
        assert_eq!(ev.severity, Severity::Warn);
        // slack_us is negative: stored as two's-complement u64.
        assert_eq!(ev.payload2 as i64 as i32, -20);
    }

    #[test]
    fn rtos_task_zero_slack_deadline_met_is_trace() {
        let ev = rtos_task_to_semantic_event(0, 1, true, 0);
        assert_eq!(ev.severity, Severity::Trace);
        assert_eq!(ev.payload2, 0);
    }

    #[test]
    fn rtos_task_different_hashes_differ() {
        let ev1 = rtos_task_to_semantic_event(0, 0xAAAA, true, 10);
        let ev2 = rtos_task_to_semantic_event(0, 0xBBBB, true, 10);
        assert_ne!(ev1.source_id, ev2.source_id);
        assert_ne!(ev1.payload, ev2.payload);
    }
}
