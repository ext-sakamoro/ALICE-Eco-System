//! Auth extended bridges — ALICE-Auth advanced features ↔ Sync, Crypto, API, Edge, Cache
//!
//! 9 bridges connecting Auth's NIZK, Recovery, RBAC, Key Rotation, HD derivation,
//! Endorsement Expiry, N-Gen Rotation, Revocation Purge, and Token Expiry
//! to the wider ALICE ecosystem.

use alice_auth::{AliceId, Identity};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Auth NIZK → Sync (ZKP-authenticated multiplayer session) ──

/// Schnorr NIZK proof metadata for ALICE-Sync ZKP-authenticated sessions.
pub struct AuthNizkSyncSession {
    /// Content hash for session deduplication.
    pub content_hash: u64,
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Proof size in bytes (commitment 32 + response 32).
    pub proof_bytes: usize,
    /// Whether this session requires ZKP (vs standard signature).
    pub zkp_required: bool,
}

/// Prepare ZKP-authenticated session metadata for ALICE-Sync.
#[inline]
#[must_use]
pub fn auth_nizk_to_sync_session(id: &AliceId, zkp_required: bool) -> AuthNizkSyncSession {
    let bytes = *id.as_bytes();
    let zkp_flag = zkp_required as usize;
    AuthNizkSyncSession {
        content_hash: fnv1a(&bytes),
        id_bytes: bytes,
        proof_bytes: 64 * zkp_flag + 64 * (1 - zkp_flag),
        zkp_required,
    }
}

// ── Bridge 2: Auth Recovery → Crypto (seed shard distribution) ───────────

/// Seed recovery shard metadata for ALICE-Crypto SSS distribution.
pub struct AuthRecoveryCryptoShard {
    /// Content hash of the identity being backed up.
    pub content_hash: u64,
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Total number of shards (N).
    pub total_shards: u8,
    /// Threshold for recovery (K).
    pub threshold: u8,
    /// Seed payload size.
    pub seed_bytes: usize,
}

/// Prepare seed recovery metadata for ALICE-Crypto SSS shard distribution.
#[inline]
#[must_use]
pub fn auth_recovery_to_crypto_shard(
    identity: &Identity,
    total_shards: u8,
    threshold: u8,
) -> AuthRecoveryCryptoShard {
    let id_bytes = *identity.id().as_bytes();
    AuthRecoveryCryptoShard {
        content_hash: fnv1a(&id_bytes),
        id_bytes,
        total_shards,
        threshold,
        seed_bytes: 32,
    }
}

// ── Bridge 3: Auth RBAC → API (role-based access control metadata) ───────

/// RBAC role metadata for ALICE-API gateway policy enforcement.
pub struct AuthRbacApiPolicy {
    /// Content hash of identity + role.
    pub content_hash: u64,
    /// Identity public key bytes.
    pub id_bytes: [u8; 32],
    /// Role bitmask (bit 0=Read, 1=Write, 2=Admin, 3=Execute).
    pub role_mask: u8,
    /// Whether admin privileges are granted.
    pub is_admin: bool,
}

/// Prepare RBAC role metadata for ALICE-API gateway.
#[inline]
#[must_use]
pub fn auth_rbac_to_api_policy(id: &AliceId, role_mask: u8) -> AuthRbacApiPolicy {
    let bytes = *id.as_bytes();
    let mut hash_input = [0u8; 33];
    hash_input[..32].copy_from_slice(&bytes);
    hash_input[32] = role_mask;
    let is_admin = (role_mask & 0x04) != 0;
    AuthRbacApiPolicy {
        content_hash: fnv1a(&hash_input),
        id_bytes: bytes,
        role_mask,
        is_admin,
    }
}

// ── Bridge 4: Auth KeyRotation → Edge (device key refresh) ───────────────

/// Key rotation state for ALICE-Edge `IoT` device refresh.
pub struct AuthRotationEdgeRefresh {
    /// Content hash of current + previous key state.
    pub content_hash: u64,
    /// Current identity public key bytes.
    pub current_id: [u8; 32],
    /// Whether a previous key exists (grace period active).
    pub has_previous: bool,
    /// Grace period expiry timestamp (ms), 0 if no previous key.
    pub grace_expiry_ms: u64,
}

/// Prepare key rotation state for ALICE-Edge device key refresh.
#[inline]
#[must_use]
pub fn auth_rotation_to_edge_refresh(
    current_id: &AliceId,
    has_previous: bool,
    grace_expiry_ms: u64,
) -> AuthRotationEdgeRefresh {
    let bytes = *current_id.as_bytes();
    let flag = has_previous as u8;
    let mut hash_input = [0u8; 33];
    hash_input[..32].copy_from_slice(&bytes);
    hash_input[32] = flag;
    AuthRotationEdgeRefresh {
        content_hash: fnv1a(&hash_input),
        current_id: bytes,
        has_previous,
        grace_expiry_ms: grace_expiry_ms * (flag as u64),
    }
}

// ── Bridge 5: Auth HD → Cache (child key session cache) ──────────────────

/// HD-derived child key metadata for ALICE-Cache session management.
pub struct AuthHdCacheEntry {
    /// Content hash of parent + child index.
    pub content_hash: u64,
    /// Parent identity public key bytes.
    pub parent_id: [u8; 32],
    /// Child derivation index.
    pub child_index: u32,
    /// TTL in seconds for cache entry.
    pub ttl_secs: u32,
}

/// Prepare HD child key metadata for ALICE-Cache session caching.
#[inline]
#[must_use]
pub fn auth_hd_to_cache_entry(
    parent_id: &AliceId,
    child_index: u32,
    ttl_secs: u32,
) -> AuthHdCacheEntry {
    let bytes = *parent_id.as_bytes();
    let idx_bytes = child_index.to_le_bytes();
    let mut hash_input = [0u8; 36];
    hash_input[..32].copy_from_slice(&bytes);
    hash_input[32..36].copy_from_slice(&idx_bytes);
    AuthHdCacheEntry {
        content_hash: fnv1a(&hash_input),
        parent_id: bytes,
        child_index,
        ttl_secs,
    }
}

// ── Bridge 6: Auth Endorsement Expiry → Cache TTL ─────────────────────

/// Endorsement expiry metadata for ALICE-Cache TTL enforcement.
pub struct AuthEndorsementCacheTtl {
    /// Content hash of endorser + endorsed.
    pub content_hash: u64,
    /// Endorser public key bytes.
    pub endorser_id: [u8; 32],
    /// Endorsed public key bytes.
    pub endorsed_id: [u8; 32],
    /// Remaining TTL in seconds (branchless: 0 if expired).
    pub ttl_secs: u32,
    /// Whether the endorsement is still valid.
    pub is_valid: bool,
}

/// Convert endorsement expiry into Cache TTL metadata.
#[inline]
#[must_use]
pub fn auth_endorsement_to_cache_ttl(
    endorser: &AliceId,
    endorsed: &AliceId,
    expires_ms: u64,
    now_ms: u64,
) -> AuthEndorsementCacheTtl {
    let er = *endorser.as_bytes();
    let ed = *endorsed.as_bytes();
    let mut hash_input = [0u8; 64];
    hash_input[..32].copy_from_slice(&er);
    hash_input[32..64].copy_from_slice(&ed);
    // Branchless TTL: 0 if expired
    let expired = (now_ms > expires_ms) as u32;
    let remaining_ms = expires_ms.saturating_sub(now_ms);
    let ttl_secs = ((remaining_ms / 1000) as u32) * (1 - expired);
    AuthEndorsementCacheTtl {
        content_hash: fnv1a(&hash_input),
        endorser_id: er,
        endorsed_id: ed,
        ttl_secs,
        is_valid: expired == 0,
    }
}

// ── Bridge 7: Auth N-Gen Rotation → Edge Device Management ────────────

/// N-generation rotation state for ALICE-Edge multi-device key management.
pub struct AuthRotationNgenEdge {
    /// Content hash of current key + generation count.
    pub content_hash: u64,
    /// Current identity public key bytes.
    pub current_id: [u8; 32],
    /// Number of retained previous generations.
    pub generation_count: u8,
    /// Whether any previous keys exist.
    pub has_previous: bool,
    /// Most recent rotation timestamp (0 if no rotation).
    pub last_rotation_ms: u64,
}

/// Convert N-generation rotation state into Edge device key metadata.
#[inline]
#[must_use]
pub fn auth_rotation_ngen_to_edge(
    current_id: &AliceId,
    generation_count: u8,
    last_rotation_ms: u64,
) -> AuthRotationNgenEdge {
    let bytes = *current_id.as_bytes();
    let mut hash_input = [0u8; 34];
    hash_input[..32].copy_from_slice(&bytes);
    hash_input[32] = generation_count;
    hash_input[33] = (generation_count > 0) as u8;
    AuthRotationNgenEdge {
        content_hash: fnv1a(&hash_input),
        current_id: bytes,
        generation_count,
        has_previous: generation_count > 0,
        last_rotation_ms: last_rotation_ms * (generation_count > 0) as u64,
    }
}

// ── Bridge 8: Auth Revocation Purge → Analytics ───────────────────────

/// Revocation purge result for ALICE-Analytics security metrics.
pub struct AuthRevocationPurgeAnalytics {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Number of entries purged.
    pub purged_count: u32,
    /// Remaining entries after purge.
    pub remaining_count: u32,
    /// Purge timestamp (ms).
    pub purge_ms: u64,
    /// Purge efficiency (purged / total before, scaled 0-255).
    pub efficiency_u8: u8,
}

/// Convert revocation `auto_purge` result into Analytics metrics.
#[inline]
#[must_use]
pub fn auth_revocation_purge_to_analytics(
    purged_count: u32,
    remaining_count: u32,
    now_ms: u64,
) -> AuthRevocationPurgeAnalytics {
    let total_before = purged_count.saturating_add(remaining_count);
    // 効率: purged / total_before → u8 (0-255)
    let efficiency = if total_before > 0 {
        ((purged_count as u64 * 255) / total_before as u64) as u8
    } else {
        0
    };
    let hash_data = purged_count.to_le_bytes();
    AuthRevocationPurgeAnalytics {
        content_hash: fnv1a(&hash_data),
        purged_count,
        remaining_count,
        purge_ms: now_ms,
        efficiency_u8: efficiency,
    }
}

// ── Bridge 9: Auth Token Expiry → API Gateway TTL ─────────────────────

/// Auth token expiry metadata for ALICE-API gateway session management.
pub struct AuthTokenApiGateway {
    /// Content hash of nonce + expiry.
    pub content_hash: u64,
    /// Token nonce (unique per session).
    pub nonce_ms: u64,
    /// Token expiry timestamp (ms).
    pub expires_ms: u64,
    /// Remaining TTL in seconds (branchless: 0 if expired).
    pub ttl_secs: u32,
    /// Whether the token is still valid.
    pub is_valid: bool,
}

/// Convert auth token expiry into API Gateway TTL metadata.
#[inline]
#[must_use]
pub fn auth_token_to_api_gateway(
    nonce_ms: u64,
    expires_ms: u64,
    now_ms: u64,
) -> AuthTokenApiGateway {
    let expired = (now_ms > expires_ms) as u32;
    let remaining_ms = expires_ms.saturating_sub(now_ms);
    let ttl_secs = ((remaining_ms / 1000) as u32) * (1 - expired);
    let mut hash_input = [0u8; 16];
    hash_input[..8].copy_from_slice(&nonce_ms.to_le_bytes());
    hash_input[8..16].copy_from_slice(&expires_ms.to_le_bytes());
    AuthTokenApiGateway {
        content_hash: fnv1a(&hash_input),
        nonce_ms,
        expires_ms,
        ttl_secs,
        is_valid: expired == 0,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> Identity {
        Identity::from_seed(&[42u8; 32])
    }

    #[test]
    fn test_auth_nizk_to_sync_session_zkp() {
        let id = test_identity().id();
        let session = auth_nizk_to_sync_session(&id, true);
        assert_ne!(session.content_hash, 0);
        assert_eq!(session.proof_bytes, 64);
        assert!(session.zkp_required);
    }

    #[test]
    fn test_auth_nizk_to_sync_session_standard() {
        let id = test_identity().id();
        let session = auth_nizk_to_sync_session(&id, false);
        assert_eq!(session.proof_bytes, 64);
        assert!(!session.zkp_required);
    }

    #[test]
    fn test_auth_recovery_to_crypto_shard() {
        let identity = test_identity();
        let shard = auth_recovery_to_crypto_shard(&identity, 5, 3);
        assert_ne!(shard.content_hash, 0);
        assert_eq!(shard.total_shards, 5);
        assert_eq!(shard.threshold, 3);
        assert_eq!(shard.seed_bytes, 32);
    }

    #[test]
    fn test_auth_rbac_to_api_policy_admin() {
        let id = test_identity().id();
        let policy = auth_rbac_to_api_policy(&id, 0x07); // Read+Write+Admin
        assert_ne!(policy.content_hash, 0);
        assert!(policy.is_admin);
        assert_eq!(policy.role_mask, 0x07);
    }

    #[test]
    fn test_auth_rbac_to_api_policy_reader() {
        let id = test_identity().id();
        let policy = auth_rbac_to_api_policy(&id, 0x01); // Read only
        assert!(!policy.is_admin);
        assert_eq!(policy.role_mask, 0x01);
    }

    #[test]
    fn test_auth_rotation_to_edge_refresh_active() {
        let id = test_identity().id();
        let refresh = auth_rotation_to_edge_refresh(&id, true, 1_000_000);
        assert_ne!(refresh.content_hash, 0);
        assert!(refresh.has_previous);
        assert_eq!(refresh.grace_expiry_ms, 1_000_000);
    }

    #[test]
    fn test_auth_rotation_to_edge_refresh_no_previous() {
        let id = test_identity().id();
        let refresh = auth_rotation_to_edge_refresh(&id, false, 0);
        assert!(!refresh.has_previous);
        assert_eq!(refresh.grace_expiry_ms, 0);
    }

    #[test]
    fn test_auth_hd_to_cache_entry() {
        let id = test_identity().id();
        let entry = auth_hd_to_cache_entry(&id, 42, 3600);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.child_index, 42);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn test_hash_determinism() {
        let id = test_identity().id();
        let s1 = auth_nizk_to_sync_session(&id, true);
        let s2 = auth_nizk_to_sync_session(&id, true);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    #[test]
    fn test_different_ids_different_hashes() {
        let id_a = Identity::from_seed(&[1u8; 32]).id();
        let id_b = Identity::from_seed(&[2u8; 32]).id();
        let h_a = auth_nizk_to_sync_session(&id_a, true).content_hash;
        let h_b = auth_nizk_to_sync_session(&id_b, true).content_hash;
        assert_ne!(h_a, h_b);
    }

    // --- Bridge 6: Endorsement Expiry → Cache TTL ---

    #[test]
    fn test_endorsement_cache_ttl_valid() {
        let id = test_identity().id();
        let id2 = Identity::from_seed(&[99u8; 32]).id();
        let ttl = auth_endorsement_to_cache_ttl(&id, &id2, 10_000, 5_000);
        assert_ne!(ttl.content_hash, 0);
        assert!(ttl.is_valid);
        assert_eq!(ttl.ttl_secs, 5); // (10000-5000)/1000 = 5
    }

    #[test]
    fn test_endorsement_cache_ttl_expired() {
        let id = test_identity().id();
        let id2 = Identity::from_seed(&[99u8; 32]).id();
        let ttl = auth_endorsement_to_cache_ttl(&id, &id2, 5_000, 10_000);
        assert!(!ttl.is_valid);
        assert_eq!(ttl.ttl_secs, 0);
    }

    // --- Bridge 7: N-Gen Rotation → Edge ---

    #[test]
    fn test_rotation_ngen_edge_active() {
        let id = test_identity().id();
        let ngen = auth_rotation_ngen_to_edge(&id, 2, 50_000);
        assert_ne!(ngen.content_hash, 0);
        assert!(ngen.has_previous);
        assert_eq!(ngen.generation_count, 2);
        assert_eq!(ngen.last_rotation_ms, 50_000);
    }

    #[test]
    fn test_rotation_ngen_edge_no_previous() {
        let id = test_identity().id();
        let ngen = auth_rotation_ngen_to_edge(&id, 0, 0);
        assert!(!ngen.has_previous);
        assert_eq!(ngen.last_rotation_ms, 0);
    }

    // --- Bridge 8: Revocation Purge → Analytics ---

    #[test]
    fn test_revocation_purge_analytics() {
        let a = auth_revocation_purge_to_analytics(50, 100, 1_000_000);
        assert_ne!(a.content_hash, 0);
        assert_eq!(a.purged_count, 50);
        assert_eq!(a.remaining_count, 100);
        assert_eq!(a.purge_ms, 1_000_000);
        // 50/150 * 255 ≈ 85
        assert_eq!(a.efficiency_u8, 85);
    }

    #[test]
    fn test_revocation_purge_analytics_zero() {
        let a = auth_revocation_purge_to_analytics(0, 0, 0);
        assert_eq!(a.efficiency_u8, 0);
    }

    // --- Bridge 9: Token Expiry → API Gateway ---

    #[test]
    fn test_token_api_gateway_valid() {
        let g = auth_token_to_api_gateway(1000, 60_000, 30_000);
        assert_ne!(g.content_hash, 0);
        assert!(g.is_valid);
        assert_eq!(g.ttl_secs, 30); // (60000-30000)/1000 = 30
    }

    #[test]
    fn test_token_api_gateway_expired() {
        let g = auth_token_to_api_gateway(1000, 30_000, 60_000);
        assert!(!g.is_valid);
        assert_eq!(g.ttl_secs, 0);
    }

    #[test]
    fn test_endorsement_cache_hash_determinism() {
        let id = test_identity().id();
        let id2 = Identity::from_seed(&[99u8; 32]).id();
        let a = auth_endorsement_to_cache_ttl(&id, &id2, 10_000, 5_000);
        let b = auth_endorsement_to_cache_ttl(&id, &id2, 10_000, 5_000);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
