//! DB bridges — ALICE-DB as data hub ↔ Analytics, Cache, Queue
//!
//! 7 bridges using ALICE-DB as the central persistence layer.
//! Covers DB-backed analytics storage, cache-assisted query planning,
//! and queue-based async write pipelines.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: DB → Analytics (query execution telemetry) ─────────────────

/// Query execution telemetry for ALICE-Analytics.
///
/// Emitted after every DB query so that the analytics pipeline can build
/// latency histograms and flag slow queries for optimization.
pub struct DbAnalyticsQueryEvent {
    /// FNV-1a hash of the query string — analytics stream key.
    pub query_hash: u64,
    /// Query execution time in milliseconds.
    pub exec_ms: f64,
    /// Number of rows scanned.
    pub rows_scanned: u64,
    /// Number of rows returned.
    pub rows_returned: u64,
    /// Whether the query was served from the index (true) or a full scan (false).
    pub index_hit: bool,
    /// Estimated query cost in abstract planner units.
    pub planner_cost: f64,
}

/// Build a query telemetry event for ALICE-Analytics.
#[inline]
pub fn db_to_analytics_query_event(
    query: &str,
    exec_ms: f64,
    rows_scanned: u64,
    rows_returned: u64,
    index_hit: bool,
    planner_cost: f64,
) -> DbAnalyticsQueryEvent {
    DbAnalyticsQueryEvent {
        query_hash: fnv1a(query.as_bytes()),
        exec_ms,
        rows_scanned,
        rows_returned,
        index_hit,
        planner_cost,
    }
}

// ── Bridge 2: DB → Cache (query result caching) ───────────────────────────

/// Query result cache entry for ALICE-Cache.
///
/// Stores the output of a cacheable read query keyed by FNV-1a of the
/// canonicalized query string.  TTL is inversely proportional to result
/// volatility, approximated by `rows_returned`.
pub struct DbCacheQueryResult {
    /// FNV-1a hash of the query string — cache key.
    pub query_hash: u64,
    /// Serialized result size in bytes.
    pub result_bytes: usize,
    /// Number of rows in the result set.
    pub row_count: u64,
    /// Derived TTL in seconds (branchless: smaller result sets cached longer).
    pub ttl_secs: u32,
    /// Schema version tag for cache invalidation on migration.
    pub schema_version: u16,
}

/// Build a query result cache entry for ALICE-Cache.
///
/// TTL derivation (branchless):
/// - `row_count` == 0 → 3600 s (empty result, stable)
/// - `row_count` <= 100 → 300 s
/// - `row_count` <= 10 000 → 60 s
/// - larger → 10 s (high-cardinality result, likely to change soon)
#[inline]
pub fn db_to_cache_query_result(
    query: &str,
    result_bytes: usize,
    row_count: u64,
    schema_version: u16,
) -> DbCacheQueryResult {
    let query_hash = fnv1a(query.as_bytes());
    // Branchless TTL via a 4-entry lookup indexed by cardinality tier.
    // Tier = number of bits needed to represent row_count / 7 (log₂ proxy), clamped to 3.
    let tier = if row_count == 0 { 0usize }
               else if row_count <= 100 { 1 }
               else if row_count <= 10_000 { 2 }
               else { 3 };
    const TTL_TABLE: [u32; 4] = [3_600, 300, 60, 10];
    let ttl_secs = TTL_TABLE[tier];
    DbCacheQueryResult {
        query_hash,
        result_bytes,
        row_count,
        ttl_secs,
        schema_version,
    }
}

// ── Bridge 3: DB → Queue (async write pipeline) ──────────────────────────

/// Async write request for ALICE-Queue.
///
/// Heavy insert/update operations are forwarded to a write queue so the
/// DB layer can acknowledge the request immediately and apply the write
/// asynchronously without blocking the caller.
pub struct DbQueueWriteRequest {
    /// FNV-1a hash of the table name — queue routing key.
    pub table_hash: u64,
    /// Byte length of the serialized row payload.
    pub payload_bytes: usize,
    /// Write operation: 0=insert, 1=update, 2=upsert, 3=delete.
    pub operation: u8,
    /// Scheduling priority (0=low, 1=normal, 2=high).
    pub priority: u8,
    /// Enqueue timestamp in milliseconds.
    pub enqueue_ms: u64,
}

/// Build an async write request for ALICE-Queue.
///
/// Priority derivation (branchless):
/// - delete (3) → 2 (high: must propagate quickly for consistency)
/// - upsert (2) → 1 (normal)
/// - insert (0) / update (1) → 0 (low: can be batched)
#[inline]
pub fn db_to_queue_write_request(
    table: &str,
    payload_bytes: usize,
    operation: u8,
    enqueue_ms: u64,
) -> DbQueueWriteRequest {
    let table_hash = fnv1a(table.as_bytes());
    let is_delete = (operation == 3) as u8;
    let is_upsert = (operation == 2) as u8;
    let priority = is_delete.wrapping_mul(2) | (is_upsert & !is_delete);
    DbQueueWriteRequest {
        table_hash,
        payload_bytes,
        operation,
        priority,
        enqueue_ms,
    }
}

// ── Bridge 4: DB → Analytics (schema change event) ────────────────────────

/// Schema change event for ALICE-Analytics audit trail.
///
/// Records DDL operations (CREATE/ALTER/DROP) so that the analytics
/// pipeline can correlate schema migrations with query performance regressions.
pub struct DbAnalyticsSchemaEvent {
    /// FNV-1a hash of `table_name` — stream key.
    pub table_hash: u64,
    /// DDL operation: 0=create, 1=alter_add, 2=alter_drop, 3=drop, 4=index_create.
    pub ddl_op: u8,
    /// New schema version after this change.
    pub schema_version: u16,
    /// Estimated migration duration in milliseconds.
    pub migration_ms: f64,
    /// Number of rows affected by the migration (0 for CREATE/DROP).
    pub rows_affected: u64,
}

/// Build a schema change event for ALICE-Analytics.
#[inline]
pub fn db_to_analytics_schema_event(
    table_name: &str,
    ddl_op: u8,
    schema_version: u16,
    migration_ms: f64,
    rows_affected: u64,
) -> DbAnalyticsSchemaEvent {
    DbAnalyticsSchemaEvent {
        table_hash: fnv1a(table_name.as_bytes()),
        ddl_op,
        schema_version,
        migration_ms,
        rows_affected,
    }
}

// ── Bridge 5: DB → Cache (table stats for query planning) ─────────────────

/// Table statistics cache entry for ALICE-Cache.
///
/// Pre-computed planner statistics (row count, index cardinality, page count)
/// stored in ALICE-Cache so the query planner can skip expensive ANALYZE scans.
pub struct DbCacheTableStats {
    /// FNV-1a hash of the table name — cache key.
    pub table_hash: u64,
    /// Approximate total row count.
    pub row_count: u64,
    /// Number of data pages (each ~8 KiB).
    pub page_count: u32,
    /// Number of indexes on the table.
    pub index_count: u8,
    /// Mean row width in bytes.
    pub mean_row_bytes: u32,
    /// Cache TTL in seconds (statistics valid until next ANALYZE).
    pub ttl_secs: u32,
}

/// Build a table statistics cache entry for ALICE-Cache.
///
/// TTL is set to 1800 s (30 minutes) — a typical ANALYZE interval.
#[inline]
pub fn db_to_cache_table_stats(
    table_name: &str,
    row_count: u64,
    page_count: u32,
    index_count: u8,
    mean_row_bytes: u32,
) -> DbCacheTableStats {
    DbCacheTableStats {
        table_hash: fnv1a(table_name.as_bytes()),
        row_count,
        page_count,
        index_count,
        mean_row_bytes,
        ttl_secs: 1_800,
    }
}

// ── Bridge 6: DB → Queue (replication event) ─────────────────────────────

/// Row replication event for ALICE-Queue.
///
/// Publishes WAL-derived change data capture (CDC) events so that downstream
/// consumers (search indexers, analytics, replicas) can stay in sync without
/// polling the primary DB.
pub struct DbQueueReplicationEvent {
    /// FNV-1a hash of `table_name || lsn` — deduplication key.
    pub event_hash: u64,
    /// FNV-1a hash of the table name.
    pub table_hash: u64,
    /// Change type: 0=insert, 1=update, 2=delete.
    pub change_type: u8,
    /// Log sequence number (monotonically increasing WAL offset).
    pub lsn: u64,
    /// Serialized old+new row delta size in bytes.
    pub delta_bytes: usize,
}

/// Build a CDC replication event for ALICE-Queue.
#[inline]
pub fn db_to_queue_replication_event(
    table_name: &str,
    change_type: u8,
    lsn: u64,
    delta_bytes: usize,
) -> DbQueueReplicationEvent {
    let table_hash = fnv1a(table_name.as_bytes());
    // Combine table hash and LSN for a unique event dedup key.
    let mut data = [0u8; 16];
    data[0..8].copy_from_slice(&table_hash.to_le_bytes());
    data[8..16].copy_from_slice(&lsn.to_le_bytes());
    let event_hash = fnv1a(&data);
    DbQueueReplicationEvent {
        event_hash,
        table_hash,
        change_type,
        lsn,
        delta_bytes,
    }
}

// ── Bridge 7: DB → Analytics (connection pool metrics) ────────────────────

/// Connection pool metrics for ALICE-Analytics.
///
/// Emitted periodically by the DB connection pool manager so that the
/// analytics layer can detect pool exhaustion and saturation events.
pub struct DbAnalyticsPoolMetrics {
    /// FNV-1a hash of the pool name — analytics stream key.
    pub pool_hash: u64,
    /// Total connections in the pool.
    pub pool_size: u32,
    /// Currently active (checked-out) connections.
    pub active_connections: u32,
    /// Connections waiting for an available slot.
    pub wait_count: u32,
    /// Pool utilisation in permille (active * 1000 / pool_size).
    pub utilisation_permille: u32,
    /// Mean connection wait time in milliseconds.
    pub mean_wait_ms: f64,
}

/// Build connection pool metrics for ALICE-Analytics.
///
/// `utilisation_permille` is computed branchlessly (guard pool_size=0 with max(1)).
#[inline]
pub fn db_to_analytics_pool_metrics(
    pool_name: &str,
    pool_size: u32,
    active_connections: u32,
    wait_count: u32,
    mean_wait_ms: f64,
) -> DbAnalyticsPoolMetrics {
    let pool_hash = fnv1a(pool_name.as_bytes());
    let size_safe = pool_size.max(1);
    let active_clamped = active_connections.min(pool_size);
    let utilisation_permille = active_clamped.wrapping_mul(1_000) / size_safe;
    DbAnalyticsPoolMetrics {
        pool_hash,
        pool_size,
        active_connections,
        wait_count,
        utilisation_permille,
        mean_wait_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_to_analytics_query_event() {
        let ev = db_to_analytics_query_event("SELECT * FROM users WHERE id = ?", 1.5, 1, 1, true, 0.02);
        assert_ne!(ev.query_hash, 0);
        assert!((ev.exec_ms - 1.5).abs() < f64::EPSILON);
        assert_eq!(ev.rows_scanned, 1);
        assert!(ev.index_hit);
        assert!((ev.planner_cost - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn test_db_to_cache_query_result_ttl_tiers() {
        // Empty result → 3600 s.
        let r0 = db_to_cache_query_result("SELECT 1", 0, 0, 1);
        assert_eq!(r0.ttl_secs, 3_600);

        // 50 rows → 300 s.
        let r1 = db_to_cache_query_result("SELECT * FROM small", 400, 50, 1);
        assert_eq!(r1.ttl_secs, 300);

        // 5 000 rows → 60 s.
        let r2 = db_to_cache_query_result("SELECT * FROM medium", 40_000, 5_000, 1);
        assert_eq!(r2.ttl_secs, 60);

        // 1 000 000 rows → 10 s.
        let r3 = db_to_cache_query_result("SELECT * FROM large", 8_000_000, 1_000_000, 1);
        assert_eq!(r3.ttl_secs, 10);
    }

    #[test]
    fn test_db_to_queue_write_request_priority() {
        // Insert → priority 0.
        let ins = db_to_queue_write_request("orders", 256, 0, 1_000);
        assert_eq!(ins.priority, 0);

        // Upsert → priority 1.
        let ups = db_to_queue_write_request("sessions", 128, 2, 1_001);
        assert_eq!(ups.priority, 1);

        // Delete → priority 2.
        let del = db_to_queue_write_request("tokens", 64, 3, 1_002);
        assert_eq!(del.priority, 2);

        assert_ne!(ins.table_hash, 0);
    }

    #[test]
    fn test_db_to_analytics_schema_event() {
        let ev = db_to_analytics_schema_event("accounts", 0, 1, 0.0, 0);
        assert_ne!(ev.table_hash, 0);
        assert_eq!(ev.ddl_op, 0);
        assert_eq!(ev.schema_version, 1);
        assert_eq!(ev.rows_affected, 0);
    }

    #[test]
    fn test_db_to_cache_table_stats() {
        let stats = db_to_cache_table_stats("products", 1_000_000, 125_000, 3, 128);
        assert_ne!(stats.table_hash, 0);
        assert_eq!(stats.row_count, 1_000_000);
        assert_eq!(stats.page_count, 125_000);
        assert_eq!(stats.index_count, 3);
        assert_eq!(stats.mean_row_bytes, 128);
        assert_eq!(stats.ttl_secs, 1_800);
    }

    #[test]
    fn test_db_to_queue_replication_event() {
        let ev = db_to_queue_replication_event("orders", 0, 42_000, 512);
        assert_ne!(ev.event_hash, 0);
        assert_ne!(ev.table_hash, 0);
        assert_eq!(ev.change_type, 0);
        assert_eq!(ev.lsn, 42_000);
        assert_eq!(ev.delta_bytes, 512);

        // Same table, different LSN → different event hash.
        let ev2 = db_to_queue_replication_event("orders", 0, 42_001, 512);
        assert_ne!(ev.event_hash, ev2.event_hash);
    }

    #[test]
    fn test_db_to_analytics_pool_metrics_utilisation() {
        let m = db_to_analytics_pool_metrics("primary", 100, 75, 5, 2.3);
        assert_ne!(m.pool_hash, 0);
        assert_eq!(m.pool_size, 100);
        assert_eq!(m.active_connections, 75);
        // 75 * 1000 / 100 = 750 permille.
        assert_eq!(m.utilisation_permille, 750);
        assert_eq!(m.wait_count, 5);
    }

    #[test]
    fn test_db_to_analytics_pool_metrics_zero_size_no_panic() {
        // Pool size 0 must not divide by zero.
        let m = db_to_analytics_pool_metrics("empty-pool", 0, 0, 0, 0.0);
        assert_eq!(m.pool_size, 0);
        assert_eq!(m.utilisation_permille, 0);
    }
}
