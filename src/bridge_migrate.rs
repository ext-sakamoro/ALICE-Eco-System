//! Migrate bridges — ALICE-Migrate ↔ DB, Analytics, Cache, Edge, Crypto
//!
//! 5 bridges connecting the migration engine to the ALICE ecosystem.

use alice_migrate::{schema_hash, DriftReport, Migration, MigrationPlan, MigrationState};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Migrate → DB (migration records) ───────────────────────────

/// Migration execution record for ALICE-DB.
///
/// Written immediately after a migration is applied so that the audit trail
/// is durable even if the application crashes before completing the run.
pub struct MigrateDbRecord {
    /// FNV-1a hash of the migration version string — DB row key.
    pub content_hash: u64,
    /// FNV-1a hash of the `up_sql` statement.
    pub sql_hash: u64,
    /// Version string length in bytes.
    pub version_len: usize,
    /// True when a rollback (`down_sql`) is available.
    pub reversible: bool,
    /// Number of dependency versions listed in `depends_on`.
    pub dep_count: usize,
    /// Application timestamp in milliseconds.
    pub applied_at_ms: u64,
}

/// Build a migration execution record for ALICE-DB.
#[inline]
#[must_use]
pub fn migrate_to_db_record(migration: &Migration, applied_at_ms: u64) -> MigrateDbRecord {
    let content_hash = fnv1a(migration.version.as_bytes());
    let sql_hash = fnv1a(migration.up_sql.as_bytes());
    MigrateDbRecord {
        content_hash,
        sql_hash,
        version_len: migration.version.len(),
        reversible: migration.reversible,
        dep_count: migration.depends_on.len(),
        applied_at_ms,
    }
}

// ── Bridge 2: Migrate → Analytics (migration metrics) ────────────────────

/// Migration run metrics event for ALICE-Analytics.
///
/// Emitted after a migration plan is executed so the analytics layer can
/// track migration frequency, pending backlogs, and applied-to-pending ratios.
pub struct MigrateAnalyticsEvent {
    /// FNV-1a hash of the pending-version list — analytics stream key.
    pub content_hash: u64,
    /// Number of pending migrations in the plan.
    pub pending_count: usize,
    /// Number of already-applied migrations.
    pub applied_count: usize,
    /// Total migration count (pending + applied).
    pub total_count: usize,
    /// Applied-to-total ratio in permille.
    pub applied_permille: u32,
    /// True when there are no pending migrations.
    pub is_current: bool,
}

/// Build a migration metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn migrate_to_analytics_event(plan: &MigrationPlan) -> MigrateAnalyticsEvent {
    let pending_bytes: Vec<u8> = plan.pending.iter().flat_map(|v| v.bytes()).collect();
    let content_hash = if pending_bytes.is_empty() {
        fnv1a(b"no-pending")
    } else {
        fnv1a(&pending_bytes)
    };
    let pending_count = plan.pending.len();
    let applied_count = plan.applied.len();
    let total_count = pending_count + applied_count;
    let total_safe = total_count.max(1);
    let applied_permille = (applied_count.min(total_safe) * 1_000 / total_safe) as u32;
    MigrateAnalyticsEvent {
        content_hash,
        pending_count,
        applied_count,
        total_count,
        applied_permille,
        is_current: pending_count == 0,
    }
}

// ── Bridge 3: Migrate → Cache (schema hash cache) ────────────────────────

/// Cached schema hash entry for ALICE-Cache.
///
/// The schema hash is expensive to compute (requires normalizing and sorting
/// all DDL statements). Caching it for the duration of a deployment window
/// avoids repeated DDL scans on every health-check call.
/// TTL is branchlessly set to 0 when drift is detected (cache invalidation)
/// and to 600 seconds when the schema is clean.
pub struct MigrateCacheEntry {
    /// FNV-1a hash of the schema hash value itself — cache key.
    pub content_hash: u64,
    /// Computed schema hash from `alice_migrate::schema_hash`.
    pub schema_hash_val: u64,
    /// True when the cached schema matches the expected schema.
    pub is_clean: bool,
    /// Cache TTL in seconds (branchless: 0 on drift, 600 when clean).
    pub ttl_secs: u32,
    /// Number of DDL statements that contributed to the hash.
    pub ddl_count: usize,
}

/// Build a cached schema hash entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn migrate_to_cache_entry(ddl_statements: &[&str], expected_hash: u64) -> MigrateCacheEntry {
    let schema_hash_val = schema_hash(ddl_statements);
    let content_hash = fnv1a(&schema_hash_val.to_le_bytes());
    let is_clean = schema_hash_val == expected_hash;
    // ブランチレスTTL: clean → 600秒、drift → 0秒
    let clean_flag = is_clean as u32;
    let ttl_secs = clean_flag * 600;
    MigrateCacheEntry {
        content_hash,
        schema_hash_val,
        is_clean,
        ttl_secs,
        ddl_count: ddl_statements.len(),
    }
}

// ── Bridge 4: Migrate → Edge (migration events) ───────────────────────────

/// Migration event payload for ALICE-Edge.
///
/// Notifies edge agents that a schema migration has been applied so that
/// they can refresh their local schema caches and re-validate any
/// cached query plans.
pub struct MigrateEdgeEvent {
    /// FNV-1a hash of the applied migration version — edge routing key.
    pub content_hash: u64,
    /// Applied schema hash after the migration.
    pub schema_hash_val: u64,
    /// True when the migration is reversible.
    pub reversible: bool,
    /// Number of migrations in the current state.
    pub applied_count: usize,
    /// Event timestamp in milliseconds.
    pub event_at_ms: u64,
    /// True when drift was detected before the migration.
    pub had_drift: bool,
}

/// Build a migration event payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn migrate_to_edge_event(
    migration: &Migration,
    state: &MigrationState,
    drift: &DriftReport,
    event_at_ms: u64,
) -> MigrateEdgeEvent {
    let content_hash = fnv1a(migration.version.as_bytes());
    let schema_hash_val = fnv1a(migration.up_sql.as_bytes());
    MigrateEdgeEvent {
        content_hash,
        schema_hash_val,
        reversible: migration.reversible,
        applied_count: state.count(),
        event_at_ms,
        had_drift: drift.has_drift,
    }
}

// ── Bridge 5: Migrate → Crypto (encrypted DDL) ───────────────────────────

/// Encrypted DDL descriptor for ALICE-Crypto.
///
/// Sensitive migrations (e.g. adding encrypted column keys, altering ACL
/// tables) are wrapped with a crypto descriptor so the Crypto layer can
/// apply envelope encryption before the DDL is sent to the database driver.
pub struct MigrateCryptoDescriptor {
    /// FNV-1a hash of the `up_sql` — Crypto envelope key identifier.
    pub content_hash: u64,
    /// FNV-1a hash of the migration version — nonce seed.
    pub version_hash: u64,
    /// SQL byte length.
    pub sql_len: usize,
    /// True when the DDL contains a `down_sql` that also needs encryption.
    pub has_down_sql: bool,
    /// Cipher hint: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
    pub cipher: u8,
    /// Estimated ciphertext size (sql_len + 16 tag bytes).
    pub ciphertext_len: usize,
}

/// Build an encrypted DDL descriptor for ALICE-Crypto.
///
/// `cipher`: 0=AES-256-GCM, 1=ChaCha20-Poly1305.
#[inline]
#[must_use]
pub fn migrate_to_crypto_descriptor(migration: &Migration, cipher: u8) -> MigrateCryptoDescriptor {
    let content_hash = fnv1a(migration.up_sql.as_bytes());
    let version_hash = fnv1a(migration.version.as_bytes());
    let sql_len = migration.up_sql.len();
    MigrateCryptoDescriptor {
        content_hash,
        version_hash,
        sql_len,
        has_down_sql: migration.down_sql.is_some(),
        cipher: cipher.min(1),
        ciphertext_len: sql_len + 16,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_migrate::{detect_drift, plan_migrations};

    fn make_migration() -> Migration {
        Migration::new(
            "001",
            "create users",
            "CREATE TABLE users (id INT PRIMARY KEY)",
        )
        .with_rollback("DROP TABLE users")
    }

    fn make_plan() -> MigrationPlan {
        let migrations = vec![
            Migration::new("001", "first", "CREATE TABLE a (id INT)"),
            Migration::new("002", "second", "CREATE TABLE b (id INT)"),
        ];
        plan_migrations(&migrations, &[String::from("001")]).unwrap()
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_record_hash_nonzero() {
        let m = make_migration();
        let rec = migrate_to_db_record(&m, 1_700_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.sql_hash, 0);
    }

    #[test]
    fn test_db_record_fields() {
        let m = make_migration();
        let rec = migrate_to_db_record(&m, 1_700_000_000_000);
        assert!(rec.reversible);
        assert_eq!(rec.dep_count, 0);
        assert_eq!(rec.applied_at_ms, 1_700_000_000_000);
        assert!(rec.version_len > 0);
    }

    #[test]
    fn test_db_record_determinism() {
        let m = make_migration();
        let r1 = migrate_to_db_record(&m, 0);
        let r2 = migrate_to_db_record(&m, 0);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_event_hash_nonzero() {
        let plan = make_plan();
        let ev = migrate_to_analytics_event(&plan);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_analytics_event_fields() {
        let plan = make_plan();
        let ev = migrate_to_analytics_event(&plan);
        assert_eq!(ev.pending_count, 1);
        assert_eq!(ev.applied_count, 1);
        assert_eq!(ev.total_count, 2);
        assert_eq!(ev.applied_permille, 500);
        assert!(!ev.is_current);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_clean_ttl() {
        let ddl = ["CREATE TABLE users (id INT)"];
        let expected = schema_hash(&ddl);
        let entry = migrate_to_cache_entry(&ddl, expected);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.is_clean);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_cache_entry_drift_ttl_zero() {
        let ddl = ["CREATE TABLE users (id INT)"];
        let entry = migrate_to_cache_entry(&ddl, 0xdeadbeef);
        assert!(!entry.is_clean);
        assert_eq!(entry.ttl_secs, 0);
    }

    #[test]
    fn test_cache_entry_ddl_count() {
        let ddl = ["CREATE TABLE a (id INT)", "CREATE TABLE b (id INT)"];
        let expected = schema_hash(&ddl);
        let entry = migrate_to_cache_entry(&ddl, expected);
        assert_eq!(entry.ddl_count, 2);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_event_hash_nonzero() {
        let m = make_migration();
        let mut state = MigrationState::new();
        state.record_applied("001", 12345, 1000);
        let drift = detect_drift(&["CREATE TABLE a (id INT)"], &["CREATE TABLE a (id INT)"]);
        let ev = migrate_to_edge_event(&m, &state, &drift, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_edge_event_fields() {
        let m = make_migration();
        let mut state = MigrationState::new();
        state.record_applied("001", 111, 1000);
        let drift = detect_drift(&["CREATE TABLE a (id INT)"], &["CREATE TABLE b (id INT)"]);
        let ev = migrate_to_edge_event(&m, &state, &drift, 2_000);
        assert!(ev.reversible);
        assert_eq!(ev.applied_count, 1);
        assert_eq!(ev.event_at_ms, 2_000);
        assert!(ev.had_drift);
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_crypto_descriptor_hash_nonzero() {
        let m = make_migration();
        let desc = migrate_to_crypto_descriptor(&m, 0);
        assert_ne!(desc.content_hash, 0);
        assert_ne!(desc.version_hash, 0);
    }

    #[test]
    fn test_crypto_descriptor_fields() {
        let m = make_migration();
        let desc = migrate_to_crypto_descriptor(&m, 0);
        assert!(desc.has_down_sql);
        assert_eq!(desc.cipher, 0);
        assert_eq!(desc.ciphertext_len, desc.sql_len + 16);
    }

    #[test]
    fn test_crypto_descriptor_cipher_clamped() {
        let m = make_migration();
        let desc = migrate_to_crypto_descriptor(&m, 99);
        assert_eq!(desc.cipher, 1);
    }
}
