//! Cross-domain bridges — ALICE-History ↔ Text, Search, Codec, Crypto
//!
//! 4 bridges connecting inverse entropy restoration data to text compression,
//! search indexing, codec frame metadata, and cryptographic provenance proofs.

use alice_history::{Fragment, RestorationResult};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Fragment(Text) → Text (compression record) ────────────

/// Text compression record derived from a history fragment.
///
/// Maps text-kind fragments into ALICE-Text compression metadata so the
/// Text layer can estimate storage and prioritise text-specific encoding
/// without accessing raw fragment data.
pub struct HistoryTextRecord {
    /// FNV-1a hash over fragment_id, kind, data_length, known_fraction bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Fragment kind discriminant: 0=Text, 1=Image, 2=Artifact, 3=Inscription, 4=Audio.
    pub kind: u8,
    /// Number of data elements in the fragment.
    pub data_length: usize,
    /// Fraction of data that is known (0.0 to 1.0).
    pub known_fraction: f64,
    /// Estimated text byte count (data.len() * 4, assuming f64 to UTF-8 expansion).
    pub estimated_text_bytes: usize,
    /// True when the fragment kind is Text (kind == 0).
    pub is_text_fragment: bool,
}

/// Convert a history fragment into a text compression record.
#[inline]
pub fn history_fragment_to_text_record(fragment: &Fragment) -> HistoryTextRecord {
    let kind_byte = fragment.kind as u8;
    let known_frac = fragment.known_fraction();
    let data_length = fragment.data.len();
    let estimated_text_bytes = data_length * 4;
    let is_text_fragment = kind_byte == 0;

    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&fragment.id.to_le_bytes());
    key[8] = kind_byte;
    key[9..17].copy_from_slice(&(data_length as u64).to_le_bytes());
    key[17..25].copy_from_slice(&known_frac.to_bits().to_le_bytes());

    HistoryTextRecord {
        content_hash: fnv1a(&key),
        fragment_id: fragment.id,
        kind: kind_byte,
        data_length,
        known_fraction: known_frac,
        estimated_text_bytes,
        is_text_fragment,
    }
}

// ── Bridge 2: RestorationResult → Search (FM-index record) ──────────

/// Search FM-index record derived from a restoration result.
///
/// Encodes restoration quality into ALICE-Search index metadata so the
/// Search layer can rank and filter results by confidence without
/// accessing the full restoration field.
pub struct HistorySearchIndex {
    /// FNV-1a hash over fragment_id, value_count, mean_confidence, iterations bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Number of restored values.
    pub value_count: usize,
    /// Mean confidence across all restored elements.
    pub mean_confidence: f64,
    /// Number of solver iterations performed.
    pub iterations: u32,
    /// True when mean_confidence exceeds 0.7.
    pub is_searchable: bool,
    /// Priority: 1=high_quality(>0.9), 2=medium(>0.7), 3=low.
    pub index_priority: u8,
}

/// Convert a restoration result into a search FM-index record.
#[inline]
pub fn history_restoration_to_search_index(result: &RestorationResult) -> HistorySearchIndex {
    let mean_conf = result.field.confidence.mean_confidence;
    let value_count = result.field.values.len();
    let iterations = result.field.iterations;
    let is_searchable = mean_conf > 0.7;
    let index_priority = if mean_conf > 0.9 { 1u8 } else if mean_conf > 0.7 { 2 } else { 3 };

    let mut key = [0u8; 28];
    key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    key[8..16].copy_from_slice(&(value_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&mean_conf.to_bits().to_le_bytes());
    key[24..28].copy_from_slice(&iterations.to_le_bytes());

    HistorySearchIndex {
        content_hash: fnv1a(&key),
        fragment_id: result.fragment_id,
        value_count,
        mean_confidence: mean_conf,
        iterations,
        is_searchable,
        index_priority,
    }
}

// ── Bridge 3: Fragment(Image/Audio) → Codec (frame metadata) ────────

/// Codec frame metadata derived from a history fragment.
///
/// Maps media-kind fragments (Image, Audio) into ALICE-Codec frame
/// metadata so the Codec layer can estimate bitrate and choose an
/// appropriate encoder without touching raw fragment data.
pub struct HistoryCodecFrame {
    /// FNV-1a hash over fragment_id, kind, data_length, missing_count, known_fraction bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Fragment kind discriminant.
    pub kind: u8,
    /// Number of data elements in the fragment.
    pub data_length: usize,
    /// Number of missing (unknown) elements.
    pub missing_count: usize,
    /// Fraction of data that is known (0.0 to 1.0).
    pub known_fraction: f64,
    /// True when the fragment kind is Image (1) or Audio (4).
    pub is_media_fragment: bool,
    /// Estimated frame bytes: data.len() * 8 for raw, scaled by known_fraction for compressed.
    pub estimated_frame_bytes: usize,
}

/// Convert a history fragment into codec frame metadata.
#[inline]
pub fn history_fragment_to_codec_frame(fragment: &Fragment) -> HistoryCodecFrame {
    let kind_byte = fragment.kind as u8;
    let data_length = fragment.data.len();
    let missing = fragment.missing_count();
    let known_frac = fragment.known_fraction();
    let is_media_fragment = kind_byte == 1 || kind_byte == 4;
    let raw_bytes = data_length * 8;
    let estimated_frame_bytes = (raw_bytes as f64 * known_frac) as usize;

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&fragment.id.to_le_bytes());
    key[8] = kind_byte;
    key[9..17].copy_from_slice(&(data_length as u64).to_le_bytes());
    key[17..25].copy_from_slice(&(missing as u64).to_le_bytes());
    key[25..33].copy_from_slice(&known_frac.to_bits().to_le_bytes());

    HistoryCodecFrame {
        content_hash: fnv1a(&key),
        fragment_id: fragment.id,
        kind: kind_byte,
        data_length,
        missing_count: missing,
        known_fraction: known_frac,
        is_media_fragment,
        estimated_frame_bytes,
    }
}

// ── Bridge 4: RestorationResult → Crypto (provenance proof) ─────────

/// Cryptographic provenance proof derived from a restoration result.
///
/// Produces an ALICE-Crypto-compatible provenance hash chain so the
/// Crypto layer can verify that a restoration originated from a
/// specific fragment and solver configuration.
pub struct HistoryCryptoProof {
    /// FNV-1a hash over fragment_id, field_hash, iterations, elapsed_ns, mean_confidence bytes.
    pub content_hash: u64,
    /// Fragment identifier.
    pub fragment_id: u64,
    /// Content hash of the restoration field.
    pub field_hash: u64,
    /// Number of solver iterations performed.
    pub iterations: u32,
    /// Wall-clock elapsed nanoseconds.
    pub elapsed_ns: u64,
    /// Mean confidence across all restored elements.
    pub mean_confidence: f64,
    /// Provenance hash: fnv1a of fragment_id + field_hash + iterations.
    pub provenance_hash: u64,
}

/// Convert a restoration result into a cryptographic provenance proof.
#[inline]
pub fn history_restoration_to_crypto_proof(result: &RestorationResult) -> HistoryCryptoProof {
    let field_hash = result.field.content_hash;
    let iterations = result.field.iterations;
    let mean_conf = result.field.confidence.mean_confidence;

    // Compute provenance hash: fnv1a of fragment_id + field_hash + iterations
    let mut prov_key = [0u8; 20];
    prov_key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    prov_key[8..16].copy_from_slice(&field_hash.to_le_bytes());
    prov_key[16..20].copy_from_slice(&iterations.to_le_bytes());
    let provenance_hash = fnv1a(&prov_key);

    // Content hash over all significant fields
    let mut key = [0u8; 36];
    key[0..8].copy_from_slice(&result.fragment_id.to_le_bytes());
    key[8..16].copy_from_slice(&field_hash.to_le_bytes());
    key[16..20].copy_from_slice(&iterations.to_le_bytes());
    key[20..28].copy_from_slice(&result.elapsed_ns.to_le_bytes());
    key[28..36].copy_from_slice(&mean_conf.to_bits().to_le_bytes());

    HistoryCryptoProof {
        content_hash: fnv1a(&key),
        fragment_id: result.fragment_id,
        field_hash,
        iterations,
        elapsed_ns: result.elapsed_ns,
        mean_confidence: mean_conf,
        provenance_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_history::{Fragment, FragmentKind, InversionConfig, restore};

    fn make_fragment() -> Fragment {
        Fragment::new(42, FragmentKind::Text, vec![10.0, 0.0, 30.0], vec![1.0, 0.0, 1.0], 1000)
    }

    fn make_restoration() -> RestorationResult {
        let f = make_fragment();
        let config = InversionConfig::default();
        restore(&f, &config)
    }

    // ── Bridge 1: fragment → text record ──────────────────────────────

    #[test]
    fn test_history_fragment_to_text_record_text_kind() {
        let f = make_fragment();
        let rec = history_fragment_to_text_record(&f);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.fragment_id, 42);
        assert_eq!(rec.kind, 0); // Text
        assert_eq!(rec.data_length, 3);
        assert!(rec.is_text_fragment);
        assert_eq!(rec.estimated_text_bytes, 12); // 3 * 4
        assert!((rec.known_fraction - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_history_fragment_to_text_record_image_kind() {
        let f = Fragment::new(7, FragmentKind::Image, vec![1.0, 2.0], vec![1.0, 1.0], 500);
        let rec = history_fragment_to_text_record(&f);
        assert_eq!(rec.kind, 1); // Image
        assert!(!rec.is_text_fragment);
        assert_eq!(rec.estimated_text_bytes, 8); // 2 * 4
    }

    #[test]
    fn test_history_fragment_to_text_record_deterministic() {
        let f = make_fragment();
        let r1 = history_fragment_to_text_record(&f);
        let r2 = history_fragment_to_text_record(&f);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 2: restoration → search index ──────────────────────────

    #[test]
    fn test_history_restoration_to_search_index() {
        let r = make_restoration();
        let idx = history_restoration_to_search_index(&r);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.fragment_id, 42);
        assert_eq!(idx.value_count, 3);
        assert!(idx.iterations > 0);
        assert!(idx.mean_confidence >= 0.0);
        // Verify priority logic
        if idx.mean_confidence > 0.9 {
            assert_eq!(idx.index_priority, 1);
            assert!(idx.is_searchable);
        } else if idx.mean_confidence > 0.7 {
            assert_eq!(idx.index_priority, 2);
            assert!(idx.is_searchable);
        } else {
            assert_eq!(idx.index_priority, 3);
            assert!(!idx.is_searchable);
        }
    }

    #[test]
    fn test_history_restoration_to_search_index_deterministic() {
        let r = make_restoration();
        let i1 = history_restoration_to_search_index(&r);
        let i2 = history_restoration_to_search_index(&r);
        assert_eq!(i1.content_hash, i2.content_hash);
    }

    // ── Bridge 3: fragment → codec frame ──────────────────────────────

    #[test]
    fn test_history_fragment_to_codec_frame_image() {
        let f = Fragment::new(10, FragmentKind::Image, vec![1.0, 2.0, 3.0, 4.0], vec![1.0, 1.0, 0.0, 1.0], 2000);
        let frame = history_fragment_to_codec_frame(&f);
        assert_ne!(frame.content_hash, 0);
        assert_eq!(frame.fragment_id, 10);
        assert_eq!(frame.kind, 1); // Image
        assert!(frame.is_media_fragment);
        assert_eq!(frame.data_length, 4);
        assert_eq!(frame.missing_count, 1);
        assert!((frame.known_fraction - 0.75).abs() < 1e-10);
        // estimated_frame_bytes = (4 * 8) * 0.75 = 24
        assert_eq!(frame.estimated_frame_bytes, 24);
    }

    #[test]
    fn test_history_fragment_to_codec_frame_audio() {
        let f = Fragment::new(11, FragmentKind::Audio, vec![5.0, 6.0], vec![1.0, 1.0], 3000);
        let frame = history_fragment_to_codec_frame(&f);
        assert_eq!(frame.kind, 4); // Audio
        assert!(frame.is_media_fragment);
        assert_eq!(frame.missing_count, 0);
        assert!((frame.known_fraction - 1.0).abs() < 1e-10);
        assert_eq!(frame.estimated_frame_bytes, 16); // (2 * 8) * 1.0
    }

    #[test]
    fn test_history_fragment_to_codec_frame_text_not_media() {
        let f = make_fragment();
        let frame = history_fragment_to_codec_frame(&f);
        assert!(!frame.is_media_fragment); // Text kind is not media
    }

    #[test]
    fn test_history_fragment_to_codec_frame_deterministic() {
        let f = Fragment::new(10, FragmentKind::Image, vec![1.0, 2.0], vec![1.0, 0.0], 100);
        let f1 = history_fragment_to_codec_frame(&f);
        let f2 = history_fragment_to_codec_frame(&f);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 4: restoration → crypto proof ──────────────────────────

    #[test]
    fn test_history_restoration_to_crypto_proof() {
        let r = make_restoration();
        let proof = history_restoration_to_crypto_proof(&r);
        assert_ne!(proof.content_hash, 0);
        assert_eq!(proof.fragment_id, 42);
        assert_ne!(proof.field_hash, 0);
        assert!(proof.iterations > 0);
        assert!(proof.mean_confidence >= 0.0);
        assert_ne!(proof.provenance_hash, 0);
        // Provenance hash must differ from content hash
        assert_ne!(proof.provenance_hash, proof.content_hash);
    }

    #[test]
    fn test_history_restoration_to_crypto_proof_deterministic() {
        let r = make_restoration();
        let p1 = history_restoration_to_crypto_proof(&r);
        let p2 = history_restoration_to_crypto_proof(&r);
        assert_eq!(p1.content_hash, p2.content_hash);
        assert_eq!(p1.provenance_hash, p2.provenance_hash);
        assert_eq!(p1.field_hash, p2.field_hash);
    }

    #[test]
    fn test_history_restoration_to_crypto_proof_provenance_depends_on_fragment() {
        let f1 = Fragment::new(42, FragmentKind::Text, vec![10.0, 0.0, 30.0], vec![1.0, 0.0, 1.0], 1000);
        let f2 = Fragment::new(99, FragmentKind::Text, vec![10.0, 0.0, 30.0], vec![1.0, 0.0, 1.0], 1000);
        let config = InversionConfig::default();
        let r1 = restore(&f1, &config);
        let r2 = restore(&f2, &config);
        let p1 = history_restoration_to_crypto_proof(&r1);
        let p2 = history_restoration_to_crypto_proof(&r2);
        // Different fragment IDs must produce different provenance hashes
        assert_ne!(p1.provenance_hash, p2.provenance_hash);
    }
}
