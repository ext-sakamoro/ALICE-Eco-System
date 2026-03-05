//! ASP bridges — ALICE-Streaming-Protocol ↔ Cache, Codec, SDF, View, CDN, Analytics, DB, Sync, Voice, Queue, Auth, ML
//!
//! 12 bridges connecting video streaming protocol to the ALICE ecosystem.

use libasp::{AspPacket, AspPayload, Color, PacketType, QualityLevel, Rect, StreamStats};

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
#[must_use]
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
#[must_use]
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
#[must_use]
pub fn asp_i_packet_to_sdf(packet: &AspPacket) -> Option<AspSdfDescriptor> {
    let ip = match &packet.payload {
        AspPayload::IPacket(p) => p,
        AspPayload::DPacket(_) | AspPayload::CPacket(_) | AspPayload::SPacket(_) => return None,
    };

    // Reciprocals for normalized center computation
    let rcp_w = if ip.width > 0 {
        1.0 / ip.width as f32
    } else {
        0.0
    };
    let rcp_h = if ip.height > 0 {
        1.0 / ip.height as f32
    } else {
        0.0
    };

    let regions: Vec<AspSdfRegion> = ip
        .regions
        .iter()
        .map(|r| {
            let dominant_color: [u8; 3] = r
                .palette
                .dominant_color()
                .map_or([0u8; 3], |c: Color| c.to_array());

            // area = width * height (no branch needed, both are u32)
            let area_px = r.bounds.width as u64 * r.bounds.height as u64;

            // Normalized center via reciprocal multiply
            let center_x = (r.bounds.width as f32).mul_add(0.5, r.bounds.x as f32) * rcp_w;
            let center_y = (r.bounds.height as f32).mul_add(0.5, r.bounds.y as f32) * rcp_h;

            AspSdfRegion {
                bounds: r.bounds,
                dominant_color,
                area_px,
                center_x,
                center_y,
            }
        })
        .collect();

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
#[must_use]
pub const fn asp_to_view_config(
    stats: &StreamStats,
    last_i_packet: Option<&AspPacket>,
) -> AspViewConfig {
    // Pull dimensions/fps from the last I-Packet when available
    let (render_width, render_height, fps, quality) = match last_i_packet {
        Some(pkt) => match &pkt.payload {
            AspPayload::IPacket(ip) => (ip.width, ip.height, ip.fps, ip.quality),
            AspPayload::DPacket(_) | AspPayload::CPacket(_) | AspPayload::SPacket(_) => {
                (1280, 720, 30.0, QualityLevel::Medium)
            }
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
    /// Packet type as a routing priority hint (`IPacket` = highest priority).
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
#[must_use]
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
    /// Keyframe ratio (`i_packets` / `total_packets`).
    pub keyframe_ratio: f32,
    /// Content fingerprint (FNV-1a of key counters).
    pub content_hash: u64,
}

/// Extract streaming performance metrics for ALICE-Analytics monitoring.
///
/// Uses reciprocal multiplication for all per-packet averages.
#[inline]
#[must_use]
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

// ── Bridge 7: ASP → DB (stream session persistence) ────────────────────

/// Stream session record for ALICE-DB persistence.
///
/// Captures a snapshot of streaming state suitable for database storage.
/// Fields are chosen for efficient indexing (`session_hash`, `channel_id`, quality).
pub struct AspDbSessionRecord {
    /// Session fingerprint (FNV-1a of stream stats counters).
    pub session_hash: u64,
    /// Channel identifier (derived from sequence range hash).
    pub channel_id: u64,
    /// Total bytes transferred in this session.
    pub total_bytes: u64,
    /// Total packets in this session.
    pub total_packets: u64,
    /// I-Packet count (keyframes).
    pub i_packets: u64,
    /// Compression ratio achieved.
    pub compression_ratio: f64,
    /// Average encode time in microseconds.
    pub avg_encode_time_us: f64,
    /// Quality level at time of snapshot.
    pub quality: QualityLevel,
    /// Estimated bitrate in kbps.
    pub bitrate_kbps: f64,
}

/// Convert ASP stream stats to a DB session record for ALICE-DB.
///
/// Uses FNV-1a to generate a deterministic session fingerprint from the
/// stream's counter state. Bitrate uses reciprocal multiplication.
#[inline]
#[must_use]
pub fn asp_to_db_session_record(stats: &StreamStats, quality: QualityLevel) -> AspDbSessionRecord {
    // Session fingerprint from counters
    let mut buf = [0u8; 32];
    buf[0..8].copy_from_slice(&stats.total_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&stats.total_packets.to_le_bytes());
    buf[16..24].copy_from_slice(&stats.frames_encoded.to_le_bytes());
    buf[24..32].copy_from_slice(&stats.i_packets.to_le_bytes());
    let session_hash = crate::hash::fnv1a(&buf);

    // Channel ID from a different hash seed (XOR with constant)
    let channel_id = session_hash ^ 0x517cc1b727220a95;

    // Bitrate: avg_bits_per_frame × 30fps × (1/1000) → kbps
    let bitrate_kbps = stats.avg_bits_per_frame * 30.0 * 0.001;

    AspDbSessionRecord {
        session_hash,
        channel_id,
        total_bytes: stats.total_bytes,
        total_packets: stats.total_packets,
        i_packets: stats.i_packets,
        compression_ratio: stats.compression_ratio,
        avg_encode_time_us: stats.avg_encode_time_us,
        quality,
        bitrate_kbps,
    }
}

// ── Bridge 8: ASP ↔ Sync (eco-system layer for sync events) ────────────

/// Sync-aware ASP frame descriptor for ALICE-Sync integration.
///
/// Combines ASP packet metadata with sync tick information so the
/// Eco-System pipeline can schedule frame delivery relative to the
/// synchronization clock.
pub struct AspSyncFrame {
    /// ASP packet sequence number.
    pub sequence: u32,
    /// Packet type.
    pub packet_type: PacketType,
    /// Payload size in bytes.
    pub payload_bytes: u32,
    /// Is this a keyframe?
    pub is_keyframe: bool,
    /// Sync tick (from `InputFrame`'s tick counter, or 0 if unavailable).
    pub sync_tick: u64,
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Priority (same mapping as CDN: 3=I, 2=C, 1=D, 0=S).
    pub priority: u8,
}

/// Build a sync-aware ASP frame descriptor for the Eco-System sync pipeline.
///
/// `sync_tick` is the current ALICE-Sync clock tick at time of packet creation.
/// This bridges libasp's internal `sync_bridge` to the Eco-System layer.
#[inline]
#[must_use]
pub fn asp_to_sync_frame(packet: &AspPacket, sync_tick: u64) -> AspSyncFrame {
    const PRIORITY: [u8; 5] = [0, 3, 1, 2, 0];
    let type_idx = (packet.header.packet_type as u8) as usize;
    let priority = PRIORITY[type_idx.min(4)];

    let is_keyframe = packet.header.packet_type == PacketType::IPacket;

    let seq_bytes = packet.header.sequence.to_le_bytes();
    let tick_bytes = sync_tick.to_le_bytes();
    let mut buf = [0u8; 12];
    buf[0..4].copy_from_slice(&seq_bytes);
    buf[4..12].copy_from_slice(&tick_bytes);
    let content_hash = crate::hash::fnv1a(&buf);

    AspSyncFrame {
        sequence: packet.header.sequence,
        packet_type: packet.header.packet_type,
        payload_bytes: packet.header.payload_length,
        is_keyframe,
        sync_tick,
        content_hash,
        priority,
    }
}

// ── Bridge 9: ASP ↔ Voice (audio channel multiplexing) ─────────────────

/// Voice channel descriptor for ASP audio multiplexing.
///
/// Maps ALICE-Voice parametric voice data into ASP's control channel.
/// The Eco-System layer uses this to multiplex voice alongside video
/// in a single ASP stream.
pub struct AspVoiceChannel {
    /// Voice channel identifier (FNV-1a of speaker + sequence).
    pub channel_hash: u64,
    /// Speaker embedding hash (identifies the voice source).
    pub speaker_hash: u64,
    /// Frame sequence number in the voice stream.
    pub voice_sequence: u32,
    /// Voice payload size in bytes (LPC coefficients + metadata).
    pub payload_bytes: u32,
    /// Is this a voice keyframe (full LPC parameters)?
    pub is_keyframe: bool,
    /// Voice activity detected.
    pub is_voiced: bool,
    /// Confidence of voice activity detection [0.0, 1.0].
    pub vad_confidence: f32,
    /// Sample rate in Hz (8000, 16000, 32000, 48000).
    pub sample_rate: u32,
}

/// Build a voice channel descriptor for ASP audio multiplexing.
///
/// `speaker_hash` identifies the voice source (e.g., FNV-1a of speaker embedding).
/// `voice_sequence` is the monotonic frame counter from the voice encoder.
/// `payload_bytes` is the encoded LPC frame size.
#[allow(clippy::too_many_arguments)]
#[inline]
#[must_use]
pub fn asp_to_voice_channel(
    asp_sequence: u32,
    speaker_hash: u64,
    voice_sequence: u32,
    payload_bytes: u32,
    is_keyframe: bool,
    is_voiced: bool,
    vad_confidence: f32,
    sample_rate: u32,
) -> AspVoiceChannel {
    let mut buf = [0u8; 12];
    buf[0..8].copy_from_slice(&speaker_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&asp_sequence.to_le_bytes());
    let channel_hash = crate::hash::fnv1a(&buf);

    AspVoiceChannel {
        channel_hash,
        speaker_hash,
        voice_sequence,
        payload_bytes,
        is_keyframe,
        is_voiced,
        vad_confidence,
        sample_rate,
    }
}

// ── Bridge 10: ASP → Queue (backpressure / priority routing) ───────────

/// Queue message descriptor for ASP packet routing via ALICE-Queue.
///
/// Maps ASP packet priority and type to queue message flags for
/// backpressure-aware delivery.
pub struct AspQueueMessage {
    /// Message identifier (FNV-1a of sequence + type).
    pub message_id: u64,
    /// Queue priority: 3 = I-Packet (highest), 0 = S-Packet (lowest).
    pub priority: u8,
    /// Packet type.
    pub packet_type: PacketType,
    /// Sequence number for ordering.
    pub sequence: u32,
    /// Total payload size (for queue capacity planning).
    pub payload_bytes: u32,
    /// Should this message require acknowledgment?
    pub requires_ack: bool,
    /// Is this a keyframe? (keyframes should not be dropped under backpressure)
    pub is_undropable: bool,
}

/// Convert an ASP packet to a queue message descriptor for ALICE-Queue.
///
/// I-Packets are marked `is_undropable = true` because losing a keyframe
/// causes decoder desync. Priority mapping uses the same const table as CDN.
#[inline]
#[must_use]
pub fn asp_to_queue_message(packet: &AspPacket) -> AspQueueMessage {
    const PRIORITY: [u8; 5] = [0, 3, 1, 2, 0];
    let type_idx = (packet.header.packet_type as u8) as usize;
    let priority = PRIORITY[type_idx.min(4)];

    let is_keyframe = packet.header.packet_type == PacketType::IPacket;

    let seq_bytes = packet.header.sequence.to_le_bytes();
    let type_byte = packet.header.packet_type as u8;
    let mut buf = [0u8; 5];
    buf[0..4].copy_from_slice(&seq_bytes);
    buf[4] = type_byte;
    let message_id = crate::hash::fnv1a(&buf);

    AspQueueMessage {
        message_id,
        priority,
        packet_type: packet.header.packet_type,
        sequence: packet.header.sequence,
        payload_bytes: packet.header.payload_length,
        requires_ack: is_keyframe,
        is_undropable: is_keyframe,
    }
}

// ── Bridge 11: ASP ↔ Auth (stream session access control) ──────────────

/// Stream authentication token for ALICE-Auth integration.
///
/// Binds an ALICE-Auth identity to a streaming session. The `session_token`
/// is derived from the identity hash + stream fingerprint, allowing
/// stateless session validation.
pub struct AspAuthStreamToken {
    /// Identity fingerprint (FNV-1a of the 32-byte `AliceId` public key).
    pub identity_hash: u64,
    /// Stream session fingerprint (FNV-1a of stats counters).
    pub session_hash: u64,
    /// Combined token (`identity_hash` XOR `session_hash`, rotated).
    pub token: u64,
    /// Quality level the identity is authorized for.
    pub authorized_quality: QualityLevel,
    /// Is this a producer (encoder) or consumer (decoder) session?
    pub is_producer: bool,
}

/// Create a stream authentication token from an ALICE-Auth identity hash
/// and ASP stream stats.
///
/// `identity_bytes` is the 32-byte Ed25519 public key from `AliceId`.
/// The token combines identity and session hashes for stateless validation.
#[inline]
#[must_use]
pub fn asp_auth_stream_token(
    identity_bytes: &[u8; 32],
    stats: &StreamStats,
    authorized_quality: QualityLevel,
    is_producer: bool,
) -> AspAuthStreamToken {
    let identity_hash = crate::hash::fnv1a(identity_bytes);

    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&stats.total_bytes.to_le_bytes());
    buf[8..16].copy_from_slice(&stats.total_packets.to_le_bytes());
    buf[16..24].copy_from_slice(&stats.frames_encoded.to_le_bytes());
    let session_hash = crate::hash::fnv1a(&buf);

    // Rotate session_hash by 17 bits before XOR to reduce collision probability
    let rotated = session_hash.rotate_left(17);
    let token = identity_hash ^ rotated;

    AspAuthStreamToken {
        identity_hash,
        session_hash,
        token,
        authorized_quality,
        is_producer,
    }
}

// ── Bridge 12: ASP → ML (adaptive bitrate model features) ──────────────

/// Feature vector for ML-based adaptive bitrate prediction.
///
/// Extracts streaming metrics from ASP `StreamStats` as a fixed-size f32
/// feature vector suitable for ALICE-ML ternary neural inference.
/// Features are normalized to roughly [0, 1] range for stable inference.
pub struct AspMlBitrateFeatures {
    /// Feature vector (8 dimensions).
    pub features: [f32; 8],
    /// Feature names (for debugging / logging).
    pub names: [&'static str; 8],
}

/// Extract ML feature vector from ASP stream stats for adaptive bitrate prediction.
///
/// Features are normalized:
/// - Packet ratios: naturally [0, 1]
/// - Bitrate: divided by `10_000` kbps (10 Mbps baseline)
/// - Compression ratio: divided by 1000 (practical max)
/// - Encode time: divided by `10_000` µs (10 ms baseline)
///
/// Uses reciprocal multiplication for all normalizations.
#[inline]
#[must_use]
pub fn asp_to_ml_bitrate_features(stats: &StreamStats) -> AspMlBitrateFeatures {
    let rcp_packets = if stats.total_packets > 0 {
        1.0 / stats.total_packets as f32
    } else {
        0.0
    };

    // Feature 0: Keyframe ratio (i_packets / total)
    let f0 = stats.i_packets as f32 * rcp_packets;
    // Feature 1: D-Packet ratio
    let f1 = stats.d_packets as f32 * rcp_packets;
    // Feature 2: C-Packet ratio
    let f2 = stats.c_packets as f32 * rcp_packets;
    // Feature 3: S-Packet ratio
    let f3 = stats.s_packets as f32 * rcp_packets;
    // Feature 4: Normalized bitrate (kbps / 10000)
    let bitrate_kbps = stats.avg_bits_per_frame as f32 * 30.0 * 0.001;
    let f4 = bitrate_kbps * 0.0001; // ÷ 10000
                                    // Feature 5: Normalized compression ratio (/ 1000)
    let f5 = stats.compression_ratio as f32 * 0.001;
    // Feature 6: Normalized encode time (µs / 10000)
    let f6 = stats.avg_encode_time_us as f32 * 0.0001;
    // Feature 7: Frames per packet (data density indicator)
    let f7 = if stats.total_packets > 0 {
        stats.frames_encoded as f32 * rcp_packets
    } else {
        0.0
    };

    AspMlBitrateFeatures {
        features: [f0, f1, f2, f3, f4, f5, f6, f7],
        names: [
            "keyframe_ratio",
            "d_packet_ratio",
            "c_packet_ratio",
            "s_packet_ratio",
            "norm_bitrate",
            "norm_compression",
            "norm_encode_time",
            "frames_per_packet",
        ],
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use libasp::{
        AspPacket, Color, DPacketPayload, IPacketPayload, PacketType, Rect, RegionDescriptor,
        StreamStats,
    };

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

    // ── Bridge 7 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_db_session_record() {
        let mut stats = StreamStats::new();
        stats.total_bytes = 5_000_000;
        stats.total_packets = 1000;
        stats.i_packets = 50;
        stats.frames_encoded = 1000;
        stats.compression_ratio = 200.0;
        stats.avg_encode_time_us = 500.0;
        stats.avg_bits_per_frame = 40_000.0;

        let record = asp_to_db_session_record(&stats, QualityLevel::High);

        assert_ne!(record.session_hash, 0);
        assert_ne!(record.channel_id, 0);
        assert_ne!(record.session_hash, record.channel_id);
        assert_eq!(record.total_bytes, 5_000_000);
        assert_eq!(record.total_packets, 1000);
        assert_eq!(record.i_packets, 50);
        assert!((record.compression_ratio - 200.0).abs() < 1e-9);
        assert_eq!(record.quality, QualityLevel::High);
        // bitrate_kbps = 40000 * 30 * 0.001 = 1200
        assert!((record.bitrate_kbps - 1200.0).abs() < 1e-6);
    }

    // ── Bridge 8 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_sync_frame() {
        let payload = IPacketPayload::new(1920, 1080, 30.0);
        let packet = AspPacket::create_i_packet(99, payload).unwrap();

        let frame = asp_to_sync_frame(&packet, 42_000);

        assert_eq!(frame.sequence, 99);
        assert_eq!(frame.packet_type, PacketType::IPacket);
        assert!(frame.is_keyframe);
        assert_eq!(frame.sync_tick, 42_000);
        assert_eq!(frame.priority, 3);
        assert_ne!(frame.content_hash, 0);

        // D-Packet should get priority 1
        let d_payload = DPacketPayload::new(99);
        let d_packet = AspPacket::create_d_packet(100, d_payload).unwrap();
        let d_frame = asp_to_sync_frame(&d_packet, 42_001);
        assert!(!d_frame.is_keyframe);
        assert_eq!(d_frame.priority, 1);

        // Different sync ticks → different hashes
        assert_ne!(frame.content_hash, d_frame.content_hash);
    }

    // ── Bridge 9 test ─────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_voice_channel() {
        let speaker_hash: u64 = 0xDEADBEEF_CAFEBABE;
        let ch = asp_to_voice_channel(10, speaker_hash, 500, 128, true, true, 0.95, 16000);

        assert_ne!(ch.channel_hash, 0);
        assert_eq!(ch.speaker_hash, speaker_hash);
        assert_eq!(ch.voice_sequence, 500);
        assert_eq!(ch.payload_bytes, 128);
        assert!(ch.is_keyframe);
        assert!(ch.is_voiced);
        assert!((ch.vad_confidence - 0.95).abs() < 1e-6);
        assert_eq!(ch.sample_rate, 16000);

        // Different speaker → different channel hash
        let ch2 =
            asp_to_voice_channel(10, 0x1234_5678_9ABC_DEF0, 500, 128, true, true, 0.95, 16000);
        assert_ne!(ch.channel_hash, ch2.channel_hash);
    }

    // ── Bridge 10 test ────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_queue_message() {
        // I-Packet: priority 3, undropable, requires ack
        let i_payload = IPacketPayload::new(1920, 1080, 30.0);
        let i_packet = AspPacket::create_i_packet(1, i_payload).unwrap();
        let q_i = asp_to_queue_message(&i_packet);
        assert_eq!(q_i.priority, 3);
        assert!(q_i.is_undropable);
        assert!(q_i.requires_ack);
        assert_eq!(q_i.sequence, 1);
        assert_ne!(q_i.message_id, 0);

        // D-Packet: priority 1, dropable
        let d_payload = DPacketPayload::new(1);
        let d_packet = AspPacket::create_d_packet(2, d_payload).unwrap();
        let q_d = asp_to_queue_message(&d_packet);
        assert_eq!(q_d.priority, 1);
        assert!(!q_d.is_undropable);
        assert!(!q_d.requires_ack);

        // Different packets → different message IDs
        assert_ne!(q_i.message_id, q_d.message_id);
    }

    // ── Bridge 11 test ────────────────────────────────────────────────────

    #[test]
    fn test_asp_auth_stream_token() {
        let identity: [u8; 32] = [0xAA; 32];
        let mut stats = StreamStats::new();
        stats.total_bytes = 1_000_000;
        stats.total_packets = 500;
        stats.frames_encoded = 500;

        let token = asp_auth_stream_token(&identity, &stats, QualityLevel::Ultra, true);

        assert_ne!(token.identity_hash, 0);
        assert_ne!(token.session_hash, 0);
        assert_ne!(token.token, 0);
        assert_eq!(token.authorized_quality, QualityLevel::Ultra);
        assert!(token.is_producer);

        // Different identity → different token
        let identity2: [u8; 32] = [0xBB; 32];
        let token2 = asp_auth_stream_token(&identity2, &stats, QualityLevel::Ultra, true);
        assert_ne!(token.identity_hash, token2.identity_hash);
        assert_ne!(token.token, token2.token);

        // Consumer flag
        let consumer = asp_auth_stream_token(&identity, &stats, QualityLevel::Low, false);
        assert!(!consumer.is_producer);
        assert_eq!(consumer.authorized_quality, QualityLevel::Low);
    }

    // ── Bridge 12 test ────────────────────────────────────────────────────

    #[test]
    fn test_asp_to_ml_bitrate_features() {
        let mut stats = StreamStats::new();
        stats.total_packets = 1000;
        stats.i_packets = 50;
        stats.d_packets = 900;
        stats.c_packets = 40;
        stats.s_packets = 10;
        stats.avg_bits_per_frame = 40_000.0; // → 1200 kbps
        stats.compression_ratio = 200.0;
        stats.avg_encode_time_us = 500.0;
        stats.frames_encoded = 1000;

        let feat = asp_to_ml_bitrate_features(&stats);

        // Feature 0: keyframe_ratio = 50/1000 = 0.05
        assert!((feat.features[0] - 0.05).abs() < 1e-4);
        // Feature 1: d_ratio = 900/1000 = 0.9
        assert!((feat.features[1] - 0.9).abs() < 1e-4);
        // Feature 2: c_ratio = 40/1000 = 0.04
        assert!((feat.features[2] - 0.04).abs() < 1e-4);
        // Feature 3: s_ratio = 10/1000 = 0.01
        assert!((feat.features[3] - 0.01).abs() < 1e-4);
        // Feature 4: norm_bitrate = 1200 * 0.0001 = 0.12
        assert!((feat.features[4] - 0.12).abs() < 1e-3);
        // Feature 5: norm_compression = 200 * 0.001 = 0.2
        assert!((feat.features[5] - 0.2).abs() < 1e-4);
        // Feature 6: norm_encode_time = 500 * 0.0001 = 0.05
        assert!((feat.features[6] - 0.05).abs() < 1e-4);
        // Feature 7: frames_per_packet = 1000/1000 = 1.0
        assert!((feat.features[7] - 1.0).abs() < 1e-4);

        // Feature names
        assert_eq!(feat.names[0], "keyframe_ratio");
        assert_eq!(feat.names[7], "frames_per_packet");

        // Empty stats → all zeros
        let empty = StreamStats::new();
        let feat0 = asp_to_ml_bitrate_features(&empty);
        for &f in &feat0.features {
            assert_eq!(f, 0.0);
        }
    }
}
