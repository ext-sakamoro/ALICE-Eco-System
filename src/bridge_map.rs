//! Map bridges — Map ↔ DB, Cache, Analytics, Render, CDN
//!
//! 5 bridges connecting map tile data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Map → DB (map record persistence) ───────────────────────────

/// Map record for ALICE-DB persistence.
pub struct MapDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Total number of tiles in the map.
    pub tile_count: u64,
    /// Number of rendering layers.
    pub layer_count: u16,
    /// Maximum zoom level supported.
    pub zoom_max: u8,
    /// Bounding box identifier hash.
    pub bounds_hash: u64,
    /// Style definition hash.
    pub style_hash: u64,
}

/// Serialize map data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn map_to_db_record(
    tile_count: u64,
    layer_count: u16,
    zoom_max: u8,
    bounds_hash: u64,
    style_hash: u64,
) -> MapDbRecord {
    // buf: tile_count(8) + layer_count(2) + zoom_max(1) + bounds_hash(8) + style_hash(8) = 27
    let mut buf = [0u8; 27];
    buf[0..8].copy_from_slice(&tile_count.to_le_bytes());
    buf[8..10].copy_from_slice(&layer_count.to_le_bytes());
    buf[10] = zoom_max;
    buf[11..19].copy_from_slice(&bounds_hash.to_le_bytes());
    buf[19..27].copy_from_slice(&style_hash.to_le_bytes());
    MapDbRecord {
        content_hash: fnv1a(&buf),
        tile_count,
        layer_count,
        zoom_max,
        bounds_hash,
        style_hash,
    }
}

// ── Bridge 2: Map → Cache (tile cache entry) ──────────────────────────────

/// Map tile cache entry for ALICE-Cache.
pub struct MapCacheEntry {
    /// Content hash for cache key.
    pub content_hash: u64,
    /// Tile content hash.
    pub tile_hash: u64,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Tile data size in bytes.
    pub tile_bytes: u64,
    /// Zoom level for this tile.
    pub zoom_level: u8,
}

/// Build map tile cache entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn map_to_cache_entry(
    tile_hash: u64,
    ttl_secs: u32,
    tile_bytes: u64,
    zoom_level: u8,
) -> MapCacheEntry {
    // buf: tile_hash(8) + tile_bytes(8) + zoom_level(1) = 17
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&tile_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&tile_bytes.to_le_bytes());
    buf[16] = zoom_level;
    MapCacheEntry {
        content_hash: fnv1a(&buf),
        tile_hash,
        ttl_secs,
        tile_bytes,
        zoom_level,
    }
}

// ── Bridge 3: Map → Analytics (tile request analytics event) ─────────────

/// Map analytics event for ALICE-Analytics ingestion.
pub struct MapAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Total tile request count.
    pub tile_request_count: u64,
    /// Number of unique map views.
    pub unique_views: u64,
    /// Zoom distribution fingerprint hash.
    pub zoom_distribution_hash: u64,
    /// Average tile serving latency in milliseconds.
    pub latency_ms: u32,
    /// Event timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build map analytics event for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn map_to_analytics_event(
    tile_request_count: u64,
    unique_views: u64,
    zoom_distribution_hash: u64,
    latency_ms: u32,
    timestamp_ms: u64,
) -> MapAnalyticsEvent {
    // buf: tile_request_count(8) + unique_views(8) + zoom_distribution_hash(8) + latency_ms(4) + timestamp_ms(8) = 36
    let mut buf = [0u8; 36];
    buf[0..8].copy_from_slice(&tile_request_count.to_le_bytes());
    buf[8..16].copy_from_slice(&unique_views.to_le_bytes());
    buf[16..24].copy_from_slice(&zoom_distribution_hash.to_le_bytes());
    buf[24..28].copy_from_slice(&latency_ms.to_le_bytes());
    buf[28..36].copy_from_slice(&timestamp_ms.to_le_bytes());
    MapAnalyticsEvent {
        content_hash: fnv1a(&buf),
        tile_request_count,
        unique_views,
        zoom_distribution_hash,
        latency_ms,
        timestamp_ms,
    }
}

// ── Bridge 4: Map → Render (tile render descriptor) ──────────────────────

/// Map tile render descriptor for ALICE-Render.
pub struct MapRenderTile {
    /// Content hash.
    pub content_hash: u64,
    /// Tile column index.
    pub tile_x: u32,
    /// Tile row index.
    pub tile_y: u32,
    /// Zoom level.
    pub zoom_level: u8,
    /// Rendered pixel data size in bytes.
    pub pixel_bytes: u64,
    /// Render time in microseconds.
    pub render_time_us: u64,
}

/// Build map tile render descriptor for ALICE-Render.
#[inline]
#[must_use]
pub fn map_to_render_tile(
    tile_x: u32,
    tile_y: u32,
    zoom_level: u8,
    pixel_bytes: u64,
    render_time_us: u64,
) -> MapRenderTile {
    // buf: tile_x(4) + tile_y(4) + zoom_level(1) + pixel_bytes(8) + render_time_us(8) = 25
    let mut buf = [0u8; 25];
    buf[0..4].copy_from_slice(&tile_x.to_le_bytes());
    buf[4..8].copy_from_slice(&tile_y.to_le_bytes());
    buf[8] = zoom_level;
    buf[9..17].copy_from_slice(&pixel_bytes.to_le_bytes());
    buf[17..25].copy_from_slice(&render_time_us.to_le_bytes());
    MapRenderTile {
        content_hash: fnv1a(&buf),
        tile_x,
        tile_y,
        zoom_level,
        pixel_bytes,
        render_time_us,
    }
}

// ── Bridge 5: Map → CDN (tile CDN delivery descriptor) ───────────────────

/// Map tile CDN delivery descriptor for ALICE-CDN.
pub struct MapCdnDelivery {
    /// Content hash.
    pub content_hash: u64,
    /// Tile data size in bytes.
    pub tile_bytes: u64,
    /// Edge cache TTL in seconds.
    pub edge_ttl_secs: u32,
    /// Tile content hash.
    pub tile_hash: u64,
    /// Zoom level for this tile.
    pub zoom_level: u8,
}

/// Build map tile CDN delivery descriptor for ALICE-CDN.
#[inline]
#[must_use]
pub fn map_to_cdn_delivery(
    tile_bytes: u64,
    edge_ttl_secs: u32,
    tile_hash: u64,
    zoom_level: u8,
) -> MapCdnDelivery {
    // buf: tile_bytes(8) + edge_ttl_secs(4) + tile_hash(8) + zoom_level(1) = 21
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&tile_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&edge_ttl_secs.to_le_bytes());
    buf[12..20].copy_from_slice(&tile_hash.to_le_bytes());
    buf[20] = zoom_level;
    MapCdnDelivery {
        content_hash: fnv1a(&buf),
        tile_bytes,
        edge_ttl_secs,
        tile_hash,
        zoom_level,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_to_db_record_hash_nonzero() {
        let rec = map_to_db_record(100_000, 12, 18, 0xdead_beef, 0xcafe_1234);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_map_to_db_record_fields() {
        let rec = map_to_db_record(50_000, 8, 15, 0x1111, 0x2222);
        assert_eq!(rec.tile_count, 50_000);
        assert_eq!(rec.layer_count, 8);
        assert_eq!(rec.zoom_max, 15);
        assert_eq!(rec.bounds_hash, 0x1111);
        assert_eq!(rec.style_hash, 0x2222);
    }

    #[test]
    fn test_map_to_db_record_determinism() {
        let a = map_to_db_record(1_000, 4, 10, 0xab, 0xcd);
        let b = map_to_db_record(1_000, 4, 10, 0xab, 0xcd);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_map_to_cache_entry_hash_nonzero() {
        let entry = map_to_cache_entry(0xbeef_cafe_1234_5678, 86_400, 32_768, 12);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_map_to_cache_entry_fields() {
        let entry = map_to_cache_entry(0x9999, 3_600, 16_384, 8);
        assert_eq!(entry.tile_hash, 0x9999);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_eq!(entry.tile_bytes, 16_384);
        assert_eq!(entry.zoom_level, 8);
    }

    #[test]
    fn test_map_to_analytics_event_hash_nonzero() {
        let ev = map_to_analytics_event(1_000_000, 50_000, 0xaabb_ccdd, 25, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_map_to_analytics_event_fields() {
        let ev = map_to_analytics_event(500_000, 20_000, 0x1234, 30, 99_999);
        assert_eq!(ev.tile_request_count, 500_000);
        assert_eq!(ev.unique_views, 20_000);
        assert_eq!(ev.zoom_distribution_hash, 0x1234);
        assert_eq!(ev.latency_ms, 30);
        assert_eq!(ev.timestamp_ms, 99_999);
    }

    #[test]
    fn test_map_to_render_tile_hash_nonzero() {
        let tile = map_to_render_tile(512, 256, 10, 65_536, 1_250);
        assert_ne!(tile.content_hash, 0);
    }

    #[test]
    fn test_map_to_render_tile_fields() {
        let tile = map_to_render_tile(100, 200, 12, 32_768, 800);
        assert_eq!(tile.tile_x, 100);
        assert_eq!(tile.tile_y, 200);
        assert_eq!(tile.zoom_level, 12);
        assert_eq!(tile.pixel_bytes, 32_768);
        assert_eq!(tile.render_time_us, 800);
    }

    #[test]
    fn test_map_to_cdn_delivery_hash_nonzero() {
        let delivery = map_to_cdn_delivery(16_384, 604_800, 0xface_cafe_0001, 14);
        assert_ne!(delivery.content_hash, 0);
    }

    #[test]
    fn test_map_to_cdn_delivery_fields() {
        let delivery = map_to_cdn_delivery(8_192, 3_600, 0x5555, 10);
        assert_eq!(delivery.tile_bytes, 8_192);
        assert_eq!(delivery.edge_ttl_secs, 3_600);
        assert_eq!(delivery.tile_hash, 0x5555);
        assert_eq!(delivery.zoom_level, 10);
    }

    #[test]
    fn test_map_to_cdn_delivery_determinism() {
        let a = map_to_cdn_delivery(1, 2, 3, 4);
        let b = map_to_cdn_delivery(1, 2, 3, 4);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
