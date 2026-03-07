//! Config bridges — ALICE-Config ↔ DB, Cache, Analytics, Auth, Edge
//!
//! 5 bridges connecting the configuration layer to the ALICE ecosystem.

use alice_config::{ConfigStore, ConfigValue, FeatureFlag, flag_enabled};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Config → DB (config persistence) ───────────────────────────

/// Config snapshot record for ALICE-DB.
///
/// Written when the active configuration changes so that auditors can replay
/// the configuration history and correlate config changes with incidents.
pub struct ConfigDbSnapshot {
    /// FNV-1a hash of all merged config values — DB row key.
    pub content_hash: u64,
    /// Number of configuration layers in the store.
    pub layer_count: usize,
    /// Number of merged keys in the final configuration.
    pub key_count: usize,
    /// Snapshot creation timestamp in milliseconds.
    pub created_at_ms: u64,
    /// True when at least one layer has been added.
    pub is_active: bool,
}

/// Build a config snapshot record for ALICE-DB.
#[inline]
#[must_use]
pub fn config_to_db_snapshot(store: &ConfigStore, created_at_ms: u64) -> ConfigDbSnapshot {
    let merged = store.merged();
    // マージ済みキーを結合してハッシュ計算
    let mut hash_data: Vec<u8> = Vec::new();
    for (k, v) in &merged {
        hash_data.extend_from_slice(k.as_bytes());
        hash_data.push(b'=');
        match v {
            ConfigValue::String(s) => hash_data.extend_from_slice(s.as_bytes()),
            ConfigValue::Int(n) => hash_data.extend_from_slice(&n.to_le_bytes()),
            ConfigValue::Float(f) => hash_data.extend_from_slice(&f.to_bits().to_le_bytes()),
            ConfigValue::Bool(b) => hash_data.push(*b as u8),
        }
        hash_data.push(b'|');
    }
    let content_hash = if hash_data.is_empty() { fnv1a(b"empty") } else { fnv1a(&hash_data) };
    ConfigDbSnapshot {
        content_hash,
        layer_count: store.layer_count(),
        key_count: merged.len(),
        created_at_ms,
        is_active: store.layer_count() > 0,
    }
}

// ── Bridge 2: Config → Cache (config cache) ───────────────────────────────

/// Cached config value entry for ALICE-Cache.
///
/// Individual config values are cached so that hot-path code can read them
/// without locking the `ConfigStore`. TTL is branchlessly set to 300 seconds
/// for stable values and 30 seconds for volatile (Bool) values.
pub struct ConfigCacheEntry {
    /// FNV-1a hash of the config key — cache key.
    pub content_hash: u64,
    /// Value type discriminant: 0=String, 1=Int, 2=Float, 3=Bool.
    pub value_type: u8,
    /// True when the value was found in the store.
    pub found: bool,
    /// Cache TTL in seconds (branchless: 30 for Bool, 300 for others).
    pub ttl_secs: u32,
    /// Encoded value fingerprint (FNV-1a of serialized value bytes).
    pub value_hash: u64,
}

/// Build a cached config value entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn config_to_cache_entry(store: &ConfigStore, key: &str) -> ConfigCacheEntry {
    let content_hash = fnv1a(key.as_bytes());
    match store.get(key) {
        Some(val) => {
            let (value_type, value_bytes): (u8, Vec<u8>) = match val {
                ConfigValue::String(s) => (0, s.as_bytes().to_vec()),
                ConfigValue::Int(n) => (1, n.to_le_bytes().to_vec()),
                ConfigValue::Float(f) => (2, f.to_bits().to_le_bytes().to_vec()),
                ConfigValue::Bool(b) => (3, vec![*b as u8]),
            };
            let value_hash = fnv1a(&value_bytes);
            // ブランチレスTTL: Bool(3) → 30秒、それ以外 → 300秒
            let is_bool = (value_type == 3) as u32;
            let ttl_secs = 300 - is_bool * 270;
            ConfigCacheEntry {
                content_hash,
                value_type,
                found: true,
                ttl_secs,
                value_hash,
            }
        }
        None => ConfigCacheEntry {
            content_hash,
            value_type: 0,
            found: false,
            ttl_secs: 10,
            value_hash: 0,
        },
    }
}

// ── Bridge 3: Config → Analytics (flag usage) ─────────────────────────────

/// Feature flag evaluation event for ALICE-Analytics.
///
/// Emitted every time a flag is evaluated so the analytics layer can compute
/// rollout coverage, A/B split accuracy, and flag evaluation latency.
pub struct ConfigAnalyticsFlagEvent {
    /// FNV-1a hash of the flag key — analytics stream key.
    pub content_hash: u64,
    /// FNV-1a hash of the user ID.
    pub user_hash: u64,
    /// True when the flag evaluated to enabled for this user.
    pub enabled: bool,
    /// Rollout rate in permille (0-1000).
    pub rollout_permille: u32,
    /// Evaluation timestamp in milliseconds.
    pub evaluated_at_ms: u64,
}

/// Build a feature flag evaluation event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn config_to_analytics_flag_event(
    flag: &FeatureFlag,
    user_id: &str,
    evaluated_at_ms: u64,
) -> ConfigAnalyticsFlagEvent {
    let content_hash = fnv1a(flag.key.as_bytes());
    let user_hash = fnv1a(user_id.as_bytes());
    let enabled = flag.enabled && flag_enabled(user_id, &flag.key, flag.rollout_rate);
    let rollout_permille = (flag.rollout_rate.clamp(0.0, 1.0) * 1000.0) as u32;
    ConfigAnalyticsFlagEvent {
        content_hash,
        user_hash,
        enabled,
        rollout_permille,
        evaluated_at_ms,
    }
}

// ── Bridge 4: Config → Auth (feature gating) ─────────────────────────────

/// Feature gate check result for ALICE-Auth.
///
/// Auth middleware queries this to decide whether a user's request should be
/// granted access to a gated feature before processing the authorization claim.
pub struct ConfigAuthGate {
    /// FNV-1a hash of the flag key — auth gate identifier.
    pub content_hash: u64,
    /// FNV-1a hash of the user ID — auth subject.
    pub user_hash: u64,
    /// True when the gate is open (feature enabled for this user).
    pub gate_open: bool,
    /// HTTP status code hint: 200 when open, 403 when closed.
    pub status_hint: u16,
    /// Flag rollout permille.
    pub rollout_permille: u32,
}

/// Build a feature gate check result for ALICE-Auth.
#[inline]
#[must_use]
pub fn config_to_auth_gate(flag: &FeatureFlag, user_id: &str) -> ConfigAuthGate {
    let content_hash = fnv1a(flag.key.as_bytes());
    let user_hash = fnv1a(user_id.as_bytes());
    let gate_open = flag.enabled && flag_enabled(user_id, &flag.key, flag.rollout_rate);
    // ブランチレス: gate_open → 200、closed → 403
    let is_open = gate_open as u16;
    let status_hint = 403 - is_open * 203; // 403 - 203 = 200
    let rollout_permille = (flag.rollout_rate.clamp(0.0, 1.0) * 1000.0) as u32;
    ConfigAuthGate {
        content_hash,
        user_hash,
        gate_open,
        status_hint,
        rollout_permille,
    }
}

// ── Bridge 5: Config → Edge (remote config push) ─────────────────────────

/// Remote config push payload for ALICE-Edge.
///
/// Delivers a subset of the configuration to edge nodes so that they can
/// update their local config without polling the central store.
pub struct ConfigEdgePush {
    /// FNV-1a hash of the config snapshot — edge version token.
    pub content_hash: u64,
    /// Number of keys included in this push.
    pub key_count: usize,
    /// Total payload size estimate in bytes (sum of key + value lengths).
    pub payload_bytes: usize,
    /// True when this is a partial push (only changed keys).
    pub is_partial: bool,
    /// Push TTL hint for the edge node in seconds.
    pub ttl_secs: u32,
}

/// Build a remote config push payload for ALICE-Edge.
///
/// `changed_only` when `true` signals the edge node that this is a delta push.
#[inline]
#[must_use]
pub fn config_to_edge_push(
    store: &ConfigStore,
    changed_only: bool,
    ttl_secs: u32,
) -> ConfigEdgePush {
    let merged = store.merged();
    let mut hash_data: Vec<u8> = Vec::new();
    let mut payload_bytes = 0usize;
    for (k, v) in &merged {
        hash_data.extend_from_slice(k.as_bytes());
        let vlen = match v {
            ConfigValue::String(s) => s.len(),
            ConfigValue::Int(_) => 8,
            ConfigValue::Float(_) => 8,
            ConfigValue::Bool(_) => 1,
        };
        payload_bytes += k.len() + vlen;
    }
    let content_hash = if hash_data.is_empty() { fnv1a(b"empty") } else { fnv1a(&hash_data) };
    ConfigEdgePush {
        content_hash,
        key_count: merged.len(),
        payload_bytes,
        is_partial: changed_only,
        ttl_secs,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_store() -> ConfigStore {
        let mut store = ConfigStore::new();
        let mut layer = BTreeMap::new();
        layer.insert(String::from("host"), ConfigValue::String(String::from("localhost")));
        layer.insert(String::from("port"), ConfigValue::Int(8080));
        layer.insert(String::from("debug"), ConfigValue::Bool(false));
        store.add_layer("base", layer);
        store
    }

    fn make_flag(rollout: f64) -> FeatureFlag {
        FeatureFlag { key: String::from("dark_mode"), enabled: true, rollout_rate: rollout }
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_snapshot_hash_nonzero() {
        let store = make_store();
        let snap = config_to_db_snapshot(&store, 1_700_000_000_000);
        assert_ne!(snap.content_hash, 0);
    }

    #[test]
    fn test_db_snapshot_fields() {
        let store = make_store();
        let snap = config_to_db_snapshot(&store, 1_700_000_000_000);
        assert_eq!(snap.layer_count, 1);
        assert_eq!(snap.key_count, 3);
        assert!(snap.is_active);
        assert_eq!(snap.created_at_ms, 1_700_000_000_000);
    }

    #[test]
    fn test_db_snapshot_determinism() {
        let store = make_store();
        let s1 = config_to_db_snapshot(&store, 0);
        let s2 = config_to_db_snapshot(&store, 0);
        assert_eq!(s1.content_hash, s2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_hash_nonzero() {
        let store = make_store();
        let entry = config_to_cache_entry(&store, "host");
        assert_ne!(entry.content_hash, 0);
        assert!(entry.found);
    }

    #[test]
    fn test_cache_entry_bool_ttl_short() {
        let store = make_store();
        let entry = config_to_cache_entry(&store, "debug");
        // Bool → TTL = 30
        assert_eq!(entry.ttl_secs, 30);
        assert_eq!(entry.value_type, 3);
    }

    #[test]
    fn test_cache_entry_string_ttl_long() {
        let store = make_store();
        let entry = config_to_cache_entry(&store, "host");
        // String → TTL = 300
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.value_type, 0);
    }

    #[test]
    fn test_cache_entry_missing_key() {
        let store = make_store();
        let entry = config_to_cache_entry(&store, "nonexistent");
        assert!(!entry.found);
        assert_eq!(entry.value_hash, 0);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_flag_event_hash_nonzero() {
        let flag = make_flag(1.0);
        let ev = config_to_analytics_flag_event(&flag, "user42", 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_ne!(ev.user_hash, 0);
    }

    #[test]
    fn test_analytics_flag_event_100pct_enabled() {
        let flag = make_flag(1.0);
        let ev = config_to_analytics_flag_event(&flag, "user42", 0);
        assert!(ev.enabled);
        assert_eq!(ev.rollout_permille, 1000);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_auth_gate_open_status_200() {
        let flag = make_flag(1.0);
        let gate = config_to_auth_gate(&flag, "user1");
        assert!(gate.gate_open);
        assert_eq!(gate.status_hint, 200);
    }

    #[test]
    fn test_auth_gate_closed_status_403() {
        let flag = make_flag(0.0);
        let gate = config_to_auth_gate(&flag, "user1");
        assert!(!gate.gate_open);
        assert_eq!(gate.status_hint, 403);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_push_hash_nonzero() {
        let store = make_store();
        let push = config_to_edge_push(&store, false, 3600);
        assert_ne!(push.content_hash, 0);
        assert_eq!(push.key_count, 3);
        assert!(!push.is_partial);
        assert_eq!(push.ttl_secs, 3600);
    }

    #[test]
    fn test_edge_push_partial_flag() {
        let store = make_store();
        let push = config_to_edge_push(&store, true, 60);
        assert!(push.is_partial);
    }
}
