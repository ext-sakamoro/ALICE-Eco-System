//! Cross-domain bridges — ALICE-Neural ↔ ML, Sync, Voice
//!
//! 3 bridges connecting brain-computer interface neural data to
//! ML feature extraction, sync event replication, and voice command synthesis.

use alice_neural::{NeuralField, IntentPacket, IntentKind};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: NeuralField → ML (feature vector extraction) ──────────────

/// ML feature vector derived from a neural field.
///
/// Extracts source count, peak amplitude, and total energy as scalar
/// features suitable for downstream classifier or regression models.
pub struct NeuralMlFeatures {
    /// FNV-1a hash over source_count, peak_amplitude, and total_energy bytes.
    pub content_hash: u64,
    /// Number of Gaussian sources in the field.
    pub source_count: usize,
    /// Peak weight amplitude across all sources.
    pub peak_amplitude: f64,
    /// Total field energy (sum of weight squared).
    pub total_energy: f64,
    /// Feature dimensionality: source_count * 3 (weight, sigma, position norm per source).
    pub feature_dim: usize,
    /// True if peak_amplitude > 1.0 (field is actively driven).
    pub is_active: bool,
}

/// Convert a neural field into ML feature vector metadata.
#[inline]
pub fn neural_field_to_ml_features(field: &NeuralField) -> NeuralMlFeatures {
    let source_count = field.source_count();
    let peak_amplitude = field.weights
        .iter()
        .map(|w| w.abs())
        .fold(0.0f64, f64::max);
    let total_energy = field.total_energy();
    let feature_dim = source_count * 3;
    let is_active = peak_amplitude > 1.0;

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&(source_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&peak_amplitude.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&total_energy.to_bits().to_le_bytes());

    NeuralMlFeatures {
        content_hash: fnv1a(&key),
        source_count,
        peak_amplitude,
        total_energy,
        feature_dim,
        is_active,
    }
}

// ── Bridge 2: IntentPacket → Sync (BCI networking event) ────────────────

/// Sync event derived from a neural intent packet for BCI networking.
///
/// Encodes intent kind, confidence, and motor classification so the
/// sync layer can prioritise low-latency replication of motor commands.
pub struct NeuralSyncEvent {
    /// FNV-1a hash over intent kind, confidence, and timestamp bytes.
    pub content_hash: u64,
    /// Intent kind discriminant (0=MotorLeft, 1=MotorRight, 2=Speech, 3=Visual, 4=Cognitive, 5=Idle).
    pub intent_kind: u8,
    /// Classifier confidence (0.0 to 1.0).
    pub confidence: f64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Wire size in bytes: always 18.
    pub wire_bytes: usize,
    /// True for motor intents: MotorLeft (0) and MotorRight (1).
    pub is_motor: bool,
}

/// Convert an intent packet into a sync event for BCI networking.
#[inline]
pub fn neural_intent_to_sync_event(packet: &IntentPacket) -> NeuralSyncEvent {
    let kind_byte = packet.kind.discriminant();

    // Motor intents: MotorLeft=0, MotorRight=1
    let is_motor = kind_byte <= 1;

    let mut key = [0u8; 17];
    key[0] = kind_byte;
    key[1..9].copy_from_slice(&packet.confidence.to_bits().to_le_bytes());
    key[9..17].copy_from_slice(&packet.timestamp_ns.to_le_bytes());

    NeuralSyncEvent {
        content_hash: fnv1a(&key),
        intent_kind: kind_byte,
        confidence: packet.confidence,
        timestamp_ns: packet.timestamp_ns,
        wire_bytes: 18,
        is_motor,
    }
}

// ── Bridge 3: IntentPacket → Voice (command synthesis) ───────────────────

/// Voice command derived from a neural intent packet.
///
/// Maps intent categories to command priorities and feedback requirements
/// so the voice synthesis layer can schedule and confirm BCI-driven speech.
pub struct NeuralVoiceCommand {
    /// FNV-1a hash over intent kind, confidence, and priority bytes.
    pub content_hash: u64,
    /// Intent kind discriminant.
    pub intent_kind: u8,
    /// Classifier confidence (0.0 to 1.0).
    pub confidence: f64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Command priority: Speech=1, Visual=2, Motor=3, Cognitive=4, Idle=5.
    pub command_priority: u8,
    /// True if the intent requires auditory feedback (Speech intents).
    pub requires_feedback: bool,
}

/// Convert an intent packet into a voice command for synthesis.
#[inline]
pub fn neural_intent_to_voice_command(packet: &IntentPacket) -> NeuralVoiceCommand {
    let kind_byte = packet.kind.discriminant();

    let command_priority = match packet.kind {
        IntentKind::Speech    => 1,
        IntentKind::Visual    => 2,
        IntentKind::MotorLeft | IntentKind::MotorRight => 3,
        IntentKind::Cognitive => 4,
        IntentKind::Idle      => 5,
    };

    // Speech requires auditory feedback confirmation
    let requires_feedback = matches!(packet.kind, IntentKind::Speech);

    let mut key = [0u8; 10];
    key[0] = kind_byte;
    key[1..9].copy_from_slice(&packet.confidence.to_bits().to_le_bytes());
    key[9] = command_priority;

    NeuralVoiceCommand {
        content_hash: fnv1a(&key),
        intent_kind: kind_byte,
        confidence: packet.confidence,
        timestamp_ns: packet.timestamp_ns,
        command_priority,
        requires_feedback,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_neural::{NeuralField, IntentKind, extract_intent};

    fn make_active_field() -> NeuralField {
        let mut field = NeuralField::new(1_000_000_000);
        field.add_source([30.0, -15.0, 60.0], 8.0, 5.0);
        field.add_source([-30.0, -15.0, 60.0], 3.0, 5.0);
        field
    }

    fn make_intent(kind: IntentKind, confidence: f64) -> IntentPacket {
        let mut packet = IntentPacket {
            kind,
            confidence,
            parameters: [0.1, 0.2, 3.0, 0.0],
            timestamp_ns: 42_000_000,
            content_hash: 0,
        };
        packet.finalize();
        packet
    }

    // ── Bridge 1: neural field → ML features ────────────────────────────

    #[test]
    fn test_field_to_ml_features_active() {
        let field = make_active_field();
        let features = neural_field_to_ml_features(&field);
        assert_ne!(features.content_hash, 0);
        assert_eq!(features.source_count, 2);
        assert!((features.peak_amplitude - 8.0).abs() < 1e-9);
        // total_energy = 8^2 + 3^2 = 64 + 9 = 73
        assert!((features.total_energy - 73.0).abs() < 1e-9);
        assert_eq!(features.feature_dim, 6); // 2 sources * 3
        assert!(features.is_active); // peak 8.0 > 1.0
    }

    #[test]
    fn test_field_to_ml_features_empty() {
        let field = NeuralField::new(0);
        let features = neural_field_to_ml_features(&field);
        assert_eq!(features.source_count, 0);
        assert_eq!(features.peak_amplitude, 0.0);
        assert_eq!(features.total_energy, 0.0);
        assert_eq!(features.feature_dim, 0);
        assert!(!features.is_active);
    }

    #[test]
    fn test_field_to_ml_features_inactive() {
        let mut field = NeuralField::new(0);
        field.add_source([0.0; 3], 0.5, 1.0);
        let features = neural_field_to_ml_features(&field);
        assert!(!features.is_active); // peak 0.5 < 1.0
    }

    #[test]
    fn test_field_to_ml_features_deterministic() {
        let field = make_active_field();
        let f1 = neural_field_to_ml_features(&field);
        let f2 = neural_field_to_ml_features(&field);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 2: intent → sync event ───────────────────────────────────

    #[test]
    fn test_intent_to_sync_event_motor() {
        let packet = make_intent(IntentKind::MotorLeft, 0.95);
        let sync = neural_intent_to_sync_event(&packet);
        assert_ne!(sync.content_hash, 0);
        assert_eq!(sync.intent_kind, 0); // MotorLeft
        assert!((sync.confidence - 0.95).abs() < 1e-10);
        assert_eq!(sync.timestamp_ns, 42_000_000);
        assert_eq!(sync.wire_bytes, 18);
        assert!(sync.is_motor);
    }

    #[test]
    fn test_intent_to_sync_event_motor_right() {
        let packet = make_intent(IntentKind::MotorRight, 0.9);
        let sync = neural_intent_to_sync_event(&packet);
        assert_eq!(sync.intent_kind, 1); // MotorRight
        assert!(sync.is_motor);
    }

    #[test]
    fn test_intent_to_sync_event_speech_not_motor() {
        let packet = make_intent(IntentKind::Speech, 0.85);
        let sync = neural_intent_to_sync_event(&packet);
        assert_eq!(sync.intent_kind, 2); // Speech
        assert!(!sync.is_motor);
    }

    #[test]
    fn test_intent_to_sync_event_idle_not_motor() {
        let packet = make_intent(IntentKind::Idle, 0.0);
        let sync = neural_intent_to_sync_event(&packet);
        assert_eq!(sync.intent_kind, 5); // Idle
        assert!(!sync.is_motor);
    }

    #[test]
    fn test_intent_to_sync_event_deterministic() {
        let packet = make_intent(IntentKind::MotorLeft, 0.95);
        let s1 = neural_intent_to_sync_event(&packet);
        let s2 = neural_intent_to_sync_event(&packet);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    // ── Bridge 3: intent → voice command ────────────────────────────────

    #[test]
    fn test_intent_to_voice_command_speech() {
        let packet = make_intent(IntentKind::Speech, 0.9);
        let cmd = neural_intent_to_voice_command(&packet);
        assert_ne!(cmd.content_hash, 0);
        assert_eq!(cmd.intent_kind, 2); // Speech
        assert_eq!(cmd.command_priority, 1); // Speech → highest priority
        assert!(cmd.requires_feedback);
    }

    #[test]
    fn test_intent_to_voice_command_visual() {
        let packet = make_intent(IntentKind::Visual, 0.8);
        let cmd = neural_intent_to_voice_command(&packet);
        assert_eq!(cmd.command_priority, 2);
        assert!(!cmd.requires_feedback);
    }

    #[test]
    fn test_intent_to_voice_command_motor() {
        let packet = make_intent(IntentKind::MotorLeft, 0.7);
        let cmd = neural_intent_to_voice_command(&packet);
        assert_eq!(cmd.command_priority, 3);
        assert!(!cmd.requires_feedback);
    }

    #[test]
    fn test_intent_to_voice_command_cognitive() {
        let packet = make_intent(IntentKind::Cognitive, 0.6);
        let cmd = neural_intent_to_voice_command(&packet);
        assert_eq!(cmd.command_priority, 4);
        assert!(!cmd.requires_feedback);
    }

    #[test]
    fn test_intent_to_voice_command_idle() {
        let packet = make_intent(IntentKind::Idle, 0.0);
        let cmd = neural_intent_to_voice_command(&packet);
        assert_eq!(cmd.command_priority, 5);
        assert!(!cmd.requires_feedback);
    }

    #[test]
    fn test_intent_to_voice_command_deterministic() {
        let packet = make_intent(IntentKind::Speech, 0.9);
        let c1 = neural_intent_to_voice_command(&packet);
        let c2 = neural_intent_to_voice_command(&packet);
        assert_eq!(c1.content_hash, c2.content_hash);
    }

    // ── Integration: field → extract_intent → bridges ───────────────────

    #[test]
    fn test_full_pipeline_field_to_sync() {
        let field = make_active_field();
        let map = vec![
            (IntentKind::MotorLeft, [-30.0, -15.0, 60.0]),
            (IntentKind::MotorRight, [30.0, -15.0, 60.0]),
        ];
        let packet = extract_intent(&field, &map);
        // Strongest source is at [30, -15, 60] with weight 8.0 → MotorRight
        assert_eq!(packet.kind, IntentKind::MotorRight);

        let sync = neural_intent_to_sync_event(&packet);
        assert!(sync.is_motor);
        assert!(sync.confidence > 0.0);

        let cmd = neural_intent_to_voice_command(&packet);
        assert_eq!(cmd.command_priority, 3); // Motor → priority 3
    }
}
