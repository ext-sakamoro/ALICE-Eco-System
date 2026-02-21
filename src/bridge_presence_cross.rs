//! Cross-domain bridges — ALICE-Presence ↔ Sync, Auth, Crypto
//!
//! 3 bridges connecting presence protocol events to sync frame transport,
//! authentication token generation, and cryptographic proof sealing.

use alice_presence::{PresenceEvent, IdentityCommitment, ProximityProof};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: PresenceEvent → Sync (InputFrame equivalent) ──────────

/// Sync frame record derived from a presence event.
///
/// Maps a presence event into ALICE-Sync InputFrame-compatible metadata
/// so the Sync layer can transport presence events over its existing
/// delta-compression and ordered-delivery infrastructure.
pub struct PresenceSyncFrame {
    /// FNV-1a hash over party_a_id, party_b_id, timestamp_ns, flags bytes.
    pub content_hash: u64,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Wire size in bytes (always 18, same as PresenceEvent).
    pub wire_bytes: usize,
    /// Whether both parties are mutually present.
    pub is_mutual: bool,
    /// Whether the event has been verified.
    pub is_verified: bool,
    /// Frame type byte (0x50 = 'P').
    pub frame_type: u8,
}

/// Convert a presence event into a sync frame record.
#[inline]
pub fn presence_event_to_sync_frame(event: &PresenceEvent) -> PresenceSyncFrame {
    let mut key = [0u8; 17];
    key[0..4].copy_from_slice(&event.party_a_id.to_le_bytes());
    key[4..8].copy_from_slice(&event.party_b_id.to_le_bytes());
    key[8..16].copy_from_slice(&event.timestamp_ns.to_le_bytes());
    key[16] = event.flags;

    PresenceSyncFrame {
        content_hash: fnv1a(&key),
        party_a_id: event.party_a_id,
        party_b_id: event.party_b_id,
        timestamp_ns: event.timestamp_ns,
        wire_bytes: PresenceEvent::byte_size(),
        is_mutual: event.is_mutual(),
        is_verified: event.is_verified(),
        frame_type: event.event_type,
    }
}

// ── Bridge 2: IdentityCommitment → Auth (verification token) ────────

/// Auth verification token derived from an identity commitment.
///
/// Maps an identity commitment into ALICE-Auth token metadata so the
/// Auth layer can verify presence identity claims using its existing
/// token validation infrastructure.
pub struct PresenceAuthToken {
    /// FNV-1a hash over commitment_hash, nonce, timestamp_ns, token_hash bytes.
    pub content_hash: u64,
    /// Commitment hash from the identity commitment.
    pub commitment_hash: u64,
    /// Nonce used in the commitment.
    pub nonce: u64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Token hash: fnv1a of commitment_hash + nonce + timestamp.
    pub token_hash: u64,
    /// Time-to-live in seconds (3600 = 1 hour).
    pub ttl_secs: u32,
}

/// Convert an identity commitment into an auth verification token.
#[inline]
pub fn presence_identity_to_auth_token(commitment: &IdentityCommitment) -> PresenceAuthToken {
    // Compute token hash: fnv1a of commitment_hash + nonce + timestamp
    let mut tok_key = [0u8; 24];
    tok_key[0..8].copy_from_slice(&commitment.commitment_hash.to_le_bytes());
    tok_key[8..16].copy_from_slice(&commitment.nonce.to_le_bytes());
    tok_key[16..24].copy_from_slice(&commitment.timestamp_ns.to_le_bytes());
    let token_hash = fnv1a(&tok_key);

    // Content hash over all fields including derived token_hash
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&commitment.commitment_hash.to_le_bytes());
    key[8..16].copy_from_slice(&commitment.nonce.to_le_bytes());
    key[16..24].copy_from_slice(&commitment.timestamp_ns.to_le_bytes());
    key[24..32].copy_from_slice(&token_hash.to_le_bytes());

    PresenceAuthToken {
        content_hash: fnv1a(&key),
        commitment_hash: commitment.commitment_hash,
        nonce: commitment.nonce,
        timestamp_ns: commitment.timestamp_ns,
        token_hash,
        ttl_secs: 3600,
    }
}

// ── Bridge 3: ProximityProof → Crypto (sealed proof) ────────────────

/// Crypto-sealed proximity proof.
///
/// Maps a proximity proof into ALICE-Crypto sealed format so the Crypto
/// layer can store and verify proximity claims using its existing
/// authenticated encryption infrastructure.
pub struct PresenceCryptoSealed {
    /// FNV-1a hash over distance, threshold, is_proximate, coord_hash_a, coord_hash_b, seal_hash bytes.
    pub content_hash: u64,
    /// Vivaldi distance between the two parties.
    pub distance: f64,
    /// Proximity threshold.
    pub threshold: f64,
    /// Whether the distance is within threshold.
    pub is_proximate: bool,
    /// Coordinate hash of party A.
    pub coord_hash_a: u64,
    /// Coordinate hash of party B.
    pub coord_hash_b: u64,
    /// Seal hash: fnv1a of all proof fields.
    pub seal_hash: u64,
    /// Sealed byte size: 57 (8*5 fields + 1 bool + 16 AEAD overhead).
    pub sealed_bytes: usize,
}

/// Convert a proximity proof into a crypto-sealed proof.
#[inline]
pub fn presence_proof_to_crypto_sealed(proof: &ProximityProof) -> PresenceCryptoSealed {
    // Compute seal hash: fnv1a of all proof fields
    let mut seal_key = [0u8; 41];
    seal_key[0..8].copy_from_slice(&proof.distance.to_bits().to_le_bytes());
    seal_key[8..16].copy_from_slice(&proof.threshold.to_bits().to_le_bytes());
    seal_key[16] = proof.is_proximate as u8;
    seal_key[17..25].copy_from_slice(&proof.coord_hash_a.to_le_bytes());
    seal_key[25..33].copy_from_slice(&proof.coord_hash_b.to_le_bytes());
    seal_key[33..41].copy_from_slice(&proof.content_hash.to_le_bytes());
    let seal_hash = fnv1a(&seal_key);

    // Content hash over all fields including derived seal_hash
    let mut key = [0u8; 49];
    key[0..8].copy_from_slice(&proof.distance.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&proof.threshold.to_bits().to_le_bytes());
    key[16] = proof.is_proximate as u8;
    key[17..25].copy_from_slice(&proof.coord_hash_a.to_le_bytes());
    key[25..33].copy_from_slice(&proof.coord_hash_b.to_le_bytes());
    key[33..41].copy_from_slice(&seal_hash.to_le_bytes());
    key[41..49].copy_from_slice(&(57usize as u64).to_le_bytes());

    PresenceCryptoSealed {
        content_hash: fnv1a(&key),
        distance: proof.distance,
        threshold: proof.threshold,
        is_proximate: proof.is_proximate,
        coord_hash_a: proof.coord_hash_a,
        coord_hash_b: proof.coord_hash_b,
        seal_hash,
        sealed_bytes: 57,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_presence::{VivaldiCoord, PresenceEvent, IdentityCommitment, ProximityProof};

    // ── Bridge 1: event → sync frame ──────────────────────────────────

    #[test]
    fn test_presence_event_to_sync_frame_basic() {
        let event = PresenceEvent::new(10, 20, 5000);
        let frame = presence_event_to_sync_frame(&event);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.party_a_id, 10);
        assert_eq!(frame.party_b_id, 20);
        assert_eq!(frame.timestamp_ns, 5000);
        assert_eq!(frame.wire_bytes, 18);
        assert!(!frame.is_mutual);
        assert!(!frame.is_verified);
        assert_eq!(frame.frame_type, 0x50);
    }

    #[test]
    fn test_presence_event_to_sync_frame_mutual_verified() {
        let mut event = PresenceEvent::new(1, 2, 1000);
        event.set_mutual();
        event.set_verified();
        let frame = presence_event_to_sync_frame(&event);
        assert!(frame.is_mutual);
        assert!(frame.is_verified);
        assert_eq!(frame.frame_type, 0x50);
    }

    #[test]
    fn test_presence_event_to_sync_frame_deterministic() {
        let event = PresenceEvent::new(5, 6, 999);
        let f1 = presence_event_to_sync_frame(&event);
        let f2 = presence_event_to_sync_frame(&event);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn test_presence_event_to_sync_frame_different_flags_differ() {
        let e1 = PresenceEvent::new(1, 2, 0);
        let mut e2 = PresenceEvent::new(1, 2, 0);
        e2.set_mutual();
        let f1 = presence_event_to_sync_frame(&e1);
        let f2 = presence_event_to_sync_frame(&e2);
        assert_ne!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 2: identity → auth token ───────────────────────────────

    #[test]
    fn test_presence_identity_to_auth_token() {
        let commitment = IdentityCommitment::new(42, 12345, 9_000_000);
        let token = presence_identity_to_auth_token(&commitment);
        assert_ne!(token.content_hash, 0);
        assert_eq!(token.commitment_hash, commitment.commitment_hash);
        assert_eq!(token.nonce, 12345);
        assert_eq!(token.timestamp_ns, 9_000_000);
        assert_ne!(token.token_hash, 0);
        assert_eq!(token.ttl_secs, 3600);
        // Token hash must differ from content hash
        assert_ne!(token.token_hash, token.content_hash);
    }

    #[test]
    fn test_presence_identity_to_auth_token_deterministic() {
        let commitment = IdentityCommitment::new(99, 777, 1_000_000);
        let t1 = presence_identity_to_auth_token(&commitment);
        let t2 = presence_identity_to_auth_token(&commitment);
        assert_eq!(t1.content_hash, t2.content_hash);
        assert_eq!(t1.token_hash, t2.token_hash);
    }

    #[test]
    fn test_presence_identity_to_auth_token_different_nonce_differs() {
        let c1 = IdentityCommitment::new(42, 111, 1000);
        let c2 = IdentityCommitment::new(42, 222, 1000);
        let t1 = presence_identity_to_auth_token(&c1);
        let t2 = presence_identity_to_auth_token(&c2);
        assert_ne!(t1.token_hash, t2.token_hash);
        assert_ne!(t1.content_hash, t2.content_hash);
    }

    #[test]
    fn test_presence_identity_to_auth_token_verify_round_trip() {
        let secret_id: u64 = 42;
        let commitment = IdentityCommitment::new(secret_id, 55555, 2_000_000);
        assert!(commitment.verify(secret_id));
        let token = presence_identity_to_auth_token(&commitment);
        // Token should reflect valid commitment
        assert_eq!(token.commitment_hash, commitment.commitment_hash);
    }

    // ── Bridge 3: proof → crypto sealed ───────────────────────────────

    #[test]
    fn test_presence_proof_to_crypto_sealed_proximate() {
        let a = VivaldiCoord::new(0.0, 0.0);
        let b = VivaldiCoord::new(3.0, 4.0); // distance = 5.0
        let proof = ProximityProof::prove(&a, &b, 10.0);
        let sealed = presence_proof_to_crypto_sealed(&proof);
        assert_ne!(sealed.content_hash, 0);
        assert!((sealed.distance - 5.0).abs() < 1e-10);
        assert!((sealed.threshold - 10.0).abs() < 1e-10);
        assert!(sealed.is_proximate);
        assert_ne!(sealed.coord_hash_a, 0);
        assert_ne!(sealed.coord_hash_b, 0);
        assert_ne!(sealed.seal_hash, 0);
        assert_eq!(sealed.sealed_bytes, 57);
        // Seal hash must differ from content hash
        assert_ne!(sealed.seal_hash, sealed.content_hash);
    }

    #[test]
    fn test_presence_proof_to_crypto_sealed_not_proximate() {
        let a = VivaldiCoord::new(0.0, 0.0);
        let b = VivaldiCoord::new(100.0, 0.0); // distance = 100.0
        let proof = ProximityProof::prove(&a, &b, 10.0);
        let sealed = presence_proof_to_crypto_sealed(&proof);
        assert!(!sealed.is_proximate);
        assert!((sealed.distance - 100.0).abs() < 1e-10);
        assert_eq!(sealed.sealed_bytes, 57);
    }

    #[test]
    fn test_presence_proof_to_crypto_sealed_deterministic() {
        let a = VivaldiCoord::new(1.0, 2.0);
        let b = VivaldiCoord::new(4.0, 6.0);
        let proof = ProximityProof::prove(&a, &b, 10.0);
        let s1 = presence_proof_to_crypto_sealed(&proof);
        let s2 = presence_proof_to_crypto_sealed(&proof);
        assert_eq!(s1.content_hash, s2.content_hash);
        assert_eq!(s1.seal_hash, s2.seal_hash);
    }

    #[test]
    fn test_presence_proof_to_crypto_sealed_with_height() {
        let a = VivaldiCoord::with_height(0.0, 0.0, 5.0);
        let b = VivaldiCoord::with_height(3.0, 4.0, 5.0);
        let proof = ProximityProof::prove(&a, &b, 20.0);
        let sealed = presence_proof_to_crypto_sealed(&proof);
        assert!(sealed.is_proximate);
        assert!(sealed.distance > 0.0);
        assert_eq!(sealed.sealed_bytes, 57);
    }
}
