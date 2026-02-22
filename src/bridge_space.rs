//! Space bridges — ALICE-Space ↔ Analytics, DB, Edge
//!
//! 5 bridges connecting deep-space communication data to the ALICE ecosystem.

use alice_space::{CommLink, ControlDecision, MissionEvent, MissionPhase, ModelDifferential};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: CommLink → Analytics (link quality metrics) ───────────────

/// Communication link quality metrics for ALICE-Analytics ingestion.
pub struct SpaceAnalyticsLinkEvent {
    /// Content hash over source, target, distance, and latency bytes.
    pub content_hash: u64,
    /// Source body ID.
    pub source_id: u64,
    /// Target body ID.
    pub target_id: u64,
    /// Distance in km.
    pub distance_km: f64,
    /// One-way latency in seconds.
    pub latency_s: f64,
    /// Bandwidth in bits per second.
    pub bandwidth_bps: f64,
}

/// Convert a comm link into an analytics link quality event.
#[inline]
#[must_use]
pub fn space_link_to_analytics(link: &CommLink) -> SpaceAnalyticsLinkEvent {
    let latency = link.latency_s();
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&link.source_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&link.target_id.0.to_le_bytes());
    key[16..24].copy_from_slice(&link.distance_km.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&latency.to_bits().to_le_bytes());

    SpaceAnalyticsLinkEvent {
        content_hash: fnv1a(&key),
        source_id: link.source_id.0,
        target_id: link.target_id.0,
        distance_km: link.distance_km,
        latency_s: latency,
        bandwidth_bps: link.bandwidth_bps,
    }
}

// ── Bridge 2: ModelDifferential → Edge (bandwidth-efficient telemetry) ──

/// Model differential telemetry for ALICE-Edge ingestion.
pub struct SpaceEdgeDifferentialEvent {
    /// Content hash over sequence, param count, and byte size.
    pub content_hash: u64,
    /// Differential sequence number.
    pub sequence: u64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Number of parameter updates.
    pub param_count: usize,
    /// Wire size in bytes.
    pub byte_size: usize,
    /// Content hash from the differential itself.
    pub differential_hash: u64,
}

/// Convert a model differential into an edge telemetry event.
#[inline]
#[must_use]
pub fn space_differential_to_edge(diff: &ModelDifferential) -> SpaceEdgeDifferentialEvent {
    let byte_size = diff.byte_size();
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&diff.sequence.to_le_bytes());
    key[8..16].copy_from_slice(&(diff.param_updates.len() as u64).to_le_bytes());
    key[16..24].copy_from_slice(&(byte_size as u64).to_le_bytes());

    SpaceEdgeDifferentialEvent {
        content_hash: fnv1a(&key),
        sequence: diff.sequence,
        timestamp_ns: diff.timestamp_ns,
        param_count: diff.param_updates.len(),
        byte_size,
        differential_hash: diff.content_hash,
    }
}

// ── Bridge 3: MissionEvent → DB (mission event record) ─────────────────

/// Mission event record for ALICE-DB persistence.
pub struct SpaceDbMissionRecord {
    /// Content hash over sequence, phase, and delta-v bytes.
    pub content_hash: u64,
    /// Event sequence number.
    pub sequence: u32,
    /// Mission phase: 0-7 mapping.
    pub phase: u8,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Delta-v used in this event (km/s).
    pub delta_v_used: f64,
    /// Remaining fuel (kg).
    pub fuel_remaining_kg: f64,
    /// Content hash from the mission event itself.
    pub event_hash: u64,
}

/// Convert a mission event into a DB record.
#[inline]
#[must_use]
pub fn space_mission_to_db(event: &MissionEvent) -> SpaceDbMissionRecord {
    let phase_byte = match event.phase {
        MissionPhase::Launch => 0,
        MissionPhase::TransferOrbit => 1,
        MissionPhase::Insertion => 2,
        MissionPhase::Orbiting => 3,
        MissionPhase::Landing => 4,
        MissionPhase::Surface => 5,
        MissionPhase::Ascent => 6,
        MissionPhase::Return => 7,
    };

    let mut key = [0u8; 21];
    key[0..4].copy_from_slice(&event.sequence.to_le_bytes());
    key[4] = phase_byte;
    key[5..13].copy_from_slice(&event.timestamp_ns.to_le_bytes());
    key[13..21].copy_from_slice(&event.delta_v_used.to_bits().to_le_bytes());

    SpaceDbMissionRecord {
        content_hash: fnv1a(&key),
        sequence: event.sequence,
        phase: phase_byte,
        timestamp_ns: event.timestamp_ns,
        delta_v_used: event.delta_v_used,
        fuel_remaining_kg: event.fuel_remaining_kg,
        event_hash: event.content_hash,
    }
}

// ── Bridge 4: ControlDecision → Analytics (correction metrics) ──────────

/// Correction decision metrics for ALICE-Analytics ingestion.
pub struct SpaceAnalyticsCorrectionEvent {
    /// Content hash over thrust vector and burn duration bytes.
    pub content_hash: u64,
    /// Thrust vector components (normalized).
    pub thrust_vector: [f64; 3],
    /// Burn duration in seconds.
    pub burn_duration_s: f64,
    /// Decision confidence (0.0 to 1.0).
    pub confidence: f64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
}

/// Convert a control decision into an analytics correction event.
#[inline]
#[must_use]
pub fn space_correction_to_analytics(decision: &ControlDecision) -> SpaceAnalyticsCorrectionEvent {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&decision.thrust_vector[0].to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&decision.thrust_vector[1].to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&decision.thrust_vector[2].to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&decision.burn_duration_s.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&decision.confidence.to_bits().to_le_bytes());

    SpaceAnalyticsCorrectionEvent {
        content_hash: fnv1a(&key),
        thrust_vector: decision.thrust_vector,
        burn_duration_s: decision.burn_duration_s,
        confidence: decision.confidence,
        timestamp_ns: decision.timestamp_ns,
    }
}

// ── Bridge 5: CommLink → Cache (real-time link status) ─────────────────

/// Communication link status for ALICE-Cache real-time lookup.
pub struct SpaceCacheLinkStatus {
    /// Content hash over source, target, and latency bytes.
    pub content_hash: u64,
    /// Source body ID.
    pub source_id: u64,
    /// Target body ID.
    pub target_id: u64,
    /// One-way latency in seconds.
    pub latency_s: f64,
    /// Cache TTL: 10s if deep-space (latency > 60s), else 60s.
    pub ttl_secs: u32,
}

/// Convert a comm link into a cache status entry with adaptive TTL.
#[inline]
#[must_use]
pub fn space_link_to_cache(link: &CommLink) -> SpaceCacheLinkStatus {
    let latency = link.latency_s();
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&link.source_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&link.target_id.0.to_le_bytes());
    key[16..24].copy_from_slice(&latency.to_bits().to_le_bytes());
    // Branchless TTL: deep_space=1 → 60-50=10, near=0 → 60-0=60.
    let deep_space = (latency > 60.0) as u32;
    let ttl_secs = 60 - deep_space * 50;

    SpaceCacheLinkStatus {
        content_hash: fnv1a(&key),
        source_id: link.source_id.0,
        target_id: link.target_id.0,
        latency_s: latency,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_space::{
        compute_correction, CommLink, MissionLog, MissionPhase, ModelDifferential, SpacecraftState,
        TrajectoryModel,
    };

    #[test]
    fn test_link_to_analytics() {
        let link = CommLink::new(1, 2, 384400.0, 9600.0); // Earth-Moon
        let ev = space_link_to_analytics(&link);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.source_id, 1);
        assert_eq!(ev.target_id, 2);
        assert!((ev.latency_s - 1.28).abs() < 0.01);
    }

    #[test]
    fn test_differential_to_edge() {
        let mut diff = ModelDifferential::new(42, 1_000_000);
        diff.add_param("thrust_x", 3.14);
        diff.add_param("thrust_y", 2.71);
        diff.finalize();
        let ev = space_differential_to_edge(&diff);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.sequence, 42);
        assert_eq!(ev.param_count, 2);
        assert_eq!(ev.byte_size, 56); // 24 + 2*16
    }

    #[test]
    fn test_mission_to_db() {
        let mut log = MissionLog::new();
        log.log_event(MissionPhase::Launch, 1000, 2.5, 500.0);
        let events = log.events_for_phase(MissionPhase::Launch);
        let rec = space_mission_to_db(events[0]);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.sequence, 0);
        assert_eq!(rec.phase, 0); // Launch
        assert!((rec.delta_v_used - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_correction_to_analytics() {
        let state = SpacecraftState {
            position_km: [0.0, 0.0, 0.0],
            velocity_km_s: [0.0, 0.0, 0.0],
            timestamp_ns: 0,
            fuel_kg: 100.0,
        };
        let model = TrajectoryModel::new(vec![1000.0, 0.0, 0.0], 0, 1_000_000_000);
        let decision = compute_correction(&state, &model, 0);
        let ev = space_correction_to_analytics(&decision);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.thrust_vector[0] > 0.0);
        assert!(ev.burn_duration_s > 0.0);
    }

    #[test]
    fn test_link_to_cache_near() {
        let link = CommLink::new(1, 2, 384400.0, 9600.0); // ~1.28s latency
        let entry = space_link_to_cache(&link);
        assert_eq!(entry.ttl_secs, 60); // near (< 60s)
    }

    #[test]
    fn test_link_to_cache_deep_space() {
        let link = CommLink::new(1, 2, 55_700_000.0, 100.0); // Mars ~186s
        let entry = space_link_to_cache(&link);
        assert_eq!(entry.ttl_secs, 10); // deep space (> 60s)
    }

    #[test]
    fn test_hash_determinism() {
        let link = CommLink::new(1, 2, 1000.0, 9600.0);
        let e1 = space_link_to_analytics(&link);
        let e2 = space_link_to_analytics(&link);
        assert_eq!(e1.content_hash, e2.content_hash);
    }
}
