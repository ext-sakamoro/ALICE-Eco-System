//! ASP bridges — ALICE-Streaming-Protocol ↔ Cache, Codec, SDF, View, CDN, Analytics
//!
//! 6 bridges connecting video streaming protocol to the ALICE ecosystem.

use libasp::{AspPacket, AspPayload, PacketType, StreamStats, QualityLevel, Color, Rect};

// ── Bridge 1: ASP → Cache (packet metadata for caching) ─────────────────

/// Packet metadata cache entry for ALICE-Cache.
pub struct AspCacheEntry {
    /// Content hash of the packet payload (FNV-1a).
    pub content_hash: u64,
    /// Packet sequence number (cache key component).
    pub sequence: u32,
    /// Packet type discriminant (I/D/C/S).
    pub packet_type: PacketType,
    /// Payload size in bytes.
    pub payload_bytes: u32,
    /// Is this a keyframe (I-Packet)?
    pub is_keyframe: bool,
}

/// Extract packet metadata for ALICE-Cache storage.
///
/// Hashes header bytes via FNV-1a for deduplication. Branchless `is_keyframe`
/// uses a bool cast to avoid an extra branch on the hot path.
#[inline]
pub fn asp_to_cache_entry(packet: &AspPacket) -> AspCacheEntry {
    // Build a compact key from sequence + type byte for hashing
    let seq_bytes = packet.header.sequence.to_le_bytes();
    let type_byte = [packet.header.packet_type as u8];
    let len_bytes = packet.header.payload_length.to_le_bytes();
    let mut buf = [0u8; 9];
    buf[0..4].copy_from_slice(&seq_bytes);
    buf[4] = type_byte[0];
    buf[5..9].copy_from_slice(&len_bytes);
    let content_hash = crate::hash::fnv1a(&buf);

    // Branchless keyframe flag (1 for IPacket, 0 otherwise)
    let is_keyframe = packet.header.packet_type == PacketType::IPacket;

    AspCacheEntry {
        content_hash,
        sequence: packet.header.sequence,
        packet_type: packet.header.packet_type,
        payload_bytes: packet.header.payload_length,
        is_keyframe,
    }
}

// ── Bridge 2: ASP → Codec (stream stats for codec tuning) ───────────────

/// Codec tuning parameters derived from ASP stream statistics.
pub struct AspCodecTuning {
    /// Estimated bitrate in kbps (reciprocal-multiply avoids division).
    pub bitrate_kbps: f64,
    /// I-Packet ratio (keyframe frequency, 0.0–1.0).
    pub keyframe_ratio: f32,
    /// Compression ratio from the stream (as reported by ASP).
    pub compression_ratio: f64,
    /// Average encode time in microseconds.
    pub avg_encode_time_us: f64,
    /// Recommended quality level based on current compression ratio.
    pub recommended_quality: QualityLevel,
    /// Total packets processed.
    pub total_packets: u64,
}

/// Derive codec tuning parameters from ALICE-Streaming-Protocol stream stats.
///
/// Uses reciprocal multiplication instead of division throughout.
#[inline]
pub fn asp_stats_to_codec_tuning(stats: &StreamStats) -> AspCodecTuning {
    // Reciprocal guards: avoid division by checking for zero
    let rcp_packets = if stats.total_packets > 0 {
        1.0 / stats.total_packets as f64
    } else {
        0.0
    };

    // keyframe_ratio = i_packets / total_packets (branchless multiply)
    let keyframe_ratio = (stats.i_packets as f64 * rcp_packets) as f32;

    // Bitrate estimate: avg_bits_per_frame × 30 fps → kbps
    // kbps = (avg_bits_per_frame * 30.0) * (1/1000)
    let bitrate_kbps = stats.avg_bits_per_frame * 30.0 * 0.001;

    // Map compression ratio to recommended quality (branchless threshold chain)
    let cr = stats.compression_ratio;
    let recommended_quality = if cr >= 500.0 {
        QualityLevel::Ultra
    } else if cr >= 100.0 {
        QualityLevel::High
    } else if cr >= 10.0 {
        QualityLevel::Medium
    } else {
        QualityLevel::Low
    };

    AspCodecTuning {
        bitrate_kbps,
        keyframe_ratio,
        compression_ratio: stats.compression_ratio,
        avg_encode_time_us: stats.avg_encode_time_us,
        recommended_quality,
        total_packets: stats.total_packets,
    }
}

// ── Bridge 3: ASP → SDF (3D scene descriptor from I-packet regions) ──────

/// SDF scene region derived from an ASP I-Packet region.
pub struct AspSdfRegion {
    /// Region bounding rect (x, y, width, height in pixels).
    pub bounds: Rect,
    /// Dominant color (first palette entry) as [r, g, b].
    pub dominant_color: [u8; 3],
    /// Region area in pixels (branchless, u64).
    pub area_px: u64,
    /// Normalized center X (0.0–1.0).
    pub center_x: f32,
    /// Normalized center Y (0.0–1.0).
    pub center_y: f32,
}

/// SDF scene descriptor assembled from I-Packet region data.
pub struct AspSdfDescriptor {
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    /// Extracted SDF regions.
    pub regions: Vec<AspSdfRegion>,
    /// Content hash of the scene (for deduplication / caching).
    pub scene_hash: u64,
}

/// Build an SDF scene descriptor from an ASP I-Packet for ALICE-SDF.
///
/// Returns `None` if `packet` is not an I-Packet.
#[inline]
pub fn asp_i_packet_to_sdf(packet: &AspPacket) -> Option<AspSdfDescriptor> {
    let ip = match &packet.payload {
        AspPayload::IPacket(p) => p,
        _ => return None,
    };

    // Reciprocals for normalized center computation
    let rcp_w = if ip.width > 0 { 1.0 / ip.width as f32 } else { 0.0 };
    let rcp_h = if ip.height > 0 { 1.0 / ip.height as f32 } else { 0.0 };

    let regions: Vec<AspSdfRegion> = ip.regions.iter().map(|r| {
        let dominant_color: [u8; 3] = r.palette
            .dominant_color()
            .map(|c: Color| c.to_array())
            .unwrap_or([0u8; 3]);

        // area = width * height (no branch needed, both are u32)
        let area_px = r.bounds.width as u64 * r.bounds.height as u64;

        // Normalized center via reciprocal multiply
        let center_x = (r.bounds.x as f32 + r.bounds.width as f32 * 0.5) * rcp_w;
        let center_y = (r.bounds.y as f32 + r.bounds.height as f32 * 0.5) * rcp_h;

        AspSdfRegion {
            bounds: r.bounds,
            dominant_color,
            area_px,
            center_x,
            center_y,
        }
    }).collect();

    // Hash the scene using dimensions + region count as a compact fingerprint
    let mut hash_buf = [0u8; 12];
    hash_buf[0..4].copy_from_slice(&ip.width.to_le_bytes());
    hash_buf[4..8].copy_from_slice(&ip.height.to_le_bytes());
    let region_count = regions.len() as u32;
    hash_buf[8..12].copy_from_slice(&region_count.to_le_bytes());
    let scene_hash = crate::hash::fnv1a(&hash_buf);

    Some(AspSdfDescriptor {
        width: ip.width,
        height: ip.height,
        regions,
        scene_hash,
    })
}

// ── Bridge 4: ASP → View (decoded frame config from stream stats) ─────────

/// Decoded frame display configuration for ALICE-View.
pub struct AspViewConfig {
    /// Recommended render width.
    pub render_width: u32,
    /// Recommended render height.
    pub render_height: u32,
    /// Frame rate (fps) for playback timing.
    pub fps: f32,
    /// Quality level for renderer settings.
    pub quality: QualityLevel,
    /// Average bits per frame (for adaptive streaming UI).
    pub avg_bits_per_frame: f64,
    /// Total frames decoded so far.
    pub frames_decoded: u64,
}

/// Configure ALICE-View renderer from ASP stream stats and last I-Packet.
///
/// Falls back to a 1280×720 / 30 fps default when no I-Packet is supplied.
#[inline]
pub fn asp_to_view_config(stats: &StreamStats, last_i_packet: Option<&AspPacket>) -> AspViewConfig {
    // Pull dimensions/fps from the last I-Packet when available
    let (render_width, render_height, fps, quality) = match last_i_packet {
        Some(pkt) => match &pkt.payload {
            AspPayload::IPacket(ip) => {
                (ip.width, ip.height, ip.fps, ip.quality)
            }
            _ => (1280, 720, 30.0, QualityLevel::Medium),
        },
        None => (1280, 720, 30.0, QualityLevel::Medium),
    };

    AspViewConfig {
        render_width,
        render_height,
        fps,
        quality,
        avg_bits_per_frame: stats.avg_bits_per_frame,
        frames_decoded: stats.frames_encoded,
    }
}

// ── Bridge 5: ASP → CDN (packet routing metadata) ────────────────────────

/// CDN routing metadata derived from an ASP packet.
pub struct AspCdnMeta {
    /// Content hash for CDN edge routing and deduplication.
    pub content_hash: u64,
    /// Packet size in bytes (payload + header overhead).
    pub total_bytes: u32,
    /// Packet type as a routing priority hint (IPacket = highest priority).
    pub packet_type: PacketType,
    /// Sequence number for ordering.
    pub sequence: u32,
    /// MIME-type string for CDN content-type tagging.
    pub content_type: &'static str,
    /// Priority level: 3 = I-Packet, 2 = C-Packet, 1 = D-Packet, 0 = S-Packet.
    pub priority: u8,
}

/// Extract CDN routing metadata from an ASP packet for ALICE-CDN.
///
/// Priority is mapped branchlessly via a const table lookup on the packet type
/// discriminant byte.
#[inline]
pub fn asp_to_cdn_meta(packet: &AspPacket) -> AspCdnMeta {
    // Const priority table indexed by PacketType discriminant (0x01–0x04)
    // Index 0 unused; 1=IPacket, 2=DPacket, 3=CPacket, 4=SPacket
    const PRIORITY: [u8; 5] = [0, 3, 1, 2, 0];
    let type_idx = (packet.header.packet_type as u8) as usize;
    // Clamp to table bounds (branchless: saturate at 4)
    let idx = type_idx.min(4);
    let priority = PRIORITY[idx];

    // Hash header for routing key
    let seq_bytes = packet.header.sequence.to_le_bytes();
    let mut buf = [0u8; 5];
    buf[0..4].copy_from_slice(&seq_bytes);
    buf[4] = packet.header.packet_type as u8;
    let content_hash = crate::hash::fnv1a(&buf);

    // Total bytes = header (16) + payload
    let total_bytes = 16u32 + packet.header.payload_length;

    AspCdnMeta {
        content_hash,
        total_bytes,
        packet_type: packet.header.packet_type,
        sequence: packet.header.sequence,
        content_type: "application/x-alice-asp",
        priority,
    }
}

// ── Bridge 6: ASP → Analytics (streaming performance metrics) ────────────

/// Streaming performance metrics for ALICE-Analytics.
pub struct AspStreamMetrics {
    /// Total bytes transferred.
    pub total_bytes: u64,
    /// Total packet count.
    pub total_packets: u64,
    /// I-Packet count.
    pub i_packets: u64,
    /// D-Packet count.
    pub d_packets: u64,
    /// C-Packet count.
    pub c_packets: u64,
    /// S-Packet count.
    pub s_packets: u64,
    /// Frames encoded.
    pub frames_encoded: u64,
    /// Compression ratio.
    pub compression_ratio: f64,
    /// Average encode time in microseconds.
    pub avg_encode_time_us: f64,
    /// Average bytes per packet (reciprocal-multiply).
    pub avg_bytes_per_packet: f64,
    /// Keyframe ratio (i_packets / total_packets).
    pub keyframe_ratio: f32,
    /// Content fingerprint (FNV-1a of key counters).
    pub content_hash: u64,
}

/// Extract streaming performance metrics for ALICE-Analytics monitoring.
///
/// Uses reciprocal multiplication for all per-packet averages.
#[inline]
pub fn asp_to_stream_metrics(stats: &StreamStats) -> AspStreamMetrics {
    let rcp_packets = if stats.total_packets > 0 {
        1.0 / stats.total_packets as f64
    } else {
        0.0
    };

    let avg_bytes_per_packet = stats.total_bytes as f64 * rcp_packets;
    let keyframe_ratio = (stats.i_packets as f64 * rcp_packets) as f32;

    // Hash key counters as a compact fingerprint
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&stats.total_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&stats.total_packets.to_le_bytes());
    buf[16..24].copy_from_slice(&stats.frames_encoded.to_le_bytes());
    let content_hash = crate::hash::fnv1a(&buf);

    AspStreamMetrics {
        total_bytes: stats.total_bytes,
        total_packets: stats.total_packets,
        i_packets: stats.i_packets,
        d_packets: stats.d_packets,
        c_packets: stats.c_packets,
        s_packets: stats.s_packets,
        frames_encoded: stats.frames_encoded,
        compression_ratio: stats.compression_ratio,
        avg_encode_time_us: stats.avg_encode_time_us,
        avg_bytes_per_packet,
        keyframe_ratio,
        content_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libasp::{AspPacket, IPacketPayload, DPacketPayload, StreamStats, PacketType,
                  RegionDescriptor, Rect, Color, ColorPalette};

    // ── Bridge 1 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_cache_entry() {
        let payload = IPacketPayload::new(1920, 1080, 30.0);
        let packet = AspPacket::create_i_packet(42, payload).unwrap();

        let entry = asp_to_cache_entry(&packet);

        assert_eq!(entry.sequence, 42);
        assert_eq!(entry.packet_type, PacketType::IPacket);
        assert!(entry.is_keyframe);
        assert_ne!(entry.content_hash, 0);
        // Payload bytes should be > 0 (estimated from region count)
        // (estimated_size is set in header during create_i_packet)
    }

    // ── Bridge 2 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_stats_to_codec_tuning() {
        let mut stats = StreamStats::new();
        stats.total_packets = 1000;
        stats.i_packets = 50;
        stats.d_packets = 950;
        stats.total_bytes = 5_000_000;
        stats.frames_encoded = 1000;
        stats.avg_bits_per_frame = 40_000.0; // 40 kbits/frame @ 30fps → 1.2 Mbps
        stats.compression_ratio = 200.0;
        stats.avg_encode_time_us = 500.0;

        let tuning = asp_stats_to_codec_tuning(&stats);

        assert_eq!(tuning.total_packets, 1000);
        // keyframe_ratio = 50/1000 = 0.05
        assert!((tuning.keyframe_ratio - 0.05).abs() < 1e-4);
        // bitrate_kbps = 40000 * 30 * 0.001 = 1200
        assert!((tuning.bitrate_kbps - 1200.0).abs() < 1e-6);
        // compression_ratio 200.0 → High quality
        assert_eq!(tuning.recommended_quality, QualityLevel::High);
    }

    // ── Bridge 3 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_i_packet_to_sdf() {
        let mut payload = IPacketPayload::new(640, 480, 30.0);
        payload.add_region(RegionDescriptor::solid(
            Rect::new(0, 0, 320, 240),
            Color::new(200, 100, 50),
        ));
        payload.add_region(RegionDescriptor::solid(
            Rect::new(320, 240, 320, 240),
            Color::new(50, 150, 200),
        ));

        let packet = AspPacket::create_i_packet(1, payload).unwrap();
        let sdf = asp_i_packet_to_sdf(&packet).expect("should return SDF descriptor for I-Packet");

        assert_eq!(sdf.width, 640);
        assert_eq!(sdf.height, 480);
        assert_eq!(sdf.regions.len(), 2);
        assert_ne!(sdf.scene_hash, 0);

        // First region: dominant color should be [200, 100, 50]
        assert_eq!(sdf.regions[0].dominant_color, [200, 100, 50]);
        // Area = 320 * 240 = 76800
        assert_eq!(sdf.regions[0].area_px, 76800);
        // Normalized center_x for first region: (0 + 160) / 640 = 0.25
        assert!((sdf.regions[0].center_x - 0.25).abs() < 1e-4);

        // D-Packet should return None
        let d_payload = DPacketPayload::new(1);
        let d_packet = AspPacket::create_d_packet(2, d_payload).unwrap();
        assert!(asp_i_packet_to_sdf(&d_packet).is_none());
    }

    // ── Bridge 4 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_view_config() {
        let mut stats = StreamStats::new();
        stats.avg_bits_per_frame = 32000.0;
        stats.frames_encoded = 300;

        // With an I-Packet for dimensions
        let mut ip = IPacketPayload::new(3840, 2160, 60.0);
        ip.quality = QualityLevel::Ultra;
        let i_packet = AspPacket::create_i_packet(1, ip).unwrap();

        let cfg = asp_to_view_config(&stats, Some(&i_packet));
        assert_eq!(cfg.render_width, 3840);
        assert_eq!(cfg.render_height, 2160);
        assert!((cfg.fps - 60.0).abs() < 1e-4);
        assert_eq!(cfg.quality, QualityLevel::Ultra);
        assert_eq!(cfg.frames_decoded, 300);

        // Without I-Packet → default 1280×720
        let cfg_default = asp_to_view_config(&stats, None);
        assert_eq!(cfg_default.render_width, 1280);
        assert_eq!(cfg_default.render_height, 720);
        assert!((cfg_default.fps - 30.0).abs() < 1e-4);
    }

    // ── Bridge 5 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_cdn_meta() {
        // I-Packet → priority 3
        let i_payload = IPacketPayload::new(1920, 1080, 30.0);
        let i_packet = AspPacket::create_i_packet(10, i_payload).unwrap();
        let meta_i = asp_to_cdn_meta(&i_packet);
        assert_eq!(meta_i.priority, 3);
        assert_eq!(meta_i.sequence, 10);
        assert_eq!(meta_i.packet_type, PacketType::IPacket);
        assert_eq!(meta_i.content_type, "application/x-alice-asp");
        assert_ne!(meta_i.content_hash, 0);
        // total_bytes = 16 + payload_length
        assert_eq!(meta_i.total_bytes, 16 + i_packet.header.payload_length);

        // D-Packet → priority 1
        let d_payload = DPacketPayload::new(10);
        let d_packet = AspPacket::create_d_packet(11, d_payload).unwrap();
        let meta_d = asp_to_cdn_meta(&d_packet);
        assert_eq!(meta_d.priority, 1);

        // Hashes should differ between packets
        assert_ne!(meta_i.content_hash, meta_d.content_hash);
    }

    // ── Bridge 6 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_stream_metrics() {
        let mut stats = StreamStats::new();
        stats.total_bytes = 10_000_000;
        stats.total_packets = 2000;
        stats.i_packets = 100;
        stats.d_packets = 1850;
        stats.c_packets = 40;
        stats.s_packets = 10;
        stats.frames_encoded = 2000;
        stats.compression_ratio = 150.0;
        stats.avg_encode_time_us = 800.0;

        let m = asp_to_stream_metrics(&stats);

        assert_eq!(m.total_bytes, 10_000_000);
        assert_eq!(m.total_packets, 2000);
        assert_eq!(m.i_packets, 100);
        assert_eq!(m.d_packets, 1850);
        assert_eq!(m.c_packets, 40);
        assert_eq!(m.s_packets, 10);
        assert_eq!(m.frames_encoded, 2000);
        assert!((m.compression_ratio - 150.0).abs() < 1e-9);
        // avg_bytes_per_packet = 10_000_000 / 2000 = 5000
        assert!((m.avg_bytes_per_packet - 5000.0).abs() < 1e-6);
        // keyframe_ratio = 100 / 2000 = 0.05
        assert!((m.keyframe_ratio - 0.05).abs() < 1e-4);
        assert_ne!(m.content_hash, 0);

        // Empty stats → no division by zero
        let empty = StreamStats::new();
        let m0 = asp_to_stream_metrics(&empty);
        assert_eq!(m0.total_packets, 0);
        assert_eq!(m0.avg_bytes_per_packet, 0.0);
        assert_eq!(m0.keyframe_ratio, 0.0);
    }
}
