//! Auth bridges — ALICE-Auth ↔ DB, Cache, Crypto, API, CDN, Edge, DNS, Sync
//!
//! 8 bridges connecting Ed25519 ZKP authentication to the ALICE ecosystem.

use alice_auth::{AliceId, AliceSig, Identity};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Auth → DB (identity persistence) ─────────────────────────

/// Identity record for ALICE-DB persistence.
pub struct AuthDbRecord {
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// DID string representation.
    pub did_string: String,
}

/// Serialize identity for ALICE-DB storage.
pub fn auth_to_db_record(id: &AliceId) -> AuthDbRecord {
    let bytes = *id.as_bytes();
    let hash = fnv1a(&bytes);
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    AuthDbRecord {
        id_bytes: bytes,
        content_hash: hash,
        did_string: format!("did:alice:{}", &hex[..32]),
    }
}

// ── Bridge 2: Auth → Cache (session token caching) ─────────────────────

/// Session token cache entry for ALICE-Cache.
pub struct AuthCacheToken {
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Content hash for cache key.
    pub content_hash: u64,
    /// TTL in seconds.
    pub ttl_secs: u32,
}

/// Prepare identity for ALICE-Cache session token.
pub fn auth_to_cache_token(id: &AliceId, ttl_secs: u32) -> AuthCacheToken {
    let bytes = *id.as_bytes();
    AuthCacheToken { id_bytes: bytes, content_hash: fnv1a(&bytes), ttl_secs }
}

// ── Bridge 3: Auth → Crypto (identity key for SSS backup) ──────────────

/// Identity backup metadata for ALICE-Crypto SSS protection.
pub struct AuthCryptoBackup {
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Hash of seed material.
    pub seed_hash: u64,
    /// Payload size (seed = 32 bytes).
    pub payload_bytes: usize,
}

/// Prepare identity seed for ALICE-Crypto SSS backup.
pub fn auth_to_crypto_backup(identity: &Identity) -> AuthCryptoBackup {
    let id_bytes = *identity.id().as_bytes();
    let seed = identity.seed();
    AuthCryptoBackup { id_bytes, seed_hash: fnv1a(&seed), payload_bytes: 32 }
}

// ── Bridge 4: Auth → API (API gateway authentication) ───────────────────

/// API gateway verification result.
pub struct AuthApiGateway {
    /// Identity hex (first 16 bytes).
    pub id_hex: String,
    /// Signature size in bytes.
    pub signature_bytes: usize,
    /// Whether signature verified.
    pub verified: bool,
}

/// Verify signature for ALICE-API gateway authentication.
pub fn auth_to_api_verify(id: &AliceId, message: &[u8], sig: &AliceSig) -> AuthApiGateway {
    let verified = alice_auth::ok(id, message, sig);
    let hex: String = id.as_bytes()[..16].iter().map(|b| format!("{:02x}", b)).collect();
    AuthApiGateway { id_hex: hex, signature_bytes: 64, verified }
}

// ── Bridge 5: Auth → CDN (authenticated content token) ──────────────────

/// Authenticated content token for ALICE-CDN.
pub struct AuthCdnToken {
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Content hash for CDN routing.
    pub content_hash: u64,
    /// Token size.
    pub token_size: usize,
}

/// Generate authenticated content token for ALICE-CDN.
pub fn auth_to_cdn_token(id: &AliceId) -> AuthCdnToken {
    let bytes = *id.as_bytes();
    AuthCdnToken { id_bytes: bytes, content_hash: fnv1a(&bytes), token_size: 32 }
}

// ── Bridge 6: Auth → Edge (IoT device authentication) ───────────────────

/// IoT device authentication config for ALICE-Edge.
pub struct AuthEdgeDevice {
    /// Device identity public key.
    pub device_id: [u8; 32],
    /// Challenge size in bytes.
    pub challenge_bytes: usize,
    /// Protocol version.
    pub protocol_version: u8,
}

/// Configure IoT device authentication for ALICE-Edge.
pub fn auth_to_edge_device(id: &AliceId) -> AuthEdgeDevice {
    AuthEdgeDevice { device_id: *id.as_bytes(), challenge_bytes: 32, protocol_version: 1 }
}

// ── Bridge 7: Auth → DNS (DNS-based identity fingerprint) ───────────────

/// DNS TXT record fingerprint from identity.
pub struct AuthDnsFingerprint {
    /// Hex fingerprint (first 16 bytes of identity).
    pub fingerprint: String,
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// DNS record type.
    pub record_type: &'static str,
}

/// Create DNS TXT record fingerprint from ALICE-Auth identity.
pub fn auth_to_dns_fingerprint(id: &AliceId) -> AuthDnsFingerprint {
    let bytes = *id.as_bytes();
    let fp: String = bytes[..16].iter().map(|b| format!("{:02x}", b)).collect();
    AuthDnsFingerprint { fingerprint: fp, id_bytes: bytes, record_type: "TXT" }
}

// ── Bridge 8: Auth → Sync (authenticated multiplayer session) ───────────

/// Authenticated multiplayer session for ALICE-Sync.
pub struct AuthSyncSession {
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Content hash for session key.
    pub content_hash: u64,
    /// Player slot.
    pub player_slot: u8,
}

/// Create authenticated session for ALICE-Sync multiplayer.
pub fn auth_to_sync_session(id: &AliceId, player_slot: u8) -> AuthSyncSession {
    let bytes = *id.as_bytes();
    AuthSyncSession { id_bytes: bytes, content_hash: fnv1a(&bytes), player_slot }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> Identity {
        Identity::from_seed(&[42u8; 32])
    }

    #[test]
    fn test_auth_to_db_record() {
        let id = test_identity().id();
        let rec = auth_to_db_record(&id);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.did_string.starts_with("did:alice:"));
    }

    #[test]
    fn test_auth_to_cache_token() {
        let id = test_identity().id();
        let tok = auth_to_cache_token(&id, 3600);
        assert_eq!(tok.ttl_secs, 3600);
        assert_ne!(tok.content_hash, 0);
    }

    #[test]
    fn test_auth_to_crypto_backup() {
        let identity = test_identity();
        let backup = auth_to_crypto_backup(&identity);
        assert_ne!(backup.seed_hash, 0);
        assert_eq!(backup.payload_bytes, 32);
    }

    #[test]
    fn test_auth_to_api_verify() {
        let identity = test_identity();
        let msg = b"hello";
        let sig = identity.sign(msg);
        let result = auth_to_api_verify(&identity.id(), msg, &sig);
        assert!(result.verified);
        assert_eq!(result.signature_bytes, 64);
    }

    #[test]
    fn test_auth_to_cdn_token() {
        let id = test_identity().id();
        let tok = auth_to_cdn_token(&id);
        assert_eq!(tok.token_size, 32);
        assert_ne!(tok.content_hash, 0);
    }

    #[test]
    fn test_auth_to_edge_device() {
        let id = test_identity().id();
        let dev = auth_to_edge_device(&id);
        assert_eq!(dev.challenge_bytes, 32);
        assert_eq!(dev.protocol_version, 1);
    }

    #[test]
    fn test_auth_to_dns_fingerprint() {
        let id = test_identity().id();
        let fp = auth_to_dns_fingerprint(&id);
        assert_eq!(fp.fingerprint.len(), 32); // 16 bytes = 32 hex chars
        assert_eq!(fp.record_type, "TXT");
    }

    #[test]
    fn test_auth_to_sync_session() {
        let id = test_identity().id();
        let session = auth_to_sync_session(&id, 3);
        assert_eq!(session.player_slot, 3);
        assert_ne!(session.content_hash, 0);
    }
}
