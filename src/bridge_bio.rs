//! Bio bridges — ALICE-Bio ↔ SDF, Analytics, DB
//!
//! 5 bridges connecting molecular structure data to the ALICE ecosystem.

use alice_bio::{Residue, ProteinSdf, TotalEnergy};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: ProteinSdf → SDF (molecular SDF evaluation) ──────────────

/// Molecular SDF sample result for ALICE-SDF pipeline ingestion.
pub struct BioSdfSample {
    /// Content hash over position and distance bytes.
    pub content_hash: u64,
    /// Evaluation point in angstroms.
    pub position: [f64; 3],
    /// Signed distance to the nearest atom surface.
    pub distance: f64,
    /// Number of residues in the protein.
    pub residue_count: usize,
}

/// Evaluate a protein SDF at a point and produce an SDF pipeline sample.
#[inline]
pub fn bio_sdf_eval(protein: &ProteinSdf, point: [f64; 3]) -> BioSdfSample {
    let distance = protein.eval(&point);
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&point[0].to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&point[1].to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&point[2].to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&distance.to_bits().to_le_bytes());

    BioSdfSample {
        content_hash: fnv1a(&key),
        position: point,
        distance,
        residue_count: protein.residue_count(),
    }
}

// ── Bridge 2: ProteinSdf → Analytics (bounding box metrics) ────────────

/// Protein bounding box metrics for ALICE-Analytics ingestion.
pub struct BioAnalyticsBoundsEvent {
    /// Content hash over min/max bounding box bytes.
    pub content_hash: u64,
    /// Minimum corner of the protein bounding box (angstroms).
    pub min_corner: [f64; 3],
    /// Maximum corner of the protein bounding box (angstroms).
    pub max_corner: [f64; 3],
    /// Span along each axis (angstroms).
    pub extent: [f64; 3],
    /// Number of residues.
    pub residue_count: usize,
}

/// Convert a protein SDF bounding box into an analytics event.
#[inline]
pub fn bio_bounds_to_analytics(protein: &ProteinSdf) -> BioAnalyticsBoundsEvent {
    let (min_c, max_c) = protein.bounding_box();
    let extent = [
        max_c[0] - min_c[0],
        max_c[1] - min_c[1],
        max_c[2] - min_c[2],
    ];
    let mut key = [0u8; 48];
    for i in 0..3 {
        key[i * 8..(i + 1) * 8].copy_from_slice(&min_c[i].to_bits().to_le_bytes());
        key[24 + i * 8..24 + (i + 1) * 8].copy_from_slice(&max_c[i].to_bits().to_le_bytes());
    }

    BioAnalyticsBoundsEvent {
        content_hash: fnv1a(&key),
        min_corner: min_c,
        max_corner: max_c,
        extent,
        residue_count: protein.residue_count(),
    }
}

// ── Bridge 3: Residue → Analytics (backbone angle metrics) ─────────────

/// Backbone angle metrics event for ALICE-Analytics ingestion.
pub struct BioAnalyticsBackboneEvent {
    /// Content hash over phi, psi, omega angle bytes.
    pub content_hash: u64,
    /// Amino acid one-letter code.
    pub amino_acid: char,
    /// Phi angle (radians).
    pub phi_rad: f64,
    /// Psi angle (radians).
    pub psi_rad: f64,
    /// Omega angle (radians).
    pub omega_rad: f64,
    /// Whether the residue falls in Ramachandran-allowed region.
    pub ramachandran_allowed: bool,
}

/// Convert a residue into a backbone angle analytics event.
#[inline]
pub fn bio_residue_to_analytics(residue: &Residue) -> BioAnalyticsBackboneEvent {
    let mut key = [0u8; 25];
    key[0] = residue.amino.one_letter() as u8;
    key[1..9].copy_from_slice(&residue.phi.to_bits().to_le_bytes());
    key[9..17].copy_from_slice(&residue.psi.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&residue.omega.to_bits().to_le_bytes());

    BioAnalyticsBackboneEvent {
        content_hash: fnv1a(&key),
        amino_acid: residue.amino.one_letter(),
        phi_rad: residue.phi,
        psi_rad: residue.psi,
        omega_rad: residue.omega,
        ramachandran_allowed: residue.is_ramachandran_allowed(),
    }
}

// ── Bridge 4: TotalEnergy → DB (energy record) ────────────────────────

/// Energy record for ALICE-DB persistence.
pub struct BioDbEnergyRecord {
    /// Content hash over all energy component bytes.
    pub content_hash: u64,
    /// Total van der Waals energy (Lennard-Jones).
    pub vdw_energy: f64,
    /// Total electrostatic energy (Coulomb).
    pub electrostatic_energy: f64,
    /// Total hydrogen bond energy.
    pub hbond_energy: f64,
    /// Total torsion energy (backbone).
    pub torsion_energy: f64,
    /// Sum of all energy components.
    pub total_energy: f64,
}

/// Convert a total energy report into a DB energy record.
#[inline]
pub fn bio_energy_to_db(energy: &TotalEnergy) -> BioDbEnergyRecord {
    let mut key = [0u8; 40];
    key[0..8].copy_from_slice(&energy.van_der_waals.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&energy.electrostatic.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&energy.hydrogen_bonds.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&energy.torsional.to_bits().to_le_bytes());
    key[32..40].copy_from_slice(&energy.total().to_bits().to_le_bytes());

    BioDbEnergyRecord {
        content_hash: fnv1a(&key),
        vdw_energy: energy.van_der_waals,
        electrostatic_energy: energy.electrostatic,
        hbond_energy: energy.hydrogen_bonds,
        torsion_energy: energy.torsional,
        total_energy: energy.total(),
    }
}

// ── Bridge 5: TotalEnergy → Cache (real-time energy lookup) ────────────

/// Energy cache entry for ALICE-Cache real-time lookup.
pub struct BioCacheEnergy {
    /// Content hash over total energy bytes.
    pub content_hash: u64,
    /// Sum of all energy components.
    pub total_energy: f64,
    /// Whether the energy is favorable (negative).
    pub is_favorable: bool,
    /// Cache TTL in seconds: 10s if unstable (|total| > 1000), else 60s.
    pub ttl_secs: u32,
}

/// Convert a total energy into a cache entry with adaptive TTL.
#[inline]
pub fn bio_energy_to_cache(energy: &TotalEnergy) -> BioCacheEnergy {
    let total = energy.total();
    let mut key = [0u8; 8];
    key[0..8].copy_from_slice(&total.to_bits().to_le_bytes());
    // Branchless TTL: unstable=1 → 60-50=10, stable=0 → 60-0=60.
    let unstable = (total.abs() > 1000.0) as u32;
    let ttl_secs = 60 - unstable * 50;

    BioCacheEnergy {
        content_hash: fnv1a(&key),
        total_energy: total,
        is_favorable: total < 0.0,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_bio::{AminoAcid, Residue, ProteinSdf, TotalEnergy};

    fn make_residue(aa: AminoAcid, phi: f64, psi: f64) -> Residue {
        Residue::new(aa, phi, psi, std::f64::consts::PI)
    }

    fn make_protein() -> ProteinSdf {
        let residues = vec![
            make_residue(AminoAcid::Ala, -1.0, -0.8),
            make_residue(AminoAcid::Gly, -1.1, -0.7),
            make_residue(AminoAcid::Val, -1.2, -0.9),
        ];
        ProteinSdf::new(residues)
    }

    #[test]
    fn test_bio_sdf_eval() {
        let protein = make_protein();
        let sample = bio_sdf_eval(&protein, [10.0, 0.0, 0.0]);
        assert_ne!(sample.content_hash, 0);
        assert_eq!(sample.residue_count, 3);
        assert!(sample.distance > 0.0); // far from protein
    }

    #[test]
    fn test_bio_bounds_to_analytics() {
        let protein = make_protein();
        let ev = bio_bounds_to_analytics(&protein);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.residue_count, 3);
        for i in 0..3 {
            assert!(ev.extent[i] >= 0.0);
        }
    }

    #[test]
    fn test_bio_residue_to_analytics() {
        let res = make_residue(AminoAcid::Ala, -1.0, -0.8);
        let ev = bio_residue_to_analytics(&res);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.amino_acid, 'A');
        assert!((ev.phi_rad - (-1.0)).abs() < 1e-10);
        assert!((ev.psi_rad - (-0.8)).abs() < 1e-10);
    }

    #[test]
    fn test_bio_energy_to_db() {
        let energy = TotalEnergy { van_der_waals: -5.0, electrostatic: -2.0, hydrogen_bonds: -1.5, torsional: 0.3 };
        let rec = bio_energy_to_db(&energy);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.vdw_energy - (-5.0)).abs() < 1e-10);
        assert!((rec.total_energy - (-8.2)).abs() < 1e-10);
    }

    #[test]
    fn test_bio_energy_to_cache_favorable() {
        let energy = TotalEnergy { van_der_waals: -500.0, electrostatic: -200.0, hydrogen_bonds: -100.0, torsional: 50.0 };
        let entry = bio_energy_to_cache(&energy);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.is_favorable);
        assert_eq!(entry.ttl_secs, 60); // |total| = 750 < 1000
    }

    #[test]
    fn test_bio_energy_to_cache_unstable() {
        let energy = TotalEnergy { van_der_waals: -2000.0, electrostatic: -500.0, hydrogen_bonds: 0.0, torsional: 100.0 };
        let entry = bio_energy_to_cache(&energy);
        assert_eq!(entry.ttl_secs, 10); // |total| = 2400 > 1000
    }

    #[test]
    fn test_hash_determinism() {
        let protein = make_protein();
        let s1 = bio_sdf_eval(&protein, [1.0, 2.0, 3.0]);
        let s2 = bio_sdf_eval(&protein, [1.0, 2.0, 3.0]);
        assert_eq!(s1.content_hash, s2.content_hash);
    }
}
