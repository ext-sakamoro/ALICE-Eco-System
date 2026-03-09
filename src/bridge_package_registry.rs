//! PackageRegistry bridges — PackageRegistry ↔ DB, Cache, Analytics, CDN, API
//!
//! 5 bridges connecting package registry data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: PackageRegistry → DB (registry record persistence) ─────────

/// Package registry record for ALICE-DB persistence.
pub struct PackageRegistryDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Total number of packages in the registry.
    pub package_count: u64,
    /// Total number of package versions.
    pub version_count: u64,
    /// Total storage size in bytes.
    pub total_bytes: u64,
    /// Registry identifier hash.
    pub registry_hash: u64,
    /// Total download count.
    pub download_count: u64,
}

/// Serialize package registry data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn package_registry_to_db_record(
    package_count: u64,
    version_count: u64,
    total_bytes: u64,
    registry_hash: u64,
    download_count: u64,
) -> PackageRegistryDbRecord {
    // buf: package_count(8) + version_count(8) + total_bytes(8) + registry_hash(8) + download_count(8) = 40
    let mut buf = [0u8; 40];
    buf[0..8].copy_from_slice(&package_count.to_le_bytes());
    buf[8..16].copy_from_slice(&version_count.to_le_bytes());
    buf[16..24].copy_from_slice(&total_bytes.to_le_bytes());
    buf[24..32].copy_from_slice(&registry_hash.to_le_bytes());
    buf[32..40].copy_from_slice(&download_count.to_le_bytes());
    PackageRegistryDbRecord {
        content_hash: fnv1a(&buf),
        package_count,
        version_count,
        total_bytes,
        registry_hash,
        download_count,
    }
}

// ── Bridge 2: PackageRegistry → Cache (package cache entry) ──────────────

/// Package cache entry for ALICE-Cache.
pub struct PackageRegistryCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Package identifier hash.
    pub package_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Number of available versions.
    pub version_count: u32,
    /// Tarball size in bytes.
    pub tarball_bytes: u64,
}

/// Build package cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn package_registry_to_cache_entry(
    package_hash: u64,
    ttl_secs: u32,
    version_count: u32,
    tarball_bytes: u64,
) -> PackageRegistryCacheEntry {
    // buf: package_hash(8) + version_count(4) + tarball_bytes(8) = 20
    let mut buf = [0u8; 20];
    buf[0..8].copy_from_slice(&package_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&version_count.to_le_bytes());
    buf[12..20].copy_from_slice(&tarball_bytes.to_le_bytes());
    PackageRegistryCacheEntry {
        content_hash: fnv1a(&buf),
        package_hash,
        ttl_secs,
        version_count,
        tarball_bytes,
    }
}

// ── Bridge 3: PackageRegistry → Analytics (download analytics event) ─────

/// Package registry analytics event for ALICE-Analytics ingestion.
pub struct PackageRegistryAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Total download count.
    pub download_count: u64,
    /// Total publish count.
    pub publish_count: u32,
    /// Number of unique packages accessed.
    pub unique_packages: u64,
    /// Total bandwidth consumed in bytes.
    pub bandwidth_bytes: u64,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build package registry analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn package_registry_to_analytics_event(
    download_count: u64,
    publish_count: u32,
    unique_packages: u64,
    bandwidth_bytes: u64,
    timestamp_ms: u64,
) -> PackageRegistryAnalyticsEvent {
    // buf: download_count(8) + publish_count(4) + unique_packages(8) + bandwidth_bytes(8) + timestamp_ms(8) = 36
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&download_count.to_le_bytes());
    buf[8..12].copy_from_slice(&publish_count.to_le_bytes());
    buf[12..20].copy_from_slice(&unique_packages.to_le_bytes());
    buf[20..28].copy_from_slice(&bandwidth_bytes.to_le_bytes());
    buf[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    PackageRegistryAnalyticsEvent {
        content_hash: fnv1a(&buf),
        download_count,
        publish_count,
        unique_packages,
        bandwidth_bytes,
        timestamp_ms,
    }
}

// ── Bridge 4: PackageRegistry → CDN (tarball delivery descriptor) ─────────

/// Package tarball CDN delivery descriptor for ALICE-CDN.
pub struct PackageRegistryCdnDelivery {
    /// Content hash.
    pub content_hash: u64,
    /// Package identifier hash.
    pub package_hash: u64,
    /// Tarball size in bytes.
    pub tarball_bytes: u64,
    /// Edge cache TTL in seconds.
    pub edge_ttl_secs: u32,
    /// Version identifier hash.
    pub version_hash: u64,
}

/// Build package tarball CDN delivery descriptor for ALICE-CDN.
#[inline]
#[must_use]
pub fn package_registry_to_cdn_delivery(
    package_hash: u64,
    tarball_bytes: u64,
    edge_ttl_secs: u32,
    version_hash: u64,
) -> PackageRegistryCdnDelivery {
    // buf: package_hash(8) + tarball_bytes(8) + edge_ttl_secs(4) + version_hash(8) = 28
    let mut buf = [0u8; 28];
    buf[0..8].copy_from_slice(&package_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tarball_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&edge_ttl_secs.to_le_bytes());
    buf[20..28].copy_from_slice(&version_hash.to_le_bytes());
    PackageRegistryCdnDelivery {
        content_hash: fnv1a(&buf),
        package_hash,
        tarball_bytes,
        edge_ttl_secs,
        version_hash,
    }
}

// ── Bridge 5: PackageRegistry → API (registry summary payload) ───────────

/// Package registry summary API payload for ALICE-API responses.
pub struct PackageRegistryApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Total package count.
    pub package_count: u64,
    /// Total storage size in bytes.
    pub total_bytes: u64,
    /// Total download count.
    pub download_count: u64,
    /// API schema version.
    pub schema_version: u16,
}

/// Build package registry summary API payload for ALICE-API.
#[inline]
#[must_use]
pub fn package_registry_to_api_payload(
    package_count: u64,
    total_bytes: u64,
    download_count: u64,
    schema_version: u16,
) -> PackageRegistryApiPayload {
    // buf: package_count(8) + total_bytes(8) + download_count(8) + schema_version(2) = 26
    let mut buf = [0u8; 26];
    buf[0..8].copy_from_slice(&package_count.to_le_bytes());
    buf[8..16].copy_from_slice(&total_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&download_count.to_le_bytes());
    buf[24..26].copy_from_slice(&schema_version.to_le_bytes());
    PackageRegistryApiPayload {
        content_hash: fnv1a(&buf),
        package_count,
        total_bytes,
        download_count,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_registry_to_db_record_hash_nonzero() {
        let rec =
            package_registry_to_db_record(5_000, 25_000, 10_737_418_240, 0xabcd_1234, 1_000_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_package_registry_to_db_record_fields() {
        let rec = package_registry_to_db_record(100, 500, 1_073_741_824, 0x9999, 10_000);
        assert_eq!(rec.package_count, 100);
        assert_eq!(rec.version_count, 500);
        assert_eq!(rec.total_bytes, 1_073_741_824);
        assert_eq!(rec.registry_hash, 0x9999);
        assert_eq!(rec.download_count, 10_000);
    }

    #[test]
    fn test_package_registry_to_db_record_determinism() {
        let a = package_registry_to_db_record(1, 2, 3, 4, 5);
        let b = package_registry_to_db_record(1, 2, 3, 4, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_package_registry_to_cache_entry_hash_nonzero() {
        let entry = package_registry_to_cache_entry(0xdead_beef, 86_400, 10, 2_097_152);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_package_registry_to_cache_entry_fields() {
        let entry = package_registry_to_cache_entry(0x1234, 3_600, 3, 524_288);
        assert_eq!(entry.package_hash, 0x1234);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_eq!(entry.version_count, 3);
        assert_eq!(entry.tarball_bytes, 524_288);
    }

    #[test]
    fn test_package_registry_to_analytics_event_hash_nonzero() {
        let ev = package_registry_to_analytics_event(
            50_000,
            200,
            1_500,
            5_368_709_120,
            1_700_000_000_000,
        );
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_package_registry_to_analytics_event_fields() {
        let ev = package_registry_to_analytics_event(1_000, 5, 100, 104_857_600, 77_777);
        assert_eq!(ev.download_count, 1_000);
        assert_eq!(ev.publish_count, 5);
        assert_eq!(ev.unique_packages, 100);
        assert_eq!(ev.bandwidth_bytes, 104_857_600);
        assert_eq!(ev.timestamp_ms, 77_777);
    }

    #[test]
    fn test_package_registry_to_cdn_delivery_hash_nonzero() {
        let delivery =
            package_registry_to_cdn_delivery(0xcafe_babe, 4_194_304, 604_800, 0x1234_5678);
        assert_ne!(delivery.content_hash, 0);
    }

    #[test]
    fn test_package_registry_to_cdn_delivery_fields() {
        let delivery = package_registry_to_cdn_delivery(0x5555, 1_048_576, 3_600, 0xaaaa);
        assert_eq!(delivery.package_hash, 0x5555);
        assert_eq!(delivery.tarball_bytes, 1_048_576);
        assert_eq!(delivery.edge_ttl_secs, 3_600);
        assert_eq!(delivery.version_hash, 0xaaaa);
    }

    #[test]
    fn test_package_registry_to_api_payload_hash_nonzero() {
        let payload = package_registry_to_api_payload(2_000, 21_474_836_480, 500_000, 1);
        assert_ne!(payload.content_hash, 0);
    }

    #[test]
    fn test_package_registry_to_api_payload_fields() {
        let payload = package_registry_to_api_payload(50, 1_073_741_824, 999, 2);
        assert_eq!(payload.package_count, 50);
        assert_eq!(payload.total_bytes, 1_073_741_824);
        assert_eq!(payload.download_count, 999);
        assert_eq!(payload.schema_version, 2);
    }

    #[test]
    fn test_package_registry_to_api_payload_determinism() {
        let a = package_registry_to_api_payload(10, 20, 30, 1);
        let b = package_registry_to_api_payload(10, 20, 30, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
