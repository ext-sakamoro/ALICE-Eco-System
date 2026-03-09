//! Astro bridges — ALICE-Astro ↔ DB, Cache, Analytics, Space, Render
//!
//! 5 bridges connecting astronomical observation data to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Astro → DB (catalogue record) ───────────────────────────────

/// Astronomical catalogue record for ALICE-DB persistence.
pub struct AstroDbRecord {
    /// Content hash over the catalogue snapshot.
    pub content_hash: u64,
    /// Number of catalogued objects.
    pub object_count: u64,
    /// Total number of stored observations.
    pub observation_count: u64,
    /// Hash of the catalogue identifier (e.g. Gaia DR3).
    pub catalog_hash: u64,
    /// Observation epoch in Julian days multiplied by 1000.
    pub epoch_jd_x1000: u64,
    /// Number of photometric bands covered.
    pub band_count: u8,
}

/// Serialize an astronomical catalogue snapshot for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn astro_to_db_record(
    object_count: u64,
    observation_count: u64,
    catalog_hash: u64,
    epoch_jd_x1000: u64,
    band_count: u8,
) -> AstroDbRecord {
    let mut buf = [0u8; 33];
    buf[0..8].copy_from_slice(&object_count.to_le_bytes());
    buf[8..16].copy_from_slice(&observation_count.to_le_bytes());
    buf[16..24].copy_from_slice(&catalog_hash.to_le_bytes());
    buf[24..32].copy_from_slice(&epoch_jd_x1000.to_le_bytes());
    buf[32] = band_count;
    AstroDbRecord {
        content_hash: fnv1a(&buf),
        object_count,
        observation_count,
        catalog_hash,
        epoch_jd_x1000,
        band_count,
    }
}

// ── Bridge 2: Astro → Cache (sky tile cache) ──────────────────────────────

/// Sky tile cache entry for ALICE-Cache.
pub struct AstroCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of objects within this tile.
    pub object_count: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Byte size of the serialised tile.
    pub tile_bytes: u64,
    /// HEALPix zoom level of this tile.
    pub zoom_level: u8,
}

/// Build a sky tile cache entry for ALICE-Cache.
///
/// Low zoom (all-sky overview) tiles receive a longer TTL (86 400 s vs 3 600 s)
/// because they are accessed by many clients and rarely change.
#[inline]
#[must_use]
pub fn astro_to_cache_entry(object_count: u64, tile_bytes: u64, zoom_level: u8) -> AstroCacheEntry {
    let mut buf = [0u8; 17];
    buf[0..8].copy_from_slice(&object_count.to_le_bytes());
    buf[8..16].copy_from_slice(&tile_bytes.to_le_bytes());
    buf[16] = zoom_level;
    let high_zoom = (zoom_level > 8) as u32;
    let ttl_secs = 86_400 - high_zoom * 82_800;
    AstroCacheEntry {
        content_hash: fnv1a(&buf),
        object_count,
        ttl_secs,
        tile_bytes,
        zoom_level,
    }
}

// ── Bridge 3: Astro → Analytics (observation pipeline event) ─────────────

/// Observation pipeline event for ALICE-Analytics ingestion.
pub struct AstroAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of observations processed.
    pub observation_count: u64,
    /// Pipeline processing time in microseconds.
    pub processing_time_us: u64,
    /// Signal-to-noise ratio multiplied by 100.
    pub snr_x100: u32,
    /// Airmass at the time of observation multiplied by 1000.
    pub airmass_x1000: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an observation pipeline event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn astro_to_analytics_event(
    observation_count: u64,
    processing_time_us: u64,
    snr_x100: u32,
    airmass_x1000: u32,
    timestamp_ms: u64,
) -> AstroAnalyticsEvent {
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&observation_count.to_le_bytes());
    buf[8..16].copy_from_slice(&processing_time_us.to_le_bytes());
    buf[16..20].copy_from_slice(&snr_x100.to_le_bytes());
    buf[20..24].copy_from_slice(&airmass_x1000.to_le_bytes());
    buf[24..32].copy_from_slice(&timestamp_ms.to_le_bytes());
    AstroAnalyticsEvent {
        content_hash: fnv1a(&buf),
        observation_count,
        processing_time_us,
        snr_x100,
        airmass_x1000,
        timestamp_ms,
    }
}

// ── Bridge 4: Astro → Space (object position link) ────────────────────────

/// Astrometric position link for ALICE-Space integration.
pub struct AstroSpaceLink {
    /// Content hash over the position descriptor.
    pub content_hash: u64,
    /// Right ascension in milli-arcseconds.
    pub ra_mas: i64,
    /// Declination in milli-arcseconds.
    pub dec_mas: i64,
    /// Distance in parsecs multiplied by 1000.
    pub distance_pc_x1000: u64,
    /// Apparent magnitude multiplied by 100 (signed).
    pub magnitude_x100: i16,
    /// Object type code (e.g. 0 = star, 1 = galaxy, 2 = nebula).
    pub object_type: u8,
}

/// Build an astrometric position link for ALICE-Space.
#[inline]
#[must_use]
pub fn astro_to_space_link(
    ra_mas: i64,
    dec_mas: i64,
    distance_pc_x1000: u64,
    magnitude_x100: i16,
    object_type: u8,
) -> AstroSpaceLink {
    let mut buf = [0u8; 27];
    buf[0..8].copy_from_slice(&ra_mas.to_le_bytes());
    buf[8..16].copy_from_slice(&dec_mas.to_le_bytes());
    buf[16..24].copy_from_slice(&distance_pc_x1000.to_le_bytes());
    buf[24..26].copy_from_slice(&magnitude_x100.to_le_bytes());
    buf[26] = object_type;
    AstroSpaceLink {
        content_hash: fnv1a(&buf),
        ra_mas,
        dec_mas,
        distance_pc_x1000,
        magnitude_x100,
        object_type,
    }
}

// ── Bridge 5: Astro → Render (sky tile render output) ────────────────────

/// Sky tile render output for ALICE-Render.
pub struct AstroRenderTile {
    /// Content hash over the render payload.
    pub content_hash: u64,
    /// Tile column index in the sky tessellation.
    pub tile_x: u32,
    /// Tile row index in the sky tessellation.
    pub tile_y: u32,
    /// HEALPix zoom level.
    pub zoom_level: u8,
    /// Byte size of the rendered tile pixels.
    pub pixel_bytes: u64,
    /// Render latency in microseconds.
    pub render_time_us: u64,
}

/// Build a sky tile render output for ALICE-Render.
#[inline]
#[must_use]
pub fn astro_to_render_tile(
    tile_x: u32,
    tile_y: u32,
    zoom_level: u8,
    pixel_bytes: u64,
    render_time_us: u64,
) -> AstroRenderTile {
    let mut buf = [0u8; 25];
    buf[0..4].copy_from_slice(&tile_x.to_le_bytes());
    buf[4..8].copy_from_slice(&tile_y.to_le_bytes());
    buf[8] = zoom_level;
    buf[9..17].copy_from_slice(&pixel_bytes.to_le_bytes());
    buf[17..25].copy_from_slice(&render_time_us.to_le_bytes());
    AstroRenderTile {
        content_hash: fnv1a(&buf),
        tile_x,
        tile_y,
        zoom_level,
        pixel_bytes,
        render_time_us,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_astro_db_record_hash_nonzero() {
        let rec = astro_to_db_record(1_811_709_771, 10_000_000, 0x6761_6961, 2_451_545_000, 3);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_astro_db_record_fields() {
        let rec = astro_to_db_record(500_000, 2_000_000, 0x326d_6173, 2_459_000_000, 2);
        assert_eq!(rec.object_count, 500_000);
        assert_eq!(rec.band_count, 2);
        assert_eq!(rec.epoch_jd_x1000, 2_459_000_000);
    }

    #[test]
    fn test_astro_db_record_determinism() {
        let a = astro_to_db_record(100_000, 400_000, 0x7364_7373, 2_456_000_000, 5);
        let b = astro_to_db_record(100_000, 400_000, 0x7364_7373, 2_456_000_000, 5);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_astro_cache_entry_low_zoom_ttl() {
        let entry = astro_to_cache_entry(10_000_000, 4_194_304, 4);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 86_400);
    }

    #[test]
    fn test_astro_cache_entry_high_zoom_ttl() {
        let entry = astro_to_cache_entry(500, 262_144, 12);
        assert_eq!(entry.ttl_secs, 3_600);
        assert_eq!(entry.zoom_level, 12);
    }

    #[test]
    fn test_astro_analytics_event() {
        let ev = astro_to_analytics_event(1_000, 250_000, 4_500, 1_200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.snr_x100, 4_500);
        assert_eq!(ev.airmass_x1000, 1_200);
    }

    #[test]
    fn test_astro_space_link() {
        let l = astro_to_space_link(83_822_000, -5_391_000, 411_000, 85, 0);
        assert_ne!(l.content_hash, 0);
        assert_eq!(l.object_type, 0);
        assert_eq!(l.magnitude_x100, 85);
    }

    #[test]
    fn test_astro_render_tile() {
        let t = astro_to_render_tile(42, 17, 8, 786_432, 4_500);
        assert_ne!(t.content_hash, 0);
        assert_eq!(t.tile_x, 42);
        assert_eq!(t.zoom_level, 8);
    }

    #[test]
    fn test_astro_render_tile_determinism() {
        let a = astro_to_render_tile(0, 0, 0, 4_194_304, 12_000);
        let b = astro_to_render_tile(0, 0, 0, 4_194_304, 12_000);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
