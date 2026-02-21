//! Cross-domain bridges — ALICE-Bio ↔ ML, Search, Text, Physics
//!
//! 4 bridges connecting molecular biology data to ML feature extraction,
//! protein sequence search indexing, text annotation generation, and
//! physics force field decomposition.

use alice_bio::{AminoAcid, Residue, ProteinSdf, TotalEnergy, radius_of_gyration};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Vec<Residue> → ML (feature extraction) ──────────────────

/// ML feature vector extracted from a protein residue chain.
///
/// Encodes per-residue backbone angles and global structural metrics
/// (mean phi/psi, radius of gyration) so the ML layer can train
/// on protein fold prediction without accessing raw molecular data.
pub struct BioMlFeatures {
    /// FNV-1a hash over residue_count, mean_phi, mean_psi, rog bytes.
    pub content_hash: u64,
    /// Number of residues in the input chain.
    pub residue_count: usize,
    /// Total feature dimensions: residue_count * 4 (amino_type, phi, psi, omega per residue).
    pub feature_count: usize,
    /// Mean phi backbone angle across all residues (radians).
    pub mean_phi: f64,
    /// Mean psi backbone angle across all residues (radians).
    pub mean_psi: f64,
    /// Radius of gyration of the protein (angstroms).
    pub radius_of_gyration: f64,
}

/// Convert a residue chain into ML feature metadata.
///
/// Requires a ProteinSdf to obtain Ca positions for radius of gyration.
/// Mean phi/psi are computed directly from the residue backbone angles.
#[inline]
pub fn bio_residues_to_ml_features(protein: &ProteinSdf) -> BioMlFeatures {
    let residues = protein.residues();
    let residue_count = residues.len();
    let feature_count = residue_count * 4;

    let (mean_phi, mean_psi) = if residue_count > 0 {
        let sum_phi: f64 = residues.iter().map(|r| r.phi).sum();
        let sum_psi: f64 = residues.iter().map(|r| r.psi).sum();
        let n = residue_count as f64;
        (sum_phi / n, sum_psi / n)
    } else {
        (0.0, 0.0)
    };

    let rog = radius_of_gyration(protein.positions());

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&(residue_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&mean_phi.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&mean_psi.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&rog.to_bits().to_le_bytes());

    BioMlFeatures {
        content_hash: fnv1a(&key),
        residue_count,
        feature_count,
        mean_phi,
        mean_psi,
        radius_of_gyration: rog,
    }
}

// ── Bridge 2: ProteinSdf → Search (sequence indexing) ──────────────────

/// Protein sequence index record for ALICE-Search.
///
/// Encodes the one-letter amino acid sequence as an FNV-1a hash and a
/// per-amino-acid histogram so the Search layer can locate proteins by
/// composition or sequence substring without storing raw residue data.
pub struct BioSearchSequence {
    /// FNV-1a hash over sequence_length + histogram bytes.
    pub content_hash: u64,
    /// Number of residues in the sequence.
    pub sequence_length: usize,
    /// FNV-1a hash of the one-letter amino acid sequence string.
    pub sequence_hash: u64,
    /// Histogram: count of each of the 20 standard amino acids (indexed by AminoAcid::ALL order).
    pub amino_histogram: [u32; 20],
}

/// Convert a protein SDF into a searchable sequence record.
#[inline]
pub fn bio_protein_to_search_sequence(protein: &ProteinSdf) -> BioSearchSequence {
    let residues = protein.residues();
    let sequence_length = residues.len();

    // Build one-letter sequence and hash it
    let one_letters: Vec<u8> = residues.iter().map(|r| r.amino.one_letter() as u8).collect();
    let sequence_hash = fnv1a(&one_letters);

    // Build histogram indexed by position in AminoAcid::ALL
    let mut amino_histogram = [0u32; 20];
    for r in residues {
        for (idx, &aa) in AminoAcid::ALL.iter().enumerate() {
            if r.amino == aa {
                amino_histogram[idx] += 1;
                break;
            }
        }
    }

    // Content hash over sequence_length + histogram bytes
    let mut key = [0u8; 88]; // 8 + 20*4 = 88
    key[0..8].copy_from_slice(&(sequence_length as u64).to_le_bytes());
    for (i, &count) in amino_histogram.iter().enumerate() {
        key[8 + i * 4..8 + (i + 1) * 4].copy_from_slice(&count.to_le_bytes());
    }

    BioSearchSequence {
        content_hash: fnv1a(&key),
        sequence_length,
        sequence_hash,
        amino_histogram,
    }
}

// ── Bridge 3: ProteinSdf → Text (annotation record) ───────────────────

/// Text annotation metadata for a protein structure.
///
/// Provides the Text layer with estimated compressed annotation size and
/// sequence identity so annotations can be cached and deduplicated
/// without storing the full residue chain.
pub struct BioTextAnnotation {
    /// FNV-1a hash over residue_count, annotation_bytes, sequence_hash bytes.
    pub content_hash: u64,
    /// Number of residues in the protein.
    pub residue_count: usize,
    /// Estimated compressed text size in bytes (one-letter code + separator per residue).
    pub annotation_bytes: usize,
    /// FNV-1a hash of the one-letter amino acid sequence.
    pub sequence_hash: u64,
}

/// Convert a protein SDF into a text annotation record.
#[inline]
pub fn bio_protein_to_text_annotation(protein: &ProteinSdf) -> BioTextAnnotation {
    let residues = protein.residues();
    let residue_count = residues.len();
    let annotation_bytes = residue_count * 2; // one-letter code + separator

    let one_letters: Vec<u8> = residues.iter().map(|r| r.amino.one_letter() as u8).collect();
    let sequence_hash = fnv1a(&one_letters);

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&(residue_count as u64).to_le_bytes());
    key[8..16].copy_from_slice(&(annotation_bytes as u64).to_le_bytes());
    key[16..24].copy_from_slice(&sequence_hash.to_le_bytes());

    BioTextAnnotation {
        content_hash: fnv1a(&key),
        residue_count,
        annotation_bytes,
        sequence_hash,
    }
}

// ── Bridge 4: TotalEnergy → Physics (force decomposition) ─────────────

/// Physics force decomposition derived from molecular total energy.
///
/// Maps molecular energy components (Lennard-Jones, Coulomb, H-bond,
/// torsion) to a physics-friendly force record so the Physics layer
/// can integrate molecular dynamics without parsing energy internals.
pub struct BioPhysicsForce {
    /// FNV-1a hash over total_energy, lj, coulomb, is_stable, magnitude bytes.
    pub content_hash: u64,
    /// Sum of all energy components (kcal/mol).
    pub total_energy: f64,
    /// Van der Waals (Lennard-Jones) energy component.
    pub lj_component: f64,
    /// Electrostatic (Coulomb) energy component.
    pub coulomb_component: f64,
    /// True if the total energy is negative (thermodynamically stable).
    pub is_stable: bool,
    /// Absolute value of the total energy.
    pub energy_magnitude: f64,
}

/// Convert a total energy report into a physics force record.
#[inline]
pub fn bio_energy_to_physics_force(energy: &TotalEnergy) -> BioPhysicsForce {
    let total = energy.total();
    let is_stable = total < 0.0;
    let magnitude = total.abs();

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&total.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&energy.van_der_waals.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&energy.electrostatic.to_bits().to_le_bytes());
    key[24] = is_stable as u8;
    key[25..33].copy_from_slice(&magnitude.to_bits().to_le_bytes());

    BioPhysicsForce {
        content_hash: fnv1a(&key),
        total_energy: total,
        lj_component: energy.van_der_waals,
        coulomb_component: energy.electrostatic,
        is_stable,
        energy_magnitude: magnitude,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_bio::{AminoAcid, Residue, ProteinSdf, TotalEnergy};
    use std::f64::consts::PI;

    fn make_residue(aa: AminoAcid, phi: f64, psi: f64) -> Residue {
        Residue::new(aa, phi, psi, PI)
    }

    fn make_protein() -> ProteinSdf {
        let residues = vec![
            make_residue(AminoAcid::Ala, -1.0, -0.8),
            make_residue(AminoAcid::Gly, -1.1, -0.7),
            make_residue(AminoAcid::Val, -1.2, -0.9),
        ];
        ProteinSdf::new(residues)
    }

    // ── Bridge 1: residues → ML features ────────────────────────────────

    #[test]
    fn test_bio_residues_to_ml_features() {
        let protein = make_protein();
        let features = bio_residues_to_ml_features(&protein);
        assert_ne!(features.content_hash, 0);
        assert_eq!(features.residue_count, 3);
        assert_eq!(features.feature_count, 12); // 3 * 4
        // Mean phi should be approximately (-1.0 + -1.1 + -1.2) / 3 = -1.1
        assert!((features.mean_phi - (-1.1)).abs() < 1e-10);
        // Mean psi should be approximately (-0.8 + -0.7 + -0.9) / 3 = -0.8
        assert!((features.mean_psi - (-0.8)).abs() < 1e-10);
        assert!(features.radius_of_gyration >= 0.0);
    }

    #[test]
    fn test_bio_residues_to_ml_features_deterministic() {
        let protein = make_protein();
        let f1 = bio_residues_to_ml_features(&protein);
        let f2 = bio_residues_to_ml_features(&protein);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    #[test]
    fn test_bio_residues_to_ml_features_empty() {
        let protein = ProteinSdf::new(vec![]);
        let features = bio_residues_to_ml_features(&protein);
        assert_eq!(features.residue_count, 0);
        assert_eq!(features.feature_count, 0);
        assert!((features.mean_phi - 0.0).abs() < 1e-10);
        assert!((features.mean_psi - 0.0).abs() < 1e-10);
    }

    // ── Bridge 2: protein → search sequence ─────────────────────────────

    #[test]
    fn test_bio_protein_to_search_sequence() {
        let protein = make_protein();
        let seq = bio_protein_to_search_sequence(&protein);
        assert_ne!(seq.content_hash, 0);
        assert_eq!(seq.sequence_length, 3);
        assert_ne!(seq.sequence_hash, 0);
        // Check histogram: 1 Ala, 1 Gly, 1 Val
        let ala_idx = AminoAcid::ALL.iter().position(|&a| a == AminoAcid::Ala).unwrap();
        let gly_idx = AminoAcid::ALL.iter().position(|&a| a == AminoAcid::Gly).unwrap();
        let val_idx = AminoAcid::ALL.iter().position(|&a| a == AminoAcid::Val).unwrap();
        assert_eq!(seq.amino_histogram[ala_idx], 1);
        assert_eq!(seq.amino_histogram[gly_idx], 1);
        assert_eq!(seq.amino_histogram[val_idx], 1);
        // All other entries should be 0
        let total: u32 = seq.amino_histogram.iter().sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn test_bio_protein_to_search_sequence_deterministic() {
        let protein = make_protein();
        let s1 = bio_protein_to_search_sequence(&protein);
        let s2 = bio_protein_to_search_sequence(&protein);
        assert_eq!(s1.content_hash, s2.content_hash);
        assert_eq!(s1.sequence_hash, s2.sequence_hash);
    }

    #[test]
    fn test_bio_protein_to_search_sequence_empty() {
        let protein = ProteinSdf::new(vec![]);
        let seq = bio_protein_to_search_sequence(&protein);
        assert_eq!(seq.sequence_length, 0);
        assert_eq!(seq.amino_histogram, [0u32; 20]);
    }

    // ── Bridge 3: protein → text annotation ─────────────────────────────

    #[test]
    fn test_bio_protein_to_text_annotation() {
        let protein = make_protein();
        let ann = bio_protein_to_text_annotation(&protein);
        assert_ne!(ann.content_hash, 0);
        assert_eq!(ann.residue_count, 3);
        assert_eq!(ann.annotation_bytes, 6); // 3 * 2
        assert_ne!(ann.sequence_hash, 0);
    }

    #[test]
    fn test_bio_protein_to_text_annotation_deterministic() {
        let protein = make_protein();
        let a1 = bio_protein_to_text_annotation(&protein);
        let a2 = bio_protein_to_text_annotation(&protein);
        assert_eq!(a1.content_hash, a2.content_hash);
        assert_eq!(a1.sequence_hash, a2.sequence_hash);
    }

    #[test]
    fn test_bio_protein_to_text_annotation_empty() {
        let protein = ProteinSdf::new(vec![]);
        let ann = bio_protein_to_text_annotation(&protein);
        assert_eq!(ann.residue_count, 0);
        assert_eq!(ann.annotation_bytes, 0);
    }

    // ── Bridge 4: energy → physics force ────────────────────────────────

    #[test]
    fn test_bio_energy_to_physics_force_stable() {
        let energy = TotalEnergy {
            van_der_waals: -5.0,
            electrostatic: -2.0,
            hydrogen_bonds: -1.5,
            torsional: 0.3,
        };
        let force = bio_energy_to_physics_force(&energy);
        assert_ne!(force.content_hash, 0);
        assert!((force.total_energy - (-8.2)).abs() < 1e-10);
        assert!((force.lj_component - (-5.0)).abs() < 1e-10);
        assert!((force.coulomb_component - (-2.0)).abs() < 1e-10);
        assert!(force.is_stable); // total < 0
        assert!((force.energy_magnitude - 8.2).abs() < 1e-10);
    }

    #[test]
    fn test_bio_energy_to_physics_force_unstable() {
        let energy = TotalEnergy {
            van_der_waals: 10.0,
            electrostatic: 5.0,
            hydrogen_bonds: 0.0,
            torsional: 3.0,
        };
        let force = bio_energy_to_physics_force(&energy);
        assert!(!force.is_stable); // total > 0
        assert!((force.total_energy - 18.0).abs() < 1e-10);
        assert!((force.energy_magnitude - 18.0).abs() < 1e-10);
    }

    #[test]
    fn test_bio_energy_to_physics_force_deterministic() {
        let energy = TotalEnergy {
            van_der_waals: -3.0,
            electrostatic: -1.0,
            hydrogen_bonds: -0.5,
            torsional: 0.1,
        };
        let f1 = bio_energy_to_physics_force(&energy);
        let f2 = bio_energy_to_physics_force(&energy);
        assert_eq!(f1.content_hash, f2.content_hash);
    }
}
