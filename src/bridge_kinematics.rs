//! Kinematics bridges — ALICE-Kinematics ↔ Sync, Edge, Physics, Animation, ASP, DB
//!
//! 9 bridges connecting human motion intent compression to the ALICE ecosystem.

use alice_kinematics::{ArmChain, Intent, Predictor, Vec3k};
use alice_physics::Vec3Fix;
use alice_sync::InputFrame;

// ── Bridge 1: Kinematics → Sync (Intent → SyncEvent) ───────────────────

/// Sync-ready intent packet for ALICE-Sync P2P exchange.
pub struct IntentSyncPacket {
    /// Encoded intent (8 bytes).
    pub intent_bytes: [u8; 8],
    /// Frame number for lockstep.
    pub frame: u64,
    /// Player ID.
    pub player_id: u8,
}

/// Convert Intent to InputFrame for ALICE-Sync lockstep.
#[inline]
pub fn kinematics_to_sync_input(intent: &Intent, frame: u64, player: u8) -> InputFrame {
    let bytes = intent.encode();
    // Pack intent bytes into InputFrame movement fields
    let mx = i16::from_le_bytes([bytes[0], bytes[1]]);
    let my = i16::from_le_bytes([bytes[2], bytes[3]]);
    let mz = i16::from_le_bytes([bytes[4], bytes[5]]);
    InputFrame::new(frame, player).with_movement(mx, my, mz)
}

/// Reconstruct Intent from InputFrame received via ALICE-Sync.
#[inline]
pub fn sync_input_to_kinematics(input: &InputFrame) -> Intent {
    let mx = input.movement[0].to_le_bytes();
    let my = input.movement[1].to_le_bytes();
    let mz = input.movement[2].to_le_bytes();
    let bytes = [mx[0], mx[1], my[0], my[1], mz[0], mz[1], 0, 0];
    Intent::decode(&bytes)
}

// ── Bridge 2: Kinematics → Edge (motion capture compression) ────────────

/// Compressed motion capture sample for ALICE-Edge IoT streaming.
pub struct MocapEdgePacket {
    /// Intent bytes (8 bytes, 10,000x compression).
    pub intent_bytes: [u8; 8],
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
    /// Compression ratio vs raw 1000Hz float coordinates.
    pub compression_ratio: f64,
}

/// Compress raw position into Intent for ALICE-Edge IoT transport.
#[inline]
pub fn kinematics_to_edge_packet(target: Vec3k, duration_ms: u8, timestamp_us: u64) -> MocapEdgePacket {
    let intent = Intent::reach(target, duration_ms);
    let raw_size = 12 * (1000.0 * duration_ms as f64 / 1000.0) as usize; // 12 bytes/sample at 1000Hz
    let raw_size = raw_size.max(12);
    MocapEdgePacket {
        intent_bytes: intent.encode(),
        timestamp_us,
        compression_ratio: raw_size as f64 / 8.0,
    }
}

// ── Bridge 3: Kinematics → Physics (IK → rigid body) ────────────────────

/// Physics-compatible joint state from kinematic chain.
pub struct KinematicsPhysicsState {
    /// End-effector position in Fix128 coordinates.
    pub end_effector: Vec3Fix,
    /// Per-joint positions in Fix128 coordinates.
    pub joint_positions: Vec<Vec3Fix>,
    /// Number of active joints.
    pub joint_count: usize,
}

/// Convert ArmChain state to ALICE-Physics Vec3Fix coordinates.
#[inline]
pub fn kinematics_to_physics_state(chain: &ArmChain) -> KinematicsPhysicsState {
    let ee = chain.forward_kinematics();
    let mut joints = Vec::new();
    for i in 0..7 {
        let jp = chain.joint_position(i);
        joints.push(Vec3Fix::from_f32(jp.x, jp.y, jp.z));
    }
    KinematicsPhysicsState {
        end_effector: Vec3Fix::from_f32(ee.x, ee.y, ee.z),
        joint_positions: joints,
        joint_count: 7,
    }
}

/// Convert Physics Vec3Fix back to Kinematics Vec3k (for IK target).
#[inline(always)]
pub fn physics_to_kinematics_target(pos: &Vec3Fix) -> Vec3k {
    Vec3k::new(pos.x.to_f32(), pos.y.to_f32(), pos.z.to_f32())
}

// ── Bridge 4: Kinematics → Animation (character motion) ─────────────────

/// Animation keyframe from kinematic intent.
pub struct KinematicsAnimKeyframe {
    /// Time offset in seconds.
    pub time_secs: f32,
    /// End-effector position (x, y, z).
    pub position: (f32, f32, f32),
    /// Joint angles (7 DOF).
    pub joint_angles: [f32; 7],
    /// Intent type (reach, grasp, etc.).
    pub intent_type: u8,
}

/// Generate animation keyframes from Intent sequence for ALICE-Animation.
#[inline]
pub fn kinematics_to_animation_keyframes(intents: &[Intent]) -> Vec<KinematicsAnimKeyframe> {
    let mut chain = ArmChain::right_arm();
    let mut predictor = Predictor::new();
    let mut keyframes = Vec::new();
    let mut time = 0.0f32;

    for intent in intents {
        // Apply intent and run IK
        let target = intent.target;
        chain.inverse_kinematics(target, 50, 0.001);
        predictor.apply_intent(*intent);
        let ee = chain.forward_kinematics();
        let angles = chain.angles();

        keyframes.push(KinematicsAnimKeyframe {
            time_secs: time,
            position: (ee.x, ee.y, ee.z),
            joint_angles: angles,
            intent_type: intent.flags.intent_type() as u8,
        });
        time += intent.duration_secs();
    }
    keyframes
}

// ── Bridge 5: Kinematics → Streaming-Protocol (Intent → ASP) ────────────

/// ASP packet payload for motion intent streaming.
pub struct IntentAspPayload {
    /// Encoded intent (8 bytes).
    pub intent_bytes: [u8; 8],
    /// ASP sequence number.
    pub sequence: u32,
    /// Packet size in bytes.
    pub packet_size: usize,
}

/// Package Intent for ALICE-Streaming-Protocol transport.
#[inline]
pub fn kinematics_to_asp_payload(intent: &Intent, sequence: u32) -> IntentAspPayload {
    IntentAspPayload {
        intent_bytes: intent.encode(),
        sequence,
        packet_size: 8,
    }
}

// ── Bridge 6: Kinematics → DB (motion capture storage) ──────────────────

/// Motion capture record for ALICE-DB persistence.
pub struct MocapDbRecord {
    /// Timestamp key.
    pub timestamp: i64,
    /// Intent bytes (8 bytes).
    pub intent_bytes: [u8; 8],
    /// Predicted position at end of intent.
    pub predicted_position: (f32, f32, f32),
    /// Content hash.
    pub content_hash: u64,
}

/// Serialize Intent sequence for ALICE-DB storage.
#[inline]
pub fn kinematics_to_db_records(intents: &[Intent], start_timestamp: i64) -> Vec<MocapDbRecord> {
    let mut predictor = Predictor::new();
    let mut records = Vec::new();
    let mut ts = start_timestamp;

    for intent in intents {
        let bytes = intent.encode();
        predictor.apply_intent(*intent);
        let dur = intent.duration_secs();
        // Advance predictor to get end position
        let steps = (dur / 0.01) as usize;
        for _ in 0..steps {
            predictor.update(0.01);
        }
        let pos = predictor.position_at(0.0);
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in &bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        records.push(MocapDbRecord {
            timestamp: ts,
            intent_bytes: bytes,
            predicted_position: (pos.x, pos.y, pos.z),
            content_hash: hash,
        });
        ts += (dur * 1000.0) as i64;
    }
    records
}

// ── Bridge 7: Kinematics → Cache (intent caching) ───────────────────

/// Intent cache entry for ALICE-Cache.
pub struct IntentCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Intent bytes (8 bytes).
    pub intent_bytes: [u8; 8],
    /// Duration in seconds (for eviction priority).
    pub duration_secs: f32,
}

/// Prepare Intent for ALICE-Cache storage.
#[inline]
pub fn kinematics_to_cache_entry(intent: &Intent) -> IntentCacheEntry {
    let bytes = intent.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    IntentCacheEntry {
        content_hash: hash,
        intent_bytes: bytes,
        duration_secs: intent.duration_secs(),
    }
}

// ── Bridge 8: Kinematics → Crypto (encrypted intent) ────────────────

/// Encrypted intent payload for secure transport.
pub struct IntentCryptoPayload {
    /// Plaintext intent bytes.
    pub plaintext: [u8; 8],
    /// Content hash for integrity.
    pub content_hash: u64,
    /// Payload size.
    pub payload_bytes: usize,
}

/// Prepare Intent for ALICE-Crypto encryption.
#[inline]
pub fn kinematics_to_crypto_payload(intent: &Intent) -> IntentCryptoPayload {
    let bytes = intent.encode();
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    IntentCryptoPayload {
        plaintext: bytes,
        content_hash: hash,
        payload_bytes: 8,
    }
}

// ── Bridge 9: Kinematics → CDN (motion library distribution) ────────

/// Motion capture library package for ALICE-CDN delivery.
pub struct MocapCdnPackage {
    /// Intent sequence bytes.
    pub data: Vec<u8>,
    /// Content hash for CDN routing.
    pub content_hash: u64,
    /// Number of intents.
    pub intent_count: usize,
    /// Total duration in seconds.
    pub total_duration_secs: f32,
}

/// Package Intent sequence for ALICE-CDN distribution.
#[inline]
pub fn kinematics_to_cdn_package(intents: &[Intent]) -> MocapCdnPackage {
    let mut data = Vec::with_capacity(intents.len() * 8);
    let mut total_dur = 0.0f32;
    for intent in intents {
        data.extend_from_slice(&intent.encode());
        total_dur += intent.duration_secs();
    }
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    MocapCdnPackage {
        data,
        content_hash: hash,
        intent_count: intents.len(),
        total_duration_secs: total_dur,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kinematics_to_sync_roundtrip() {
        let intent = Intent::reach(Vec3k::new(1.0, 2.0, 0.5), 100);
        let input = kinematics_to_sync_input(&intent, 42, 0);
        assert_eq!(input.frame, 42);
        assert_eq!(input.player_id, 0);
        // Partial roundtrip (6 of 8 bytes via movement fields)
        let recovered = sync_input_to_kinematics(&input);
        assert_eq!(recovered.encode()[..6], intent.encode()[..6]);
    }

    #[test]
    fn test_kinematics_to_edge_packet() {
        let pkt = kinematics_to_edge_packet(Vec3k::new(0.5, 0.3, 0.1), 200, 1_000_000);
        assert_eq!(pkt.intent_bytes.len(), 8);
        assert_eq!(pkt.timestamp_us, 1_000_000);
        assert!(pkt.compression_ratio > 1.0);
    }

    #[test]
    fn test_kinematics_to_physics_state() {
        let chain = ArmChain::right_arm();
        let state = kinematics_to_physics_state(&chain);
        assert_eq!(state.joint_count, 7);
        assert_eq!(state.joint_positions.len(), 7);
    }

    #[test]
    fn test_physics_to_kinematics_target() {
        let pos = Vec3Fix::from_f32(1.5, 2.0, 0.5);
        let target = physics_to_kinematics_target(&pos);
        assert!((target.x - 1.5).abs() < 0.01);
        assert!((target.y - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_kinematics_to_animation_keyframes() {
        let intents = vec![
            Intent::reach(Vec3k::new(0.3, 0.4, 0.0), 100),
            Intent::reach(Vec3k::new(0.5, 0.2, 0.1), 150),
        ];
        let kf = kinematics_to_animation_keyframes(&intents);
        assert_eq!(kf.len(), 2);
        assert!((kf[0].time_secs - 0.0).abs() < 0.001);
        assert!(kf[1].time_secs > 0.0);
    }

    #[test]
    fn test_kinematics_to_asp_payload() {
        let intent = Intent::reach(Vec3k::new(1.0, 0.0, 0.0), 50);
        let payload = kinematics_to_asp_payload(&intent, 99);
        assert_eq!(payload.packet_size, 8);
        assert_eq!(payload.sequence, 99);
    }

    #[test]
    fn test_kinematics_to_db_records() {
        let intents = vec![
            Intent::reach(Vec3k::new(0.5, 0.5, 0.0), 100),
        ];
        let recs = kinematics_to_db_records(&intents, 0);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].timestamp, 0);
        assert_ne!(recs[0].content_hash, 0);
    }

    #[test]
    fn test_kinematics_to_cache_entry() {
        let intent = Intent::reach(Vec3k::new(0.3, 0.4, 0.0), 100);
        let entry = kinematics_to_cache_entry(&intent);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.intent_bytes.len(), 8);
        assert!(entry.duration_secs > 0.0);
    }

    #[test]
    fn test_kinematics_to_crypto_payload() {
        let intent = Intent::reach(Vec3k::new(0.5, 0.2, 0.1), 50);
        let crypto = kinematics_to_crypto_payload(&intent);
        assert_eq!(crypto.payload_bytes, 8);
        assert_ne!(crypto.content_hash, 0);
    }

    #[test]
    fn test_kinematics_to_cdn_package() {
        let intents = vec![
            Intent::reach(Vec3k::new(0.3, 0.4, 0.0), 100),
            Intent::reach(Vec3k::new(0.5, 0.2, 0.1), 50),
        ];
        let pkg = kinematics_to_cdn_package(&intents);
        assert_eq!(pkg.intent_count, 2);
        assert_eq!(pkg.data.len(), 16);
        assert_ne!(pkg.content_hash, 0);
        assert!(pkg.total_duration_secs > 0.0);
    }
}
