//! Factory bridges — ALICE-Factory ↔ DB, Cache, Analytics, Config, Log
//!
//! 5 bridges connecting the ALICEFactory instance builder (Project-ALICE V3)
//! to the ALICE ecosystem.  Covers instance creation logs, preset caching,
//! factory metrics, configuration snapshots, and health monitoring events.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Factory → DB (instance creation record) ────────────────────

/// Instance creation record for ALICE-DB persistence.
///
/// Logs each `ALICEFactory::create()` invocation with the preset used,
/// enabled layers, and creation time.
pub struct FactoryDbCreationRecord {
    /// FNV-1a hash over instance_id — row deduplication key.
    pub content_hash: u64,
    /// Instance identifier (FNV-1a hash of creation context).
    pub instance_id: u64,
    /// Preset code: 0 = Minimal, 1 = Standard, 2 = Security, 3 = AgiOlympic,
    /// 4 = Embodied, 5 = Research, 6 = Innovation.
    pub preset_code: u8,
    /// Enabled layers bitfield: bit0 = cognitive, bit1 = autonomy,
    /// bit2 = consciousness, bit3 = swarm, bit4 = innovation.
    pub enabled_layers: u8,
    /// Creation timestamp in nanoseconds.
    pub created_ns: u64,
    /// Creation time in microseconds.
    pub creation_time_us: u64,
}

/// Build a `FactoryDbCreationRecord`.
#[inline]
#[must_use]
pub fn factory_to_db_creation_record(
    instance_id: u64,
    preset_code: u8,
    enabled_layers: u8,
    created_ns: u64,
    creation_time_us: u64,
) -> FactoryDbCreationRecord {
    let content_hash = fnv1a(&instance_id.to_le_bytes());
    FactoryDbCreationRecord {
        content_hash,
        instance_id,
        preset_code,
        enabled_layers,
        created_ns,
        creation_time_us,
    }
}

// ── Bridge 2: Factory → Cache (preset config cache) ──────────────────────

/// Preset configuration cache for ALICE-Cache.
///
/// Caches the expanded preset configuration so repeated factory
/// invocations with the same preset avoid re-computation.
pub struct FactoryCachePresetConfig {
    /// FNV-1a hash over preset_code — cache lookup key.
    pub content_hash: u64,
    /// Preset code (0–6).
    pub preset_code: u8,
    /// Enabled layers bitfield.
    pub enabled_layers: u8,
    /// Max reasoning depth.
    pub max_reasoning_depth: u32,
    /// Max memory entries.
    pub max_memory_entries: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build a `FactoryCachePresetConfig` entry.
///
/// TTL: 3600 s (preset configs are stable).
#[inline]
#[must_use]
pub fn factory_to_cache_preset_config(
    preset_code: u8,
    enabled_layers: u8,
    max_reasoning_depth: u32,
    max_memory_entries: u32,
) -> FactoryCachePresetConfig {
    let content_hash = fnv1a(&[preset_code]);
    FactoryCachePresetConfig {
        content_hash,
        preset_code,
        enabled_layers,
        max_reasoning_depth,
        max_memory_entries,
        ttl_secs: 3600,
    }
}

// ── Bridge 3: Factory → Analytics (factory metrics) ──────────────────────

/// Factory usage metrics for ALICE-Analytics.
///
/// Tracks instance creation patterns: preset distribution, creation
/// latency, and layer activation rates.
pub struct FactoryAnalyticsMetrics {
    /// FNV-1a hash over factory_id + tick — deduplication key.
    pub content_hash: u64,
    /// Factory identifier.
    pub factory_id: u64,
    /// Metric tick.
    pub tick: u64,
    /// Instances created in this interval.
    pub instances_created: u32,
    /// Mean creation time in microseconds.
    pub mean_creation_time_us: u32,
    /// Number of instances with autonomy enabled.
    pub autonomy_enabled_count: u32,
    /// Number of instances with consciousness enabled.
    pub consciousness_enabled_count: u32,
    /// Most common preset code in this interval.
    pub dominant_preset: u8,
}

/// Build a `FactoryAnalyticsMetrics` event.
#[inline]
#[must_use]
pub fn factory_to_analytics_metrics(
    factory_id: u64,
    tick: u64,
    instances_created: u32,
    mean_creation_time_us: u32,
    autonomy_enabled_count: u32,
    consciousness_enabled_count: u32,
    dominant_preset: u8,
) -> FactoryAnalyticsMetrics {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&factory_id.to_le_bytes());
    buf[8..16].copy_from_slice(&tick.to_le_bytes());
    let content_hash = fnv1a(&buf);
    FactoryAnalyticsMetrics {
        content_hash,
        factory_id,
        tick,
        instances_created,
        mean_creation_time_us,
        autonomy_enabled_count,
        consciousness_enabled_count,
        dominant_preset,
    }
}

// ── Bridge 4: Factory → Config (instance config snapshot) ────────────────

/// Instance configuration snapshot for ALICE-Config.
///
/// Stores the complete configuration state of a created instance for
/// reproduction and debugging.
pub struct FactoryConfigSnapshot {
    /// FNV-1a hash over instance_id — config key.
    pub content_hash: u64,
    /// Instance identifier.
    pub instance_id: u64,
    /// Preset code.
    pub preset_code: u8,
    /// LLM provider type: 0 = CoreReasoning, 1 = GGUF, 2 = Custom.
    pub llm_provider_type: u8,
    /// Max reasoning depth.
    pub max_reasoning_depth: u32,
    /// Max memory entries.
    pub max_memory_entries: u32,
    /// Schema version for config compatibility.
    pub schema_version: u32,
}

/// Build a `FactoryConfigSnapshot`.
#[inline]
#[must_use]
pub fn factory_to_config_snapshot(
    instance_id: u64,
    preset_code: u8,
    llm_provider_type: u8,
    max_reasoning_depth: u32,
    max_memory_entries: u32,
) -> FactoryConfigSnapshot {
    let content_hash = fnv1a(&instance_id.to_le_bytes());
    FactoryConfigSnapshot {
        content_hash,
        instance_id,
        preset_code,
        llm_provider_type,
        max_reasoning_depth,
        max_memory_entries,
        schema_version: 1,
    }
}

// ── Bridge 5: Factory → Log (health check event) ─────────────────────────

/// Instance health check event for ALICE-Log.
///
/// Periodic health probe result for each active ALICE instance.
pub struct FactoryLogHealthEvent {
    /// FNV-1a hash over instance_id + timestamp_ns — log key.
    pub content_hash: u64,
    /// Instance identifier.
    pub instance_id: u64,
    /// Check timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Overall health status: 0 = Healthy, 1 = Degraded, 2 = Unhealthy.
    pub health_status: u8,
    /// Number of active components.
    pub active_components: u32,
    /// Number of degraded components.
    pub degraded_components: u32,
}

/// Build a `FactoryLogHealthEvent`.
#[inline]
#[must_use]
pub fn factory_to_log_health_event(
    instance_id: u64,
    timestamp_ns: u64,
    health_status: u8,
    active_components: u32,
    degraded_components: u32,
) -> FactoryLogHealthEvent {
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&instance_id.to_le_bytes());
    buf[8..16].copy_from_slice(&timestamp_ns.to_le_bytes());
    let content_hash = fnv1a(&buf);
    FactoryLogHealthEvent {
        content_hash,
        instance_id,
        timestamp_ns,
        health_status,
        active_components,
        degraded_components,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_db_creation_hash_nonzero() {
        let rec = factory_to_db_creation_record(1, 0, 0b00001, 1_000_000, 500);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_factory_db_creation_deterministic() {
        let a = factory_to_db_creation_record(1, 0, 0b00001, 1_000_000, 500);
        let b = factory_to_db_creation_record(1, 0, 0b00001, 1_000_000, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_factory_cache_preset_ttl() {
        let entry = factory_to_cache_preset_config(3, 0b11111, 10, 1000);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn test_factory_analytics_fields() {
        let m = factory_to_analytics_metrics(1, 10, 5, 200, 3, 2, 1);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.instances_created, 5);
        assert_eq!(m.dominant_preset, 1);
    }

    #[test]
    fn test_factory_config_schema_version() {
        let snap = factory_to_config_snapshot(42, 2, 0, 8, 500);
        assert_eq!(snap.schema_version, 1);
        assert_eq!(snap.preset_code, 2);
    }

    #[test]
    fn test_factory_log_health_fields() {
        let ev = factory_to_log_health_event(1, 999_999, 0, 5, 0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.health_status, 0);
        assert_eq!(ev.active_components, 5);
    }

    #[test]
    fn test_factory_different_instances_differ() {
        let a = factory_to_db_creation_record(1, 0, 0b00001, 1_000_000, 500);
        let b = factory_to_db_creation_record(2, 0, 0b00001, 1_000_000, 500);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_factory_cache_different_presets_differ() {
        let a = factory_to_cache_preset_config(0, 0b00001, 5, 100);
        let b = factory_to_cache_preset_config(3, 0b11111, 10, 1000);
        assert_ne!(a.content_hash, b.content_hash);
    }
}
