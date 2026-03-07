//! Geo bridges — ALICE-Geo ↔ DB, Analytics, Cache, CDN, Edge
//!
//! 5 bridges connecting the geo processing layer to the ALICE ecosystem.
//! Covers geo record persistence in DB, geo metrics in Analytics,
//! map tile caching, tile CDN delivery, and geo event delivery via Edge.

use alice_geo::Coord;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Geo → DB (geo record persistence) ───────────────────────────

/// Geo record for ALICE-DB persistence.
///
/// Written when a geo query result is committed so the database layer can
/// store and query geo points by geohash prefix or bounding box.
pub struct GeoDbRecord {
    /// FNV-1a hash over lat bits and lon bits.
    pub content_hash: u64,
    /// Latitude in degrees multiplied by 1e7 and cast to i64 for lossless storage.
    pub lat_e7: i64,
    /// Longitude in degrees multiplied by 1e7 and cast to i64 for lossless storage.
    pub lon_e7: i64,
    /// Geohash string byte length (precision level 1–12).
    pub geohash_precision: u8,
}

/// Convert a geo point and geohash precision into a DB record for ALICE-DB.
#[inline]
#[must_use]
pub fn geo_point_to_db_record(point: &Coord, geohash_precision: u8) -> GeoDbRecord {
    let lat_bits = point.lat.to_bits();
    let lon_bits = point.lon.to_bits();
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&lat_bits.to_le_bytes());
    key[8..16].copy_from_slice(&lon_bits.to_le_bytes());
    GeoDbRecord {
        content_hash: fnv1a(&key),
        lat_e7: (point.lat * 1e7) as i64,
        lon_e7: (point.lon * 1e7) as i64,
        geohash_precision: geohash_precision.min(12),
    }
}

// ── Bridge 2: Geo → Analytics (geo metrics event) ─────────────────────────

/// Geo metrics event for ALICE-Analytics.
///
/// Emitted on point-in-polygon checks and distance computations so the
/// analytics layer can compute spatial query rates and coverage statistics.
pub struct GeoAnalyticsMetricsEvent {
    /// FNV-1a hash over lat bits, lon bits, and query type byte.
    pub content_hash: u64,
    /// Latitude scaled to fixed-point i64 (degrees * 1e7).
    pub lat_e7: i64,
    /// Longitude scaled to fixed-point i64 (degrees * 1e7).
    pub lon_e7: i64,
    /// Query type: 0=distance, 1=point_in_polygon, 2=tile_lookup, 3=geohash.
    pub query_type: u8,
    /// Computed distance in meters (0 for non-distance queries).
    pub distance_m: f64,
}

/// Convert a geo point and query result into a metrics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn geo_point_to_analytics_event(
    point: &Coord,
    query_type: u8,
    distance_m: f64,
) -> GeoAnalyticsMetricsEvent {
    let lat_bits = point.lat.to_bits();
    let lon_bits = point.lon.to_bits();
    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&lat_bits.to_le_bytes());
    key[8..16].copy_from_slice(&lon_bits.to_le_bytes());
    key[16] = query_type.min(3);
    GeoAnalyticsMetricsEvent {
        content_hash: fnv1a(&key),
        lat_e7: (point.lat * 1e7) as i64,
        lon_e7: (point.lon * 1e7) as i64,
        query_type: query_type.min(3),
        distance_m,
    }
}

// ── Bridge 3: Geo → Cache (map tile cache entry) ──────────────────────────

/// Map tile cache entry for ALICE-Cache.
///
/// Caches rendered map tiles by (x, y, zoom) so the tile server avoids
/// redundant rendering for frequently requested tiles.
/// High-zoom tiles (zoom >= 14) receive a longer TTL because they change
/// less frequently than lower-zoom overview tiles.
pub struct GeoCacheTileEntry {
    /// FNV-1a hash over tile x, y, and zoom bytes — cache key.
    pub content_hash: u64,
    /// Tile X coordinate.
    pub tile_x: u32,
    /// Tile Y coordinate.
    pub tile_y: u32,
    /// Zoom level (0–22).
    pub zoom: u8,
    /// Cache TTL in seconds: 3600 for high-zoom tiles (>= 14), else 300.
    pub ttl_secs: u32,
}

/// Build a map tile cache entry for ALICE-Cache.
///
/// TTL is computed branchlessly: high-zoom (>= 14) → 3600 s; else → 300 s.
#[inline]
#[must_use]
pub fn geo_tile_to_cache_entry(tile_x: u32, tile_y: u32, zoom: u8) -> GeoCacheTileEntry {
    let mut key = [0u8; 9];
    key[0..4].copy_from_slice(&tile_x.to_le_bytes());
    key[4..8].copy_from_slice(&tile_y.to_le_bytes());
    key[8] = zoom;
    // Branchless TTL: high_zoom=1 → 300+3300=3600, low_zoom=0 → 300.
    let high_zoom = (zoom >= 14) as u32;
    let ttl_secs = 300 + high_zoom * 3300;
    GeoCacheTileEntry {
        content_hash: fnv1a(&key),
        tile_x,
        tile_y,
        zoom,
        ttl_secs,
    }
}

// ── Bridge 4: Geo → CDN (tile delivery descriptor) ───────────────────────

/// Tile delivery descriptor for ALICE-CDN.
///
/// Packages a map tile request as a CDN delivery record so the CDN layer
/// can route tile requests to the nearest edge PoP and apply appropriate
/// cache-control headers.
pub struct GeoCdnTileDelivery {
    /// FNV-1a hash over tile x, y, zoom, and format bytes.
    pub content_hash: u64,
    /// Tile X coordinate.
    pub tile_x: u32,
    /// Tile Y coordinate.
    pub tile_y: u32,
    /// Zoom level.
    pub zoom: u8,
    /// Tile format: 0=PNG, 1=WebP, 2=AVIF.
    pub format: u8,
    /// CDN cache-control max-age in seconds.
    pub max_age_secs: u32,
}

/// Build a CDN tile delivery descriptor for ALICE-CDN.
///
/// max-age mirrors the cache TTL: high-zoom tiles get 3600 s, others 300 s.
#[inline]
#[must_use]
pub fn geo_tile_to_cdn_delivery(tile_x: u32, tile_y: u32, zoom: u8, format: u8) -> GeoCdnTileDelivery {
    let fmt = format.min(2);
    let mut key = [0u8; 10];
    key[0..4].copy_from_slice(&tile_x.to_le_bytes());
    key[4..8].copy_from_slice(&tile_y.to_le_bytes());
    key[8] = zoom;
    key[9] = fmt;
    let high_zoom = (zoom >= 14) as u32;
    let max_age_secs = 300 + high_zoom * 3300;
    GeoCdnTileDelivery {
        content_hash: fnv1a(&key),
        tile_x,
        tile_y,
        zoom,
        format: fmt,
        max_age_secs,
    }
}

// ── Bridge 5: Geo → Edge (geo event delivery) ─────────────────────────────

/// Geo event payload for ALICE-Edge delivery.
///
/// Packages a geo fence crossing or proximity alert as an edge event so
/// the edge layer can deliver location-triggered notifications to devices.
pub struct GeoEdgeEvent {
    /// FNV-1a hash over lat bits, lon bits, and event type byte.
    pub content_hash: u64,
    /// Latitude in degrees * 1e7 (fixed-point).
    pub lat_e7: i64,
    /// Longitude in degrees * 1e7 (fixed-point).
    pub lon_e7: i64,
    /// Event type: 0=geofence_enter, 1=geofence_exit, 2=proximity_alert.
    pub event_type: u8,
    /// Distance to fence or target in meters.
    pub distance_m: f64,
}

/// Convert a geo point into an edge geo event for ALICE-Edge.
#[inline]
#[must_use]
pub fn geo_point_to_edge_event(
    point: &Coord,
    event_type: u8,
    distance_m: f64,
) -> GeoEdgeEvent {
    let lat_bits = point.lat.to_bits();
    let lon_bits = point.lon.to_bits();
    let evt = event_type.min(2);
    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&lat_bits.to_le_bytes());
    key[8..16].copy_from_slice(&lon_bits.to_le_bytes());
    key[16] = evt;
    GeoEdgeEvent {
        content_hash: fnv1a(&key),
        lat_e7: (point.lat * 1e7) as i64,
        lon_e7: (point.lon * 1e7) as i64,
        event_type: evt,
        distance_m,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_geo::Coord;

    fn tokyo() -> Coord {
        Coord { lat: 35.6762, lon: 139.6503 }
    }

    fn london() -> Coord {
        Coord { lat: 51.5074, lon: -0.1278 }
    }

    #[test]
    fn test_geo_point_to_db_record() {
        let p = tokyo();
        let rec = geo_point_to_db_record(&p, 8);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.geohash_precision, 8);
        // lat_e7 = 35.6762 * 1e7 ≈ 356_762_000
        assert!(rec.lat_e7 > 356_000_000 && rec.lat_e7 < 357_000_000);
        assert!(rec.lon_e7 > 1_396_000_000 && rec.lon_e7 < 1_397_000_000);
    }

    #[test]
    fn test_geo_precision_clamped_to_12() {
        let rec = geo_point_to_db_record(&tokyo(), 20);
        assert_eq!(rec.geohash_precision, 12);
    }

    #[test]
    fn test_geo_to_analytics_event_distance() {
        let p = london();
        let ev = geo_point_to_analytics_event(&p, 0, 12_345.6);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.query_type, 0);
        assert_eq!(ev.distance_m, 12_345.6);
    }

    #[test]
    fn test_geo_to_analytics_event_query_type_clamped() {
        let p = tokyo();
        let ev = geo_point_to_analytics_event(&p, 99, 0.0);
        assert_eq!(ev.query_type, 3);
    }

    #[test]
    fn test_tile_cache_entry_low_zoom_ttl() {
        // zoom = 10 < 14 → ttl = 300
        let entry = geo_tile_to_cache_entry(512, 384, 10);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.tile_x, 512);
        assert_eq!(entry.tile_y, 384);
        assert_eq!(entry.zoom, 10);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_tile_cache_entry_high_zoom_ttl() {
        // zoom = 16 >= 14 → ttl = 3600
        let entry = geo_tile_to_cache_entry(100, 200, 16);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn test_cdn_tile_delivery_format_clamped() {
        let d = geo_tile_to_cdn_delivery(0, 0, 5, 99);
        assert_eq!(d.format, 2); // clamped to 2 (AVIF)
        assert_eq!(d.max_age_secs, 300); // zoom=5 < 14
    }

    #[test]
    fn test_geo_edge_event_geofence_enter() {
        let p = london();
        let ev = geo_point_to_edge_event(&p, 0, 45.2);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.event_type, 0);
        assert_eq!(ev.distance_m, 45.2);
    }

    #[test]
    fn test_hash_determinism() {
        let p = tokyo();
        let r1 = geo_point_to_db_record(&p, 6);
        let r2 = geo_point_to_db_record(&p, 6);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_eq!(r1.lat_e7, r2.lat_e7);
    }
}
