//! Neural bridges — ALICE-Neural ↔ Analytics, DB, Edge
//!
//! 5 bridges connecting brain-computer interface data to the ALICE ecosystem.

use alice_neural::{ElectrodeArray, IntentKind, IntentPacket, NeuralField, SpikeTrain};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: ElectrodeArray → Analytics (array metrics) ────────────────

/// Electrode array metrics for ALICE-Analytics ingestion.
pub struct NeuralAnalyticsArrayEvent {
    /// Content hash over electrode count, bandwidth, and bounding box bytes.
    pub content_hash: u64,
    /// Number of electrodes in the array.
    pub electrode_count: usize,
    /// Total bandwidth in bits per second (u64 from the API).
    pub total_bandwidth_bps: u64,
    /// Bounding box min corner (mm).
    pub bbox_min: [f64; 3],
    /// Bounding box max corner (mm).
    pub bbox_max: [f64; 3],
}

/// Convert an electrode array into an analytics metrics event.
#[inline]
pub fn neural_array_to_analytics(array: &ElectrodeArray) -> NeuralAnalyticsArrayEvent {
    let bandwidth = array.total_bandwidth_bps();
    let (bbox_min, bbox_max) = array.bounding_box();
    let count = array.count();

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&(count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&bandwidth.to_le_bytes());
    key[16..24].copy_from_slice(&bbox_min[0].to_bits().to_le_bytes());

    NeuralAnalyticsArrayEvent {
        content_hash: fnv1a(&key),
        electrode_count: count,
        total_bandwidth_bps: bandwidth,
        bbox_min,
        bbox_max,
    }
}

// ── Bridge 2: SpikeTrain → Analytics (firing rate metrics) ──────────────

/// Spike train firing rate metrics for ALICE-Analytics ingestion.
pub struct NeuralAnalyticsSpikeEvent {
    /// Content hash over electrode ID, spike count, and firing rate bytes.
    pub content_hash: u64,
    /// Electrode ID queried for firing rate.
    pub electrode_id: u32,
    /// Number of spikes in the train.
    pub spike_count: usize,
    /// Firing rate in Hz for the given electrode.
    pub firing_rate_hz: f64,
    /// Mean spike amplitude across all spikes.
    pub mean_amplitude: f64,
    /// Window duration in seconds.
    pub window_duration_s: f64,
}

/// Convert a spike train into a firing rate analytics event.
///
/// `electrode_id` selects which electrode's firing rate to report.
#[inline]
pub fn neural_spike_to_analytics(
    train: &SpikeTrain,
    electrode_id: u32,
) -> NeuralAnalyticsSpikeEvent {
    let firing_rate = train.firing_rate(electrode_id);
    let mean_amp = train.mean_amplitude();
    let count = train.spike_count();
    let window_s = if train.window_end_ns > train.window_start_ns {
        (train.window_end_ns - train.window_start_ns) as f64 * 1e-9
    } else {
        0.0
    };

    let mut key = [0u8; 28];
    key[0..4].copy_from_slice(&electrode_id.to_le_bytes());
    key[4..12].copy_from_slice(&(count as u64).to_le_bytes());
    key[12..20].copy_from_slice(&firing_rate.to_bits().to_le_bytes());
    key[20..28].copy_from_slice(&mean_amp.to_bits().to_le_bytes());

    NeuralAnalyticsSpikeEvent {
        content_hash: fnv1a(&key),
        electrode_id,
        spike_count: count,
        firing_rate_hz: firing_rate,
        mean_amplitude: mean_amp,
        window_duration_s: window_s,
    }
}

// ── Bridge 3: NeuralField → DB (field snapshot record) ─────────────────

/// Neural field snapshot for ALICE-DB persistence.
pub struct NeuralDbFieldRecord {
    /// Content hash over source count, total energy, and peak location bytes.
    pub content_hash: u64,
    /// Number of Gaussian sources in the field.
    pub source_count: usize,
    /// Total field energy (sum of weight squared).
    pub total_energy: f64,
    /// Peak activity location (mm), or [0;3] if no sources.
    pub peak_location: [f64; 3],
    /// Field value at origin.
    pub value_at_origin: f64,
    /// Timestamp of the field snapshot in nanoseconds.
    pub timestamp_ns: u64,
}

/// Convert a neural field into a DB snapshot record.
#[inline]
pub fn neural_field_to_db(field: &NeuralField) -> NeuralDbFieldRecord {
    let total_energy = field.total_energy();
    let peak = field.peak_location().unwrap_or([0.0; 3]);
    let value_at_origin = field.eval(&[0.0, 0.0, 0.0]);

    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&(field.source_count() as u64).to_le_bytes());
    key[8..16].copy_from_slice(&total_energy.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&peak[0].to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&peak[1].to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&peak[2].to_bits().to_le_bytes());

    NeuralDbFieldRecord {
        content_hash: fnv1a(&key),
        source_count: field.source_count(),
        total_energy,
        peak_location: peak,
        value_at_origin,
        timestamp_ns: field.timestamp_ns,
    }
}

// ── Bridge 4: IntentPacket → Edge (real-time intent telemetry) ──────────

/// Intent packet telemetry for ALICE-Edge ingestion.
pub struct NeuralEdgeIntentEvent {
    /// Content hash over intent kind, confidence, and timestamp bytes.
    pub content_hash: u64,
    /// Intent kind discriminant: 0=MotorLeft, 1=MotorRight, 2=Speech, 3=Visual, 4=Cognitive, 5=Idle.
    pub intent_kind: u8,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Packet size in bytes (constant 57).
    pub packet_bytes: usize,
}

/// Convert an intent packet into an edge telemetry event.
#[inline]
pub fn neural_intent_to_edge(packet: &IntentPacket) -> NeuralEdgeIntentEvent {
    let kind_byte = packet.kind.discriminant();

    let mut key = [0u8; 17];
    key[0] = kind_byte;
    key[1..9].copy_from_slice(&packet.confidence.to_bits().to_le_bytes());
    key[9..17].copy_from_slice(&packet.timestamp_ns.to_le_bytes());

    NeuralEdgeIntentEvent {
        content_hash: fnv1a(&key),
        intent_kind: kind_byte,
        confidence: packet.confidence,
        timestamp_ns: packet.timestamp_ns,
        packet_bytes: IntentPacket::byte_size(),
    }
}

// ── Bridge 5: IntentPacket → Cache (real-time intent lookup) ────────────

/// Intent cache entry for ALICE-Cache real-time lookup.
pub struct NeuralCacheIntent {
    /// Content hash over intent kind and confidence bytes.
    pub content_hash: u64,
    /// Intent kind discriminant byte.
    pub intent_kind: u8,
    /// Confidence level.
    pub confidence: f64,
    /// Cache TTL: 1s for motor intents (low-latency), 5s otherwise.
    pub ttl_secs: u32,
}

/// Convert an intent packet into a cache entry with adaptive TTL.
#[inline]
pub fn neural_intent_to_cache(packet: &IntentPacket) -> NeuralCacheIntent {
    let kind_byte = packet.kind.discriminant();

    let mut key = [0u8; 9];
    key[0] = kind_byte;
    key[1..9].copy_from_slice(&packet.confidence.to_bits().to_le_bytes());

    // Branchless TTL: motor=1 → 5-4=1, non_motor=0 → 5-0=5.
    let is_motor = (kind_byte <= 1) as u32;
    let ttl_secs = 5 - is_motor * 4;

    NeuralCacheIntent {
        content_hash: fnv1a(&key),
        intent_kind: kind_byte,
        confidence: packet.confidence,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_neural::{ElectrodeArray, IntentKind, IntentPacket, NeuralField, SpikeTrain};

    #[test]
    fn test_array_to_analytics() {
        let mut array = ElectrodeArray::new("motor_cortex");
        array.add_electrode([0.0, 0.0, 0.0], 100.0, 30000);
        array.add_electrode([1.0, 0.0, 0.0], 100.0, 30000);
        let ev = neural_array_to_analytics(&array);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.electrode_count, 2);
        // 2 electrodes * 30000 Hz * 16 bits = 960_000 bps
        assert_eq!(ev.total_bandwidth_bps, 960_000);
    }

    #[test]
    fn test_spike_to_analytics() {
        // Window: 0 to 1 second (1_000_000_000 ns)
        let mut train = SpikeTrain::new(0, 1_000_000_000);
        train.add_spike(0, 100_000_000, 150.0, 400);
        train.add_spike(0, 500_000_000, 250.0, 350);
        let ev = neural_spike_to_analytics(&train, 0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.electrode_id, 0);
        assert_eq!(ev.spike_count, 2);
        // 2 spikes / 1 second = 2.0 Hz
        assert!((ev.firing_rate_hz - 2.0).abs() < 1e-9);
        // mean_amplitude = (|150| + |250|) / 2 = 200.0
        assert!((ev.mean_amplitude - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_field_to_db() {
        let mut field = NeuralField::new(42_000_000);
        field.add_source([0.0, 0.0, 0.0], 1.0, 1.0);
        let rec = neural_field_to_db(&field);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.source_count, 1);
        // total_energy = 1.0^2 = 1.0
        assert!((rec.total_energy - 1.0).abs() < 1e-9);
        assert_eq!(rec.peak_location, [0.0, 0.0, 0.0]);
        // eval at origin with source at origin: weight * exp(0) = 1.0
        assert!((rec.value_at_origin - 1.0).abs() < 1e-9);
        assert_eq!(rec.timestamp_ns, 42_000_000);
    }

    #[test]
    fn test_field_to_db_empty() {
        let field = NeuralField::new(0);
        let rec = neural_field_to_db(&field);
        assert_eq!(rec.source_count, 0);
        assert_eq!(rec.total_energy, 0.0);
        assert_eq!(rec.peak_location, [0.0, 0.0, 0.0]);
        assert_eq!(rec.value_at_origin, 0.0);
    }

    #[test]
    fn test_intent_to_edge() {
        let mut packet = IntentPacket {
            kind: IntentKind::MotorLeft,
            confidence: 0.95,
            parameters: [0.1, 0.0, 5.0, 0.0],
            timestamp_ns: 1_000_000,
            content_hash: 0,
        };
        packet.finalize();
        let ev = neural_intent_to_edge(&packet);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.intent_kind, 0); // MotorLeft discriminant
        assert!((ev.confidence - 0.95).abs() < 1e-10);
        assert_eq!(ev.timestamp_ns, 1_000_000);
        assert_eq!(ev.packet_bytes, 57);
    }

    #[test]
    fn test_intent_to_cache_motor() {
        let mut packet = IntentPacket {
            kind: IntentKind::MotorRight,
            confidence: 0.9,
            parameters: [0.0; 4],
            timestamp_ns: 1000,
            content_hash: 0,
        };
        packet.finalize();
        let entry = neural_intent_to_cache(&packet);
        assert_eq!(entry.ttl_secs, 1); // motor intent → 1s
        assert_eq!(entry.intent_kind, 1); // MotorRight discriminant
    }

    #[test]
    fn test_intent_to_cache_cognitive() {
        let mut packet = IntentPacket {
            kind: IntentKind::Cognitive,
            confidence: 0.8,
            parameters: [0.0; 4],
            timestamp_ns: 2000,
            content_hash: 0,
        };
        packet.finalize();
        let entry = neural_intent_to_cache(&packet);
        assert_eq!(entry.ttl_secs, 5); // non-motor → 5s
        assert_eq!(entry.intent_kind, 4); // Cognitive discriminant
    }

    #[test]
    fn test_hash_determinism() {
        let mut packet = IntentPacket {
            kind: IntentKind::Speech,
            confidence: 0.75,
            parameters: [0.5, 0.5, 3.0, 0.0],
            timestamp_ns: 5000,
            content_hash: 0,
        };
        packet.finalize();
        let e1 = neural_intent_to_edge(&packet);
        let e2 = neural_intent_to_edge(&packet);
        assert_eq!(e1.content_hash, e2.content_hash);
    }
}
