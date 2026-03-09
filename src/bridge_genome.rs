//! Genome bridges — ALICE-Genome ↔ DB, Cache, Analytics, Bio, Search
//!
//! 5 bridges connecting genomic sequence analysis to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Genome → DB (assembly record) ───────────────────────────────

/// Genome assembly record for ALICE-DB persistence.
pub struct GenomeDbRecord {
    /// Content hash over the assembly snapshot.
    pub content_hash: u64,
    /// Total length of the assembled sequence in base pairs.
    pub sequence_length: u64,
    /// Number of annotated genes.
    pub gene_count: u32,
    /// Hash of the species taxonomy identifier.
    pub species_hash: u64,
    /// Assembly version number.
    pub assembly_version: u32,
    /// GC content in basis points (0–10 000).
    pub gc_content_bps: u16,
}

/// Serialize a genome assembly for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn genome_to_db_record(
    sequence_length: u64,
    gene_count: u32,
    species_hash: u64,
    assembly_version: u32,
    gc_content_bps: u16,
) -> GenomeDbRecord {
    let mut buf = [0u8; 26];
    buf[0..8].copy_from_slice(&sequence_length.to_le_bytes());
    buf[8..12].copy_from_slice(&gene_count.to_le_bytes());
    buf[12..20].copy_from_slice(&species_hash.to_le_bytes());
    buf[20..24].copy_from_slice(&assembly_version.to_le_bytes());
    buf[24..26].copy_from_slice(&gc_content_bps.to_le_bytes());
    GenomeDbRecord {
        content_hash: fnv1a(&buf),
        sequence_length,
        gene_count,
        species_hash,
        assembly_version,
        gc_content_bps,
    }
}

// ── Bridge 2: Genome → Cache (sequence region cache) ─────────────────────

/// Sequence region cache entry for ALICE-Cache.
pub struct GenomeCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Length of the cached sequence region in base pairs.
    pub sequence_length: u64,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Byte size of the compressed sequence.
    pub compressed_bytes: u64,
    /// Hash of the genomic region (chromosome + coordinates).
    pub region_hash: u64,
}

/// Build a sequence region cache entry for ALICE-Cache.
///
/// Short regions receive a longer TTL (3600 s vs 300 s) because they are
/// frequently accessed for variant calling and alignment lookups.
#[inline]
#[must_use]
pub fn genome_to_cache_entry(
    sequence_length: u64,
    compressed_bytes: u64,
    region_hash: u64,
) -> GenomeCacheEntry {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&sequence_length.to_le_bytes());
    buf[8..16].copy_from_slice(&compressed_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&region_hash.to_le_bytes());
    let long_region = (sequence_length > 1_000_000) as u32;
    let ttl_secs = 3_600 - long_region * 3_300;
    GenomeCacheEntry {
        content_hash: fnv1a(&buf),
        sequence_length,
        ttl_secs,
        compressed_bytes,
        region_hash,
    }
}

// ── Bridge 3: Genome → Analytics (alignment event) ───────────────────────

/// Alignment analytics event for ALICE-Analytics ingestion.
pub struct GenomeAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Alignment score (Smith–Waterman or similar).
    pub alignment_score: u32,
    /// Number of variants detected in the alignment.
    pub variant_count: u64,
    /// Sequencing coverage depth multiplied by 100.
    pub coverage_x100: u32,
    /// Base quality score in basis points (0–10 000).
    pub quality_bps: u16,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an alignment analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn genome_to_analytics_event(
    alignment_score: u32,
    variant_count: u64,
    coverage_x100: u32,
    quality_bps: u16,
    timestamp_ms: u64,
) -> GenomeAnalyticsEvent {
    let mut buf = [0u8; 30];
    buf[0..4].copy_from_slice(&alignment_score.to_le_bytes());
    buf[4..12].copy_from_slice(&variant_count.to_le_bytes());
    buf[12..16].copy_from_slice(&coverage_x100.to_le_bytes());
    buf[16..18].copy_from_slice(&quality_bps.to_le_bytes());
    buf[18..26].copy_from_slice(&timestamp_ms.to_le_bytes());
    GenomeAnalyticsEvent {
        content_hash: fnv1a(&buf[..26]),
        alignment_score,
        variant_count,
        coverage_x100,
        quality_bps,
        timestamp_ms,
    }
}

// ── Bridge 4: Genome → Bio (gene–protein link) ────────────────────────────

/// Gene–protein pathway link for ALICE-Bio integration.
pub struct GenomeBioLink {
    /// Content hash over the link descriptor.
    pub content_hash: u64,
    /// Number of genes in the pathway.
    pub gene_count: u32,
    /// Number of encoded proteins.
    pub protein_count: u32,
    /// Number of metabolic pathways referenced.
    pub pathway_count: u16,
    /// Hash of the organism identifier (e.g. NCBI taxonomy ID).
    pub organism_hash: u64,
}

/// Build a gene–protein pathway link for ALICE-Bio.
#[inline]
#[must_use]
pub fn genome_to_bio_link(
    gene_count: u32,
    protein_count: u32,
    pathway_count: u16,
    organism_hash: u64,
) -> GenomeBioLink {
    let mut buf = [0u8; 18];
    buf[0..4].copy_from_slice(&gene_count.to_le_bytes());
    buf[4..8].copy_from_slice(&protein_count.to_le_bytes());
    buf[8..10].copy_from_slice(&pathway_count.to_le_bytes());
    buf[10..18].copy_from_slice(&organism_hash.to_le_bytes());
    GenomeBioLink {
        content_hash: fnv1a(&buf),
        gene_count,
        protein_count,
        pathway_count,
        organism_hash,
    }
}

// ── Bridge 5: Genome → Search (k-mer index) ──────────────────────────────

/// K-mer index entry for ALICE-Search integration.
pub struct GenomeSearchIndex {
    /// Content hash over the index snapshot.
    pub content_hash: u64,
    /// Total sequence length covered by the index.
    pub sequence_length: u64,
    /// Length of each k-mer (e.g. 21 for typical WGS).
    pub kmer_size: u8,
    /// Total number of distinct k-mer entries.
    pub index_entries: u64,
    /// Shard identifier for distributed search.
    pub shard_id: u32,
}

/// Build a k-mer index entry for ALICE-Search.
#[inline]
#[must_use]
pub fn genome_to_search_index(
    sequence_length: u64,
    kmer_size: u8,
    index_entries: u64,
    shard_id: u32,
) -> GenomeSearchIndex {
    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&sequence_length.to_le_bytes());
    buf[8] = kmer_size;
    buf[9..17].copy_from_slice(&index_entries.to_le_bytes());
    buf[17..21].copy_from_slice(&shard_id.to_le_bytes());
    GenomeSearchIndex {
        content_hash: fnv1a(&buf),
        sequence_length,
        kmer_size,
        index_entries,
        shard_id,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genome_db_record_hash_nonzero() {
        let rec = genome_to_db_record(3_000_000_000, 20_000, 0x686f_6d6f, 38, 4_100);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_genome_db_record_fields() {
        let rec = genome_to_db_record(4_600_000_000, 29_000, 0x6d75_736d, 39, 4_200);
        assert_eq!(rec.sequence_length, 4_600_000_000);
        assert_eq!(rec.gene_count, 29_000);
        assert_eq!(rec.gc_content_bps, 4_200);
    }

    #[test]
    fn test_genome_db_record_determinism() {
        let a = genome_to_db_record(3_000_000_000, 20_000, 0x1234, 38, 4_100);
        let b = genome_to_db_record(3_000_000_000, 20_000, 0x1234, 38, 4_100);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_genome_cache_entry_short_region_ttl() {
        let entry = genome_to_cache_entry(500_000, 62_500, 0x7265_6769);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 3_600);
    }

    #[test]
    fn test_genome_cache_entry_long_region_ttl() {
        let entry = genome_to_cache_entry(5_000_000, 625_000, 0x7265_6769);
        assert_eq!(entry.ttl_secs, 300);
        assert_eq!(entry.sequence_length, 5_000_000);
    }

    #[test]
    fn test_genome_analytics_event() {
        let ev = genome_to_analytics_event(240, 1_200, 3_000, 9_200, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.variant_count, 1_200);
        assert_eq!(ev.quality_bps, 9_200);
    }

    #[test]
    fn test_genome_bio_link() {
        let l = genome_to_bio_link(20_000, 19_000, 2_500, 0x9606_0000);
        assert_ne!(l.content_hash, 0);
        assert_eq!(l.gene_count, 20_000);
        assert_eq!(l.pathway_count, 2_500);
    }

    #[test]
    fn test_genome_search_index() {
        let idx = genome_to_search_index(3_000_000_000, 21, 2_979_000_000, 0);
        assert_ne!(idx.content_hash, 0);
        assert_eq!(idx.kmer_size, 21);
        assert_eq!(idx.shard_id, 0);
    }

    #[test]
    fn test_genome_search_index_determinism() {
        let a = genome_to_search_index(1_000_000, 31, 970_000, 1);
        let b = genome_to_search_index(1_000_000, 31, 970_000, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
