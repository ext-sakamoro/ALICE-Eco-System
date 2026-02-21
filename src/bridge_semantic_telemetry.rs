//! Semantic Telemetry bridges — ALICE-Semantic-Telemetry ↔ Analytics, DB, Edge, ML, View
//!
//! 5 bridges connecting semantic telemetry to the ALICE ecosystem.

use alice_semantic_telemetry::{SemanticEvent, SemanticRing, EventKind, Severity};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
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
pub fn telemetry_to_analytics_snapshot(ring: &SemanticRing, window_start_ns: u64, window_end_ns: u64) -> TelemetryAnalyticsSnapshot {
    let kind_counts = ring.count_by_kind();
    let total = kind_counts.iter().sum::<u64>();
    let warn_and_above = ring.iter()
        .filter(|e| e.severity >= Severity::Warn)
        .count() as u64;
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
pub fn telemetry_event_to_db_record(event: &SemanticEvent) -> TelemetryDbRecord {
    let hash_data = [
        event.timestamp_ns.to_le_bytes(),
        event.source_id.to_le_bytes(),
    ].concat();
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
    ring.drain().iter().map(telemetry_event_to_db_record).collect()
}

// ── Bridge 3: Edge → Semantic Telemetry (sensor event injection) ──────────

/// Create a semantic event from an edge sensor reading.
///
/// Maps sensor state changes to StateTransition events
/// and threshold crossings to ThresholdCrossing events.
#[inline]
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
pub fn telemetry_to_view_summary(ring: &SemanticRing, window_duration_secs: f32) -> TelemetryViewSummary {
    let kind_counts = ring.count_by_kind();
    let total = kind_counts.iter().sum::<u64>();
    let warnings = ring.iter()
        .filter(|e| e.severity >= Severity::Warn)
        .count() as u64;
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
