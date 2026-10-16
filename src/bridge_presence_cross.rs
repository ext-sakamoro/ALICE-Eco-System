//! Cross-domain bridges — ALICE-Presence ↔ Sync, Auth, Crypto
//!
//! 3 bridges connecting presence protocol events to sync frame transport,
//! authentication token generation, and cryptographic proof sealing.

use alice_presence::{Challenge, ChallengeProof, PresenceEvent, ProofError, ProximityProof};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: PresenceEvent → Sync (InputFrame equivalent) ──────────

/// Sync frame record derived from a presence event.
///
/// Maps a presence event into ALICE-Sync InputFrame-compatible metadata
/// so the Sync layer can transport presence events over its existing
/// delta-compression and ordered-delivery infrastructure.
pub struct PresenceSyncFrame {
    /// FNV-1a hash over `party_a_id`, `party_b_id`, `timestamp_ns`, flags bytes.
    pub content_hash: u64,
    /// Party A compact ID.
    pub party_a_id: u32,
    /// Party B compact ID.
    pub party_b_id: u32,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Wire size in bytes (always 18, same as `PresenceEvent`).
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
#[must_use]
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

// ── Bridge 2: ChallengeProof → Auth (verification token) ────────────

/// Auth verification token issued for a verified presence identity.
///
/// Issued only for a [`ChallengeProof`] whose Ed25519 signature verifies over
/// the verifier's own challenge and the encounter transcript, so a token names
/// a key that answered this verifier. The hashes are FNV-1a identifiers for the
/// Auth layer's bookkeeping, not authentication: the authentication is the
/// signature check done before the token exists.
pub struct PresenceAuthToken {
    /// FNV-1a hash over `key_hash`, `challenge_hash`, `timestamp_ns`, `token_hash` bytes.
    pub content_hash: u64,
    /// The prover's Ed25519 public key.
    pub public_key: [u8; 32],
    /// FNV-1a hash of the public key.
    pub key_hash: u64,
    /// FNV-1a hash of the challenge the proof answered.
    pub challenge_hash: u64,
    /// Timestamp (nanoseconds).
    pub timestamp_ns: u64,
    /// Token hash: fnv1a of `key_hash` + `challenge_hash` + timestamp.
    pub token_hash: u64,
    /// Time-to-live in seconds (3600 = 1 hour).
    pub ttl_secs: u32,
}

/// Issue an auth token for a presence proof that verifies.
///
/// `expected_challenge` is the challenge this verifier issued for the
/// exchange (drawn by the verifier, so a replayed proof does not match) and
/// `transcript` the encounter transcript the proof signs
/// (`CrossingRecord::transcript`).
///
/// # Errors
///
/// The [`ProofError`] of [`ChallengeProof::verify`]: a challenge that is not
/// the expected one, an invalid public key or a signature that does not
/// verify. No token is issued then.
#[inline]
pub fn presence_proof_to_auth_token(
    proof: &ChallengeProof,
    expected_challenge: &Challenge,
    transcript: &[u8],
    timestamp_ns: u64,
) -> Result<PresenceAuthToken, ProofError> {
    proof.verify(expected_challenge, transcript)?;
    let key_hash = fnv1a(&proof.public_key);
    let challenge_hash = fnv1a(&proof.challenge.0);

    let mut tok_key = [0u8; 24];
    tok_key[0..8].copy_from_slice(&key_hash.to_le_bytes());
    tok_key[8..16].copy_from_slice(&challenge_hash.to_le_bytes());
    tok_key[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());
    let token_hash = fnv1a(&tok_key);

    let mut key = [0u8; 32];
    key[0..24].copy_from_slice(&tok_key);
    key[24..32].copy_from_slice(&token_hash.to_le_bytes());

    Ok(PresenceAuthToken {
        content_hash: fnv1a(&key),
        public_key: proof.public_key,
        key_hash,
        challenge_hash,
        timestamp_ns,
        token_hash,
        ttl_secs: 3600,
    })
}

// ── Bridge 3: ProximityProof → Crypto (sealed proof) ────────────────

/// Crypto-sealed proximity proof.
///
/// Maps a proximity proof into ALICE-Crypto sealed format so the Crypto
/// layer can store and verify proximity claims using its existing
/// authenticated encryption infrastructure.
pub struct PresenceCryptoSealed {
    /// FNV-1a hash over distance, threshold, `is_proximate`, `coord_hash_a`, `coord_hash_b`, `seal_hash` bytes.
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
#[must_use]
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
    use alice_presence::{
        execute_presence_protocol, CrossingRecord, ExchangeChallenges, Identity, PartyInfo,
        PresenceConfig, PresenceEvent, ProximityProof, VivaldiCoord,
    };

    fn crossing(seed: u8) -> (CrossingRecord, ExchangeChallenges) {
        let ia = Identity::from_seed([0xA1; 32]);
        let ib = Identity::from_seed([0xB0; 32]);
        let a = PartyInfo::new(VivaldiCoord::new(0.0, 0.0), &ia, 1);
        let b = PartyInfo::new(VivaldiCoord::new(3.0, 4.0), &ib, 2);
        let ch = ExchangeChallenges::from_seed([seed; 32]);
        let r = execute_presence_protocol(&a, &b, &ch, 1_000_000, &PresenceConfig::default())
            .expect("proximate");
        (r, ch)
    }

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

    // ── Bridge 2: verified proof → auth token ─────────────────────────

    #[test]
    fn test_presence_proof_to_auth_token() {
        let (r, ch) = crossing(1);
        let token = presence_proof_to_auth_token(&r.proof_a, &ch.for_a, &r.transcript(), 9_000_000)
            .expect("an honest proof verifies");
        assert_eq!(
            token.public_key,
            Identity::from_seed([0xA1; 32]).public_key()
        );
        assert_eq!(token.timestamp_ns, 9_000_000);
        assert_eq!(token.ttl_secs, 3600);
        assert_ne!(token.token_hash, 0);
        assert_ne!(token.token_hash, token.content_hash);
        let again = presence_proof_to_auth_token(&r.proof_a, &ch.for_a, &r.transcript(), 9_000_000)
            .unwrap();
        assert_eq!(again.content_hash, token.content_hash);
    }

    #[test]
    fn test_presence_proof_to_auth_token_keys_and_challenges_differ() {
        let (r, ch) = crossing(1);
        let ta = presence_proof_to_auth_token(&r.proof_a, &ch.for_a, &r.transcript(), 5).unwrap();
        let tb = presence_proof_to_auth_token(&r.proof_b, &ch.for_b, &r.transcript(), 5).unwrap();
        assert_ne!(ta.key_hash, tb.key_hash);
        assert_ne!(ta.token_hash, tb.token_hash);
        let (r2, ch2) = crossing(2);
        let t2 =
            presence_proof_to_auth_token(&r2.proof_a, &ch2.for_a, &r2.transcript(), 5).unwrap();
        assert_ne!(ta.challenge_hash, t2.challenge_hash);
        assert_ne!(ta.content_hash, t2.content_hash);
    }

    /// A forged or replayed proof gets no token.
    #[test]
    fn test_presence_proof_to_auth_token_refuses_forged_and_replayed() {
        let (r, ch) = crossing(1);
        let mut forged = r.proof_a;
        forged.signature = [0u8; 64];
        assert_eq!(
            presence_proof_to_auth_token(&forged, &ch.for_a, &r.transcript(), 5).err(),
            Some(ProofError::InvalidSignature)
        );
        let mut flipped = r.proof_a;
        flipped.signature[10] ^= 1;
        assert!(presence_proof_to_auth_token(&flipped, &ch.for_a, &r.transcript(), 5).is_err());
        // a proof for another exchange's challenge (replay) is refused
        let (_, other) = crossing(2);
        assert_eq!(
            presence_proof_to_auth_token(&r.proof_a, &other.for_a, &r.transcript(), 5).err(),
            Some(ProofError::ChallengeMismatch)
        );
        // the transcript of another encounter does not match the signature
        let (r2, _) = crossing(2);
        assert!(presence_proof_to_auth_token(&r.proof_a, &ch.for_a, &r2.transcript(), 5).is_err());
        // B's proof does not answer A's challenge
        assert!(presence_proof_to_auth_token(&r.proof_b, &ch.for_a, &r.transcript(), 5).is_err());
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
