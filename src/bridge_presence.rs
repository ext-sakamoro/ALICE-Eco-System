//! Presence bridges — ALICE-Presence ↔ Analytics, DB, Edge, Cache
//!
//! 5 bridges connecting presence protocol to the ALICE ecosystem.

use alice_presence::{CrossingRecord, CrossingStatus, PresenceEvent, ProximityProof};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: CrossingRecord → DB (permanent record) ─────────────────

/// Crossing DB record for ALICE-DB persistence.
pub struct PresenceDbCrossingRecord {
    /// Content hash over event bytes, proof responses, proximity hash.
    pub content_hash: u64,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Crossing timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Whether both ZKPs verified and proximity confirmed.
    pub fully_verified: bool,
    /// Crossing status discriminant: 0=Initiated, 1=Mutual, 2=Verified, 3=Recorded, 4=Revoked.
    pub status: u8,
    /// Vivaldi distance between the two parties.
    pub distance: f64,
}

/// Convert a crossing record into a DB record.
#[inline]
pub fn presence_crossing_to_db(record: &CrossingRecord) -> PresenceDbCrossingRecord {
    let status_byte = match record.status() {
        CrossingStatus::Initiated => 0u8,
        CrossingStatus::Mutual => 1,
        CrossingStatus::Verified => 2,
        CrossingStatus::Recorded => 3,
        CrossingStatus::Revoked => 4,
    };

    let mut key = [0u8; 33];
    key[0..4].copy_from_slice(&record.event.party_a_id.to_le_bytes());
    key[4..8].copy_from_slice(&record.event.party_b_id.to_le_bytes());
    key[8..16].copy_from_slice(&record.event.timestamp_ns.to_le_bytes());
    key[16] = status_byte;
    key[17..25].copy_from_slice(&record.proximity.distance.to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&record.content_hash.to_le_bytes());

    PresenceDbCrossingRecord {
        content_hash: fnv1a(&key),
        party_a_id: record.event.party_a_id,
        party_b_id: record.event.party_b_id,
        timestamp_ns: record.event.timestamp_ns,
        fully_verified: record.is_fully_verified(),
        status: status_byte,
        distance: record.proximity.distance,
    }
}

// ── Bridge 2: CrossingRecord → Analytics (crossing metrics) ──────────

/// Crossing analytics event for ALICE-Analytics ingestion.
pub struct PresenceAnalyticsCrossingEvent {
    /// Content hash over party IDs, distance, status bytes.
    pub content_hash: u64,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Vivaldi distance.
    pub distance: f64,
    /// Whether the crossing is fully verified.
    pub fully_verified: bool,
    /// Whether party A's ZKP was verified.
    pub proof_a_valid: bool,
    /// Whether party B's ZKP was verified.
    pub proof_b_valid: bool,
}

/// Convert a crossing record into an analytics event.
#[inline]
pub fn presence_crossing_to_analytics(record: &CrossingRecord) -> PresenceAnalyticsCrossingEvent {
    let mut key = [0u8; 19];
    key[0..4].copy_from_slice(&record.event.party_a_id.to_le_bytes());
    key[4..8].copy_from_slice(&record.event.party_b_id.to_le_bytes());
    key[8..16].copy_from_slice(&record.proximity.distance.to_bits().to_le_bytes());
    key[16] = record.proof_a.verified as u8;
    key[17] = record.proof_b.verified as u8;
    key[18] = record.is_fully_verified() as u8;

    PresenceAnalyticsCrossingEvent {
        content_hash: fnv1a(&key),
        party_a_id: record.event.party_a_id,
        party_b_id: record.event.party_b_id,
        distance: record.proximity.distance,
        fully_verified: record.is_fully_verified(),
        proof_a_valid: record.proof_a.verified,
        proof_b_valid: record.proof_b.verified,
    }
}

// ── Bridge 3: PresenceEvent → Edge (telemetry event) ─────────────────

/// Presence telemetry for ALICE-Edge ingestion.
pub struct PresenceEdgeEvent {
    /// Content hash over event bytes.
    pub content_hash: u64,
    /// Event type byte (0x50 = 'P').
    pub event_type: u8,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Wire size in bytes (always 18).
    pub wire_bytes: usize,
}

/// Convert a presence event into an edge telemetry event.
#[inline]
pub fn presence_event_to_edge(event: &PresenceEvent) -> PresenceEdgeEvent {
    let bytes = event.to_bytes();

    PresenceEdgeEvent {
        content_hash: fnv1a(&bytes),
        event_type: event.event_type,
        party_a_id: event.party_a_id,
        party_b_id: event.party_b_id,
        timestamp_ns: event.timestamp_ns,
        wire_bytes: PresenceEvent::byte_size(),
    }
}

// ── Bridge 4: PresenceEvent → Cache (quick presence lookup) ──────────

/// Presence cache entry for ALICE-Cache real-time lookup.
pub struct PresenceCacheEvent {
    /// Content hash over party IDs and flags bytes.
    pub content_hash: u64,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Whether both parties are mutually present.
    pub is_mutual: bool,
    /// Cache TTL: 5s if not verified, 60s if verified.
    pub ttl_secs: u32,
}

/// Convert a presence event into a cache entry with adaptive TTL.
#[inline]
pub fn presence_event_to_cache(event: &PresenceEvent) -> PresenceCacheEvent {
    let mut key = [0u8; 9];
    key[0..4].copy_from_slice(&event.party_a_id.to_le_bytes());
    key[4..8].copy_from_slice(&event.party_b_id.to_le_bytes());
    key[8] = event.flags;

    // Branchless TTL: not_verified=1 → 60-55=5, verified=0 → 60-0=60.
    let not_verified = (!event.is_verified()) as u32;
    let ttl_secs = 60 - not_verified * 55;

    PresenceCacheEvent {
        content_hash: fnv1a(&key),
        party_a_id: event.party_a_id,
        party_b_id: event.party_b_id,
        is_mutual: event.is_mutual(),
        ttl_secs,
    }
}

// ── Bridge 5: ProximityProof → Analytics (distance metrics) ──────────

/// Proximity distance metrics for ALICE-Analytics ingestion.
pub struct PresenceAnalyticsProximityEvent {
    /// Content hash over distance, threshold, is_proximate bytes.
    pub content_hash: u64,
    /// Vivaldi distance.
    pub distance: f64,
    /// Proximity threshold.
    pub threshold: f64,
    /// Whether the distance is within threshold.
    pub is_proximate: bool,
    /// Normalized distance (distance / threshold).
    pub normalized_distance: f64,
}

/// Convert a proximity proof into an analytics event.
#[inline]
pub fn presence_proximity_to_analytics(proof: &ProximityProof) -> PresenceAnalyticsProximityEvent {
    let norm = if proof.threshold > 0.0 { proof.distance / proof.threshold } else { 0.0 };

    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&proof.distance.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&proof.threshold.to_bits().to_le_bytes());
    key[16] = proof.is_proximate as u8;

    PresenceAnalyticsProximityEvent {
        content_hash: fnv1a(&key),
        distance: proof.distance,
        threshold: proof.threshold,
        is_proximate: proof.is_proximate,
        normalized_distance: norm,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_presence::{VivaldiCoord, IdentityCommitment, ZkProof, ProximityProof, PresenceEvent, CrossingRecord, PresenceConfig, PartyInfo, execute_presence_protocol};

    fn make_crossing() -> CrossingRecord {
        let a = PartyInfo::new(VivaldiCoord::new(0.0, 0.0), 42, 1);
        let b = PartyInfo::new(VivaldiCoord::new(1.0, 0.0), 99, 2);
        let cfg = PresenceConfig::default();
        execute_presence_protocol(&a, &b, 1_000_000, &cfg).unwrap()
    }

    #[test]
    fn test_crossing_to_db() {
        let record = make_crossing();
        let db = presence_crossing_to_db(&record);
        assert_ne!(db.content_hash, 0);
        assert_eq!(db.party_a_id, 1);
        assert_eq!(db.party_b_id, 2);
        assert_eq!(db.timestamp_ns, 1_000_000);
        assert!(db.fully_verified);
        assert!(db.distance > 0.0);
    }

    #[test]
    fn test_crossing_to_analytics() {
        let record = make_crossing();
        let ev = presence_crossing_to_analytics(&record);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.party_a_id, 1);
        assert_eq!(ev.party_b_id, 2);
        assert!(ev.fully_verified);
        assert!(ev.proof_a_valid);
        assert!(ev.proof_b_valid);
    }

    #[test]
    fn test_event_to_edge() {
        let mut event = PresenceEvent::new(10, 20, 5000);
        event.set_mutual();
        let ev = presence_event_to_edge(&event);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_type, 0x50);
        assert_eq!(ev.party_a_id, 10);
        assert_eq!(ev.party_b_id, 20);
        assert_eq!(ev.wire_bytes, 18);
    }

    #[test]
    fn test_event_to_cache_not_verified() {
        let event = PresenceEvent::new(1, 2, 0);
        let entry = presence_event_to_cache(&event);
        assert_eq!(entry.ttl_secs, 5); // not verified → 5s
        assert!(!entry.is_mutual);
    }

    #[test]
    fn test_event_to_cache_verified() {
        let mut event = PresenceEvent::new(1, 2, 0);
        event.set_verified();
        let entry = presence_event_to_cache(&event);
        assert_eq!(entry.ttl_secs, 60); // verified → 60s
    }

    #[test]
    fn test_proximity_to_analytics_proximate() {
        let a = VivaldiCoord::new(0.0, 0.0);
        let b = VivaldiCoord::new(3.0, 4.0); // distance = 5.0
        let proof = ProximityProof::prove(&a, &b, 10.0);
        let ev = presence_proximity_to_analytics(&proof);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.is_proximate);
        assert!((ev.distance - 5.0).abs() < 1e-10);
        assert!((ev.normalized_distance - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_proximity_to_analytics_not_proximate() {
        let a = VivaldiCoord::new(0.0, 0.0);
        let b = VivaldiCoord::new(100.0, 0.0);
        let proof = ProximityProof::prove(&a, &b, 10.0);
        let ev = presence_proximity_to_analytics(&proof);
        assert!(!ev.is_proximate);
        assert!(ev.normalized_distance > 1.0);
    }

    #[test]
    fn test_hash_determinism() {
        let record = make_crossing();
        let d1 = presence_crossing_to_db(&record);
        let d2 = presence_crossing_to_db(&record);
        assert_eq!(d1.content_hash, d2.content_hash);
    }
}
