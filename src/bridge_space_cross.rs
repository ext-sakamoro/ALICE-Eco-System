//! Cross-domain bridges — ALICE-Space ↔ Codec, Crypto, Sync, ASP
//!
//! 4 bridges connecting deep-space communication and mission data to
//! codec compression, encrypted channels, sync events, and streaming payloads.

use alice_space::{CommLink, MissionEvent, MissionPhase, ModelDifferential};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: ModelDifferential → Codec (compressed frame metadata) ──────

/// Compressed frame metadata derived from a space model differential.
///
/// Encodes parameter update deltas into codec-friendly bit-width estimates
/// so the codec layer can choose an optimal variable-length encoding.
pub struct SpaceCodecFrame {
    /// FNV-1a hash over timestamp, param count, and byte size.
    pub content_hash: u64,
    /// Timestamp of the differential (nanoseconds).
    pub timestamp_ns: u64,
    /// Number of parameter updates in the differential.
    pub param_count: u32,
    /// Maximum absolute delta across all parameter updates.
    pub max_delta: f64,
    /// Bit-width estimate: `ceil(log2(|max_delta`| * 1000 + 1)), min 1.
    pub delta_bits: u32,
    /// Estimated wire bytes: (`delta_bits` * `param_count` + 7) / 8.
    pub estimated_bytes: usize,
}

/// Convert a model differential into codec compressed frame metadata.
#[inline]
pub fn space_differential_to_codec_frame(diff: &ModelDifferential) -> SpaceCodecFrame {
    let max_delta = diff
        .param_updates
        .iter()
        .map(|&(_, v)| v.abs())
        .fold(0.0f64, f64::max);

    let delta_bits = (max_delta.mul_add(1000.0, 1.0).log2().ceil() as u32).max(1);
    let total_bits = delta_bits as usize * diff.param_updates.len();
    let estimated_bytes = total_bits.div_ceil(8);

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&diff.timestamp_ns.to_le_bytes());
    key[8..12].copy_from_slice(&(diff.param_updates.len() as u32).to_le_bytes());
    key[12..16].copy_from_slice(&(diff.byte_size() as u32).to_le_bytes());
    key[16..24].copy_from_slice(&max_delta.to_bits().to_le_bytes());

    SpaceCodecFrame {
        content_hash: fnv1a(&key),
        timestamp_ns: diff.timestamp_ns,
        param_count: diff.param_updates.len() as u32,
        max_delta,
        delta_bits,
        estimated_bytes,
    }
}

// ── Bridge 2: CommLink → Crypto (encrypted channel metadata) ─────────────

/// Encrypted channel metadata derived from a deep-space communication link.
///
/// Provides the crypto layer with link identity, frequency hash, and
/// key rotation interval scaled to the link's one-way latency.
pub struct SpaceCryptoChannel {
    /// FNV-1a hash over source, target, and distance bytes.
    pub content_hash: u64,
    /// Source body ID.
    pub source_id: u64,
    /// Target body ID.
    pub target_id: u64,
    /// Distance in km.
    pub distance_km: f64,
    /// FNV-1a hash of source + target + distance bytes (link identity).
    pub link_hash: u64,
    /// Key rotation interval: `ceil(latency_s)` * 2, min 1.
    pub key_rotation_interval_s: u64,
    /// Fixed AEAD overhead: 40 bytes (tag + nonce).
    pub estimated_overhead_bytes: usize,
}

/// Convert a comm link into crypto encrypted channel metadata.
#[inline]
#[must_use]
pub fn space_commlink_to_crypto_channel(link: &CommLink) -> SpaceCryptoChannel {
    let latency = link.latency_s();

    // Link identity hash
    let mut link_key = [0u8; 24];
    link_key[0..8].copy_from_slice(&link.source_id.0.to_le_bytes());
    link_key[8..16].copy_from_slice(&link.target_id.0.to_le_bytes());
    link_key[16..24].copy_from_slice(&link.distance_km.to_bits().to_le_bytes());
    let link_hash = fnv1a(&link_key);

    let key_rotation_interval_s = (latency.ceil() as u64 * 2).max(1);

    SpaceCryptoChannel {
        content_hash: fnv1a(&link_key),
        source_id: link.source_id.0,
        target_id: link.target_id.0,
        distance_km: link.distance_km,
        link_hash,
        key_rotation_interval_s,
        estimated_overhead_bytes: 40,
    }
}

// ── Bridge 3: MissionEvent → Sync (mission sync event) ──────────────────

/// Sync event derived from a mission event for state replication.
///
/// Encodes mission phase, timestamp, and criticality flag so the sync
/// layer can prioritise replication of safety-critical mission phases.
pub struct SpaceSyncEvent {
    /// FNV-1a hash over phase, timestamp, and sequence bytes.
    pub content_hash: u64,
    /// Mission phase as u8 discriminant.
    pub phase: u8,
    /// Event timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Event sequence number.
    pub sequence: u32,
    /// Wire size in bytes: always 18 (phase:1 + timestamp:8 + sequence:4 + hash:5).
    pub wire_bytes: usize,
    /// True for safety-critical phases: Launch, Landing.
    pub is_critical: bool,
}

/// Convert a mission event into a sync event.
#[inline]
#[must_use]
pub fn space_mission_to_sync_event(event: &MissionEvent) -> SpaceSyncEvent {
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

    // Critical phases: Launch and Landing
    let is_critical = matches!(event.phase, MissionPhase::Launch | MissionPhase::Landing);

    let mut key = [0u8; 13];
    key[0] = phase_byte;
    key[1..9].copy_from_slice(&event.timestamp_ns.to_le_bytes());
    key[9..13].copy_from_slice(&event.sequence.to_le_bytes());

    SpaceSyncEvent {
        content_hash: fnv1a(&key),
        phase: phase_byte,
        timestamp_ns: event.timestamp_ns,
        sequence: event.sequence,
        wire_bytes: 18,
        is_critical,
    }
}

// ── Bridge 4: ModelDifferential → ASP (streaming payload) ────────────────

/// ASP streaming payload derived from a model differential.
///
/// Packs differential metadata into a fixed-size payload descriptor
/// so the ASP layer can schedule transmission without inspecting
/// individual parameter updates.
pub struct SpaceAspPayload {
    /// FNV-1a hash over timestamp, param count, and sequence bytes.
    pub content_hash: u64,
    /// Timestamp of the differential (nanoseconds).
    pub timestamp_ns: u64,
    /// Sequence number.
    pub sequence: u64,
    /// Number of parameter updates.
    pub param_count: u32,
    /// Payload bytes: 20 (timestamp:8 + sequence:8 + `param_count:4`).
    pub payload_bytes: usize,
    /// Priority: 1 (high) if any |delta| > 1.0, else 2 (normal).
    pub priority: u8,
}

/// Convert a model differential into an ASP streaming payload.
#[inline]
#[must_use]
pub fn space_differential_to_asp_payload(diff: &ModelDifferential) -> SpaceAspPayload {
    let has_large_delta = diff.param_updates.iter().any(|&(_, v)| v.abs() > 1.0);
    let priority = if has_large_delta { 1 } else { 2 };

    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&diff.timestamp_ns.to_le_bytes());
    key[8..16].copy_from_slice(&diff.sequence.to_le_bytes());
    key[16..20].copy_from_slice(&(diff.param_updates.len() as u32).to_le_bytes());

    SpaceAspPayload {
        content_hash: fnv1a(&key),
        timestamp_ns: diff.timestamp_ns,
        sequence: diff.sequence,
        param_count: diff.param_updates.len() as u32,
        payload_bytes: 20,
        priority,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_space::{CommLink, MissionLog, MissionPhase, ModelDifferential};

    fn make_earth_moon_link() -> CommLink {
        CommLink::new(1, 2, 384400.0, 9600.0)
    }

    fn make_differential() -> ModelDifferential {
        let mut diff = ModelDifferential::new(42, 1_000_000);
        diff.add_param("thrust_x", 3.25);
        diff.add_param("thrust_y", 2.71);
        diff.finalize();
        diff
    }

    // ── Bridge 1: differential → codec frame ────────────────────────────

    #[test]
    fn test_differential_to_codec_frame() {
        let diff = make_differential();
        let frame = space_differential_to_codec_frame(&diff);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.timestamp_ns, 1_000_000);
        assert_eq!(frame.param_count, 2);
        assert!(frame.max_delta > 0.0);
        assert!(frame.delta_bits >= 1);
        assert!(frame.estimated_bytes > 0);
    }

    #[test]
    fn test_differential_to_codec_frame_deterministic() {
        let diff = make_differential();
        let f1 = space_differential_to_codec_frame(&diff);
        let f2 = space_differential_to_codec_frame(&diff);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn test_differential_to_codec_frame_empty() {
        let diff = ModelDifferential::new(1, 0);
        let frame = space_differential_to_codec_frame(&diff);
        assert_eq!(frame.param_count, 0);
        assert!(frame.delta_bits >= 1);
    }

    // ── Bridge 2: commlink → crypto channel ─────────────────────────────

    #[test]
    fn test_commlink_to_crypto_channel() {
        let link = make_earth_moon_link();
        let ch = space_commlink_to_crypto_channel(&link);
        assert_ne!(ch.content_hash, 0);
        assert_eq!(ch.source_id, 1);
        assert_eq!(ch.target_id, 2);
        assert!((ch.distance_km - 384400.0).abs() < 1e-6);
        assert_ne!(ch.link_hash, 0);
        // Latency ~1.28s → ceil = 2 → rotation = 4
        assert!(ch.key_rotation_interval_s >= 2);
        assert_eq!(ch.estimated_overhead_bytes, 40);
    }

    #[test]
    fn test_commlink_to_crypto_channel_deterministic() {
        let link = make_earth_moon_link();
        let c1 = space_commlink_to_crypto_channel(&link);
        let c2 = space_commlink_to_crypto_channel(&link);
        assert_eq!(c1.content_hash, c2.content_hash);
        assert_eq!(c1.link_hash, c2.link_hash);
    }

    #[test]
    fn test_commlink_to_crypto_channel_min_rotation() {
        // Very short distance → latency < 1s → rotation still >= 1
        let link = CommLink::new(1, 2, 100.0, 9600.0);
        let ch = space_commlink_to_crypto_channel(&link);
        assert!(ch.key_rotation_interval_s >= 1);
    }

    // ── Bridge 3: mission → sync event ──────────────────────────────────

    #[test]
    fn test_mission_to_sync_event_launch() {
        let mut log = MissionLog::new();
        log.log_event(MissionPhase::Launch, 1000, 2.5, 500.0);
        let events = log.events_for_phase(MissionPhase::Launch);
        let sync = space_mission_to_sync_event(events[0]);
        assert_ne!(sync.content_hash, 0);
        assert_eq!(sync.phase, 0); // Launch
        assert_eq!(sync.timestamp_ns, 1000);
        assert_eq!(sync.wire_bytes, 18);
        assert!(sync.is_critical); // Launch is critical
    }

    #[test]
    fn test_mission_to_sync_event_landing() {
        let mut log = MissionLog::new();
        log.log_event(MissionPhase::Landing, 5000, 1.0, 300.0);
        let events = log.events_for_phase(MissionPhase::Landing);
        let sync = space_mission_to_sync_event(events[0]);
        assert_eq!(sync.phase, 4); // Landing
        assert!(sync.is_critical); // Landing is critical
    }

    #[test]
    fn test_mission_to_sync_event_not_critical() {
        let mut log = MissionLog::new();
        log.log_event(MissionPhase::Orbiting, 3000, 0.0, 480.0);
        let events = log.events_for_phase(MissionPhase::Orbiting);
        let sync = space_mission_to_sync_event(events[0]);
        assert_eq!(sync.phase, 3); // Orbiting
        assert!(!sync.is_critical); // Orbiting is not critical
    }

    #[test]
    fn test_mission_to_sync_event_deterministic() {
        let mut log = MissionLog::new();
        log.log_event(MissionPhase::Launch, 1000, 2.5, 500.0);
        let events = log.events_for_phase(MissionPhase::Launch);
        let s1 = space_mission_to_sync_event(events[0]);
        let s2 = space_mission_to_sync_event(events[0]);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    // ── Bridge 4: differential → ASP payload ────────────────────────────

    #[test]
    fn test_differential_to_asp_payload() {
        let diff = make_differential();
        let payload = space_differential_to_asp_payload(&diff);
        assert_ne!(payload.content_hash, 0);
        assert_eq!(payload.timestamp_ns, 1_000_000);
        assert_eq!(payload.sequence, 42);
        assert_eq!(payload.param_count, 2);
        assert_eq!(payload.payload_bytes, 20);
        // 3.14 > 1.0 → high priority
        assert_eq!(payload.priority, 1);
    }

    #[test]
    fn test_differential_to_asp_payload_normal_priority() {
        let mut diff = ModelDifferential::new(1, 0);
        diff.add_param("small", 0.5);
        diff.finalize();
        let payload = space_differential_to_asp_payload(&diff);
        assert_eq!(payload.priority, 2); // |0.5| < 1.0 → normal
    }

    #[test]
    fn test_differential_to_asp_payload_deterministic() {
        let diff = make_differential();
        let p1 = space_differential_to_asp_payload(&diff);
        let p2 = space_differential_to_asp_payload(&diff);
        assert_eq!(p1.content_hash, p2.content_hash);
    }
}
