//! Cross-domain bridges — ALICE-Atoms ↔ SDF, Physics, ML
//!
//! 3 bridges connecting crystallographic data to SDF tree visualization,
//! physics rigid body simulation, and ML feature extraction.

use alice_atoms::{Crystal, Element, PairPotential, lattice_energy};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

// ── Bridge 1: Crystal → SDF (tree for visualization) ────────────────

/// SDF tree metadata derived from a crystal structure.
///
/// Maps a crystal unit cell into ALICE-SDF tree metadata so the SDF
/// layer can construct a union-of-spheres CSG tree for molecular
/// visualization without accessing raw atom positions.
pub struct AtomsSdfTree {
    /// FNV-1a hash over lattice_type, atom_count, volume, bounding_box bytes.
    pub content_hash: u64,
    /// Lattice type discriminant: 0=FCC, 1=BCC, 2=HCP, 3=Diamond, 4=Rocksalt, 5=Zincblende.
    pub lattice_type: u8,
    /// Number of atoms in the unit cell.
    pub atom_count: usize,
    /// Bounding box dimensions (lattice constants in angstroms).
    pub bounding_box: [f64; 3],
    /// Unit cell volume in cubic angstroms.
    pub volume_a3: f64,
    /// SDF node count: atom_count + 1 (union root + per-atom spheres).
    pub node_count: usize,
    /// Estimated ASDF byte size: 8 (header) + atom_count * 32 (per-atom SDF node).
    pub estimated_asdf_bytes: usize,
}

/// Convert a crystal into SDF tree metadata for visualization.
#[inline]
pub fn atoms_crystal_to_sdf_tree(crystal: &Crystal) -> AtomsSdfTree {
    let lt_byte = crystal.lattice_type as u8;
    let atom_count = crystal.atom_count();
    let volume = crystal.volume();
    let bounding_box = crystal.lattice_constants;
    let node_count = atom_count + 1;
    let estimated_asdf_bytes = 8 + atom_count * 32;

    let mut key = [0u8; 41];
    key[0] = lt_byte;
    key[1..9].copy_from_slice(&(atom_count as u64).to_le_bytes());
    key[9..17].copy_from_slice(&volume.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&bounding_box[0].to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&bounding_box[1].to_bits().to_le_bytes());
    key[33..41].copy_from_slice(&bounding_box[2].to_bits().to_le_bytes());

    AtomsSdfTree {
        content_hash: fnv1a(&key),
        lattice_type: lt_byte,
        atom_count,
        bounding_box,
        volume_a3: volume,
        node_count,
        estimated_asdf_bytes,
    }
}

// ── Bridge 2: Crystal → Physics (rigid body) ────────────────────────

/// Physics rigid body derived from a crystal structure.
///
/// Maps crystal density and volume into ALICE-Physics rigid body
/// metadata so the Physics layer can simulate macro-scale dynamics
/// without accessing atomic-level detail.
pub struct AtomsPhysicsBody {
    /// FNV-1a hash over lattice_type, atom_count, density, volume, mass bytes.
    pub content_hash: u64,
    /// Lattice type discriminant.
    pub lattice_type: u8,
    /// Number of atoms in the unit cell.
    pub atom_count: usize,
    /// Density in kg/m^3.
    pub density_kg_m3: f64,
    /// Unit cell volume in cubic angstroms.
    pub volume_a3: f64,
    /// Estimated mass in kg: density * volume * 1e-30 (angstrom^3 to m^3).
    pub mass_kg: f64,
    /// Always true for crystalline structures.
    pub is_rigid: bool,
}

/// Convert a crystal into a physics rigid body.
#[inline]
pub fn atoms_crystal_to_physics_body(crystal: &Crystal) -> AtomsPhysicsBody {
    let lt_byte = crystal.lattice_type as u8;
    let atom_count = crystal.atom_count();
    let density = crystal.density();
    let volume = crystal.volume();
    let mass_kg = density * volume * 1e-30;

    let mut key = [0u8; 33];
    key[0] = lt_byte;
    key[1..9].copy_from_slice(&(atom_count as u64).to_le_bytes());
    key[9..17].copy_from_slice(&density.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&volume.to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&mass_kg.to_bits().to_le_bytes());

    AtomsPhysicsBody {
        content_hash: fnv1a(&key),
        lattice_type: lt_byte,
        atom_count,
        density_kg_m3: density,
        volume_a3: volume,
        mass_kg,
        is_rigid: true,
    }
}

// ── Bridge 3: Crystal → ML (feature vector) ─────────────────────────

/// ML feature vector derived from a crystal structure.
///
/// Extracts numeric features from a crystal (lattice type, atom count,
/// volume, density, mean electronegativity, lattice energy) so the ML
/// layer can train on material property prediction without accessing
/// raw crystallographic data.
pub struct AtomsMlFeatures {
    /// FNV-1a hash over lattice_type, atom_count, volume, density, mean_en, lattice_energy bytes.
    pub content_hash: u64,
    /// Lattice type discriminant.
    pub lattice_type: u8,
    /// Number of atoms in the unit cell.
    pub atom_count: usize,
    /// Unit cell volume in cubic angstroms.
    pub volume_a3: f64,
    /// Density in kg/m^3.
    pub density_kg_m3: f64,
    /// Mean electronegativity across all basis atoms.
    pub mean_electronegativity: f64,
    /// Feature dimensionality: always 6.
    pub feature_dim: usize,
    /// Lattice energy in eV.
    pub lattice_energy: f64,
}

/// Convert a crystal into an ML feature vector.
#[inline]
pub fn atoms_crystal_to_ml_features(crystal: &Crystal) -> AtomsMlFeatures {
    let lt_byte = crystal.lattice_type as u8;
    let atom_count = crystal.atom_count();
    let volume = crystal.volume();
    let density = crystal.density();

    // Compute mean electronegativity from basis atoms
    let mean_en = if crystal.basis.is_empty() {
        0.0
    } else {
        let sum: f64 = crystal.basis.iter().map(|a| a.element.electronegativity()).sum();
        sum / crystal.basis.len() as f64
    };

    // Default LJ potential: epsilon=0.01 eV, sigma=2.5 A, cutoff=10.0 A
    let default_potential = PairPotential::new(0.01, 2.5, 10.0);
    let le = lattice_energy(crystal, &default_potential);

    let mut key = [0u8; 49];
    key[0] = lt_byte;
    key[1..9].copy_from_slice(&(atom_count as u64).to_le_bytes());
    key[9..17].copy_from_slice(&volume.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&density.to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&mean_en.to_bits().to_le_bytes());
    key[33..41].copy_from_slice(&le.to_bits().to_le_bytes());
    // Pad feature_dim as constant 6
    key[41..49].copy_from_slice(&(6u64).to_le_bytes());

    AtomsMlFeatures {
        content_hash: fnv1a(&key),
        lattice_type: lt_byte,
        atom_count,
        volume_a3: volume,
        density_kg_m3: density,
        mean_electronegativity: mean_en,
        feature_dim: 6,
        lattice_energy: le,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_atoms::{Crystal, LatticeType, Atom, Element, compute_band_structure, predict_properties};
    use std::f64::consts::PI;

    fn make_si_crystal() -> Crystal {
        let mut c = Crystal::new(LatticeType::Diamond, [5.43, 5.43, 5.43], [PI / 2.0; 3]);
        c.add_atom(Atom::new(Element::Si, [0.0, 0.0, 0.0]));
        c.add_atom(Atom::new(Element::Si, [0.25, 0.25, 0.25]));
        c.finalize();
        c
    }

    // ── Bridge 1: crystal → SDF tree ──────────────────────────────────

    #[test]
    fn test_atoms_crystal_to_sdf_tree() {
        let c = make_si_crystal();
        let tree = atoms_crystal_to_sdf_tree(&c);
        assert_ne!(tree.content_hash, 0);
        assert_eq!(tree.lattice_type, 3); // Diamond
        assert_eq!(tree.atom_count, 2);
        assert_eq!(tree.bounding_box, [5.43, 5.43, 5.43]);
        assert!(tree.volume_a3 > 0.0);
        assert_eq!(tree.node_count, 3); // 2 atoms + 1 union root
        assert_eq!(tree.estimated_asdf_bytes, 8 + 2 * 32); // header + 2 * per-atom
    }

    #[test]
    fn test_atoms_crystal_to_sdf_tree_empty() {
        let c = Crystal::new(LatticeType::FCC, [4.0, 4.0, 4.0], [PI / 2.0; 3]);
        let tree = atoms_crystal_to_sdf_tree(&c);
        assert_eq!(tree.atom_count, 0);
        assert_eq!(tree.node_count, 1); // just the union root
        assert_eq!(tree.estimated_asdf_bytes, 8); // header only
    }

    #[test]
    fn test_atoms_crystal_to_sdf_tree_deterministic() {
        let c = make_si_crystal();
        let t1 = atoms_crystal_to_sdf_tree(&c);
        let t2 = atoms_crystal_to_sdf_tree(&c);
        assert_eq!(t1.content_hash, t2.content_hash);
    }

    // ── Bridge 2: crystal → physics body ──────────────────────────────

    #[test]
    fn test_atoms_crystal_to_physics_body() {
        let c = make_si_crystal();
        let body = atoms_crystal_to_physics_body(&c);
        assert_ne!(body.content_hash, 0);
        assert_eq!(body.lattice_type, 3); // Diamond
        assert_eq!(body.atom_count, 2);
        assert!(body.density_kg_m3 > 0.0);
        assert!(body.volume_a3 > 0.0);
        // mass = density * volume * 1e-30
        let expected_mass = body.density_kg_m3 * body.volume_a3 * 1e-30;
        assert!((body.mass_kg - expected_mass).abs() < 1e-45);
        assert!(body.is_rigid);
    }

    #[test]
    fn test_atoms_crystal_to_physics_body_empty() {
        let c = Crystal::new(LatticeType::BCC, [3.0, 3.0, 3.0], [PI / 2.0; 3]);
        let body = atoms_crystal_to_physics_body(&c);
        assert_eq!(body.atom_count, 0);
        assert_eq!(body.density_kg_m3, 0.0);
        assert_eq!(body.mass_kg, 0.0);
        assert!(body.is_rigid);
    }

    #[test]
    fn test_atoms_crystal_to_physics_body_deterministic() {
        let c = make_si_crystal();
        let b1 = atoms_crystal_to_physics_body(&c);
        let b2 = atoms_crystal_to_physics_body(&c);
        assert_eq!(b1.content_hash, b2.content_hash);
    }

    // ── Bridge 3: crystal → ML features ───────────────────────────────

    #[test]
    fn test_atoms_crystal_to_ml_features() {
        let c = make_si_crystal();
        let feat = atoms_crystal_to_ml_features(&c);
        assert_ne!(feat.content_hash, 0);
        assert_eq!(feat.lattice_type, 3); // Diamond
        assert_eq!(feat.atom_count, 2);
        assert!(feat.volume_a3 > 0.0);
        assert!(feat.density_kg_m3 > 0.0);
        // Si electronegativity is 1.90
        assert!((feat.mean_electronegativity - Element::Si.electronegativity()).abs() < 1e-10);
        assert_eq!(feat.feature_dim, 6);
        // Lattice energy should be non-zero for a valid crystal
        assert!(feat.lattice_energy.abs() > 0.0);
    }

    #[test]
    fn test_atoms_crystal_to_ml_features_mixed_elements() {
        let mut c = Crystal::new(LatticeType::Rocksalt, [5.64, 5.64, 5.64], [PI / 2.0; 3]);
        c.add_atom(Atom::new(Element::Na, [0.0, 0.0, 0.0]));
        c.add_atom(Atom::new(Element::Cl, [0.5, 0.0, 0.0]));
        c.finalize();
        let feat = atoms_crystal_to_ml_features(&c);
        // Mean EN should be average of Na (0.93) and Cl (3.16)
        let expected_en = (Element::Na.electronegativity() + Element::Cl.electronegativity()) / 2.0;
        assert!((feat.mean_electronegativity - expected_en).abs() < 1e-10);
        assert_eq!(feat.atom_count, 2);
    }

    #[test]
    fn test_atoms_crystal_to_ml_features_empty() {
        let c = Crystal::new(LatticeType::HCP, [3.0, 3.0, 5.0], [PI / 2.0; 3]);
        let feat = atoms_crystal_to_ml_features(&c);
        assert_eq!(feat.atom_count, 0);
        assert!((feat.mean_electronegativity - 0.0).abs() < 1e-10);
        assert_eq!(feat.feature_dim, 6);
    }

    #[test]
    fn test_atoms_crystal_to_ml_features_deterministic() {
        let c = make_si_crystal();
        let f1 = atoms_crystal_to_ml_features(&c);
        let f2 = atoms_crystal_to_ml_features(&c);
        assert_eq!(f1.content_hash, f2.content_hash);
    }
}
