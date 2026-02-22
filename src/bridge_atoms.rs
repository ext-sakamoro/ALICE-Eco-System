//! Atoms bridges — ALICE-Atoms ↔ Analytics, DB, Cache
//!
//! 5 bridges connecting molecular compilation to the ALICE ecosystem.

use alice_atoms::{
    compute_band_structure, predict_properties, BandStructure, CompilationResult, Crystal,
    MaterialProperties,
};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Crystal → Analytics (material metrics) ─────────────────

/// Crystal material metrics for ALICE-Analytics ingestion.
pub struct AtomsAnalyticsCrystalEvent {
    /// Content hash over lattice type, atom count, volume, density bytes.
    pub content_hash: u64,
    /// Lattice type discriminant: 0=FCC, 1=BCC, 2=HCP, 3=Diamond, 4=Rocksalt, 5=Zincblende.
    pub lattice_type: u8,
    /// Number of atoms in the unit cell.
    pub atom_count: usize,
    /// Unit cell volume in cubic angstroms.
    pub volume_a3: f64,
    /// Density in kg/m³.
    pub density_kg_m3: f64,
}

/// Convert a crystal into a material metrics analytics event.
#[inline]
pub fn atoms_crystal_to_analytics(crystal: &Crystal) -> AtomsAnalyticsCrystalEvent {
    let vol = crystal.volume();
    let density = crystal.density();
    let lt_byte = crystal.lattice_type as u8;

    let mut key = [0u8; 25];
    key[0] = lt_byte;
    key[1..9].copy_from_slice(&(crystal.atom_count() as u64).to_le_bytes());
    key[9..17].copy_from_slice(&vol.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&density.to_bits().to_le_bytes());

    AtomsAnalyticsCrystalEvent {
        content_hash: fnv1a(&key),
        lattice_type: lt_byte,
        atom_count: crystal.atom_count(),
        volume_a3: vol,
        density_kg_m3: density,
    }
}

// ── Bridge 2: BandStructure → Analytics (electronic properties) ──────

/// Band structure event for ALICE-Analytics ingestion.
pub struct AtomsAnalyticsBandEvent {
    /// Content hash over band_gap, fermi_energy, classification bytes.
    pub content_hash: u64,
    /// Band gap in eV.
    pub band_gap_ev: f64,
    /// Fermi energy in eV.
    pub fermi_energy_ev: f64,
    /// Classification: 0=metal, 1=semiconductor, 2=insulator.
    pub classification: u8,
    /// Effective electron mass (units of free electron mass).
    pub effective_mass: f64,
}

/// Convert a band structure into an analytics event.
#[inline]
pub fn atoms_band_to_analytics(bs: &BandStructure) -> AtomsAnalyticsBandEvent {
    let class_byte = if bs.is_metal {
        0u8
    } else if bs.is_semiconductor {
        1
    } else {
        2
    };

    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&bs.band_gap_ev.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&bs.fermi_energy_ev.to_bits().to_le_bytes());
    key[16] = class_byte;
    key[17..25].copy_from_slice(&bs.effective_mass_electron.to_bits().to_le_bytes());

    AtomsAnalyticsBandEvent {
        content_hash: fnv1a(&key),
        band_gap_ev: bs.band_gap_ev,
        fermi_energy_ev: bs.fermi_energy_ev,
        classification: class_byte,
        effective_mass: bs.effective_mass_electron,
    }
}

// ── Bridge 3: MaterialProperties → Analytics (property metrics) ──────

/// Material property metrics for ALICE-Analytics ingestion.
pub struct AtomsAnalyticsPropertyEvent {
    /// Content hash over hardness, conductivity, melting_point bytes.
    pub content_hash: u64,
    /// Hardness in GPa.
    pub hardness_gpa: f64,
    /// Electrical conductivity in S/m.
    pub conductivity_s_m: f64,
    /// Melting point in Kelvin.
    pub melting_point_k: f64,
    /// Thermal conductivity in W/(m·K).
    pub thermal_conductivity_w_mk: f64,
}

/// Convert material properties into an analytics event.
#[inline]
pub fn atoms_properties_to_analytics(props: &MaterialProperties) -> AtomsAnalyticsPropertyEvent {
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&props.hardness_gpa.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&props.conductivity_s_m.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&props.melting_point_k.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&props.thermal_conductivity_w_mk.to_bits().to_le_bytes());

    AtomsAnalyticsPropertyEvent {
        content_hash: fnv1a(&key),
        hardness_gpa: props.hardness_gpa,
        conductivity_s_m: props.conductivity_s_m,
        melting_point_k: props.melting_point_k,
        thermal_conductivity_w_mk: props.thermal_conductivity_w_mk,
    }
}

// ── Bridge 4: CompilationResult → DB (compilation record) ────────────

/// Compilation record for ALICE-DB persistence.
pub struct AtomsDbCompilationRecord {
    /// Content hash over fitness, iterations, crystal content_hash bytes.
    pub content_hash: u64,
    /// Fitness score (0.0 to 1.0).
    pub fitness: f64,
    /// Number of GA iterations performed.
    pub iterations: u32,
    /// Crystal lattice type discriminant.
    pub lattice_type: u8,
    /// Number of atoms in the compiled crystal.
    pub atom_count: usize,
    /// Band gap of the compiled crystal (eV).
    pub band_gap_ev: f64,
    /// Density of the compiled crystal (kg/m³).
    pub density_kg_m3: f64,
}

/// Convert a compilation result into a DB record.
#[inline]
pub fn atoms_compilation_to_db(result: &CompilationResult) -> AtomsDbCompilationRecord {
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&result.fitness.to_bits().to_le_bytes());
    key[8..12].copy_from_slice(&result.iterations.to_le_bytes());
    key[12..20].copy_from_slice(&result.crystal.content_hash.to_le_bytes());

    AtomsDbCompilationRecord {
        content_hash: fnv1a(&key),
        fitness: result.fitness,
        iterations: result.iterations,
        lattice_type: result.crystal.lattice_type as u8,
        atom_count: result.crystal.atom_count(),
        band_gap_ev: result.band_structure.band_gap_ev,
        density_kg_m3: result.predicted_properties.density_kg_m3,
    }
}

// ── Bridge 5: CompilationResult → Cache (quick fitness lookup) ───────

/// Compilation cache entry for ALICE-Cache real-time lookup.
pub struct AtomsCacheCompilation {
    /// Content hash over fitness bytes.
    pub content_hash: u64,
    /// Fitness score (0.0 to 1.0).
    pub fitness: f64,
    /// Whether the compilation met the fitness threshold.
    pub is_converged: bool,
    /// Cache TTL: 60s if not converged, else 600s.
    pub ttl_secs: u32,
}

/// Convert a compilation result into a cache entry with adaptive TTL.
#[inline]
pub fn atoms_compilation_to_cache(result: &CompilationResult) -> AtomsCacheCompilation {
    let mut key = [0u8; 8];
    key[0..8].copy_from_slice(&result.fitness.to_bits().to_le_bytes());

    // Branchless TTL: not_converged=1 → 600-540=60, converged=0 → 600-0=600.
    let not_converged = (result.fitness < 0.95) as u32;
    let ttl_secs = 600 - not_converged * 540;

    AtomsCacheCompilation {
        content_hash: fnv1a(&key),
        fitness: result.fitness,
        is_converged: result.fitness >= 0.95,
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_atoms::{
        compile, compute_band_structure, predict_properties, Atom, CompilerConfig, Crystal,
        Element, LatticeType, PropertyTarget,
    };
    use std::f64::consts::PI;

    fn make_si_crystal() -> Crystal {
        let mut c = Crystal::new(LatticeType::Diamond, [5.43, 5.43, 5.43], [PI / 2.0; 3]);
        c.add_atom(Atom::new(Element::Si, [0.0, 0.0, 0.0]));
        c.add_atom(Atom::new(Element::Si, [0.25, 0.25, 0.25]));
        c.finalize();
        c
    }

    fn make_compilation() -> CompilationResult {
        let target = PropertyTarget {
            density_kg_m3: Some(2700.0),
            ..PropertyTarget::default()
        };
        let config = CompilerConfig {
            max_iterations: 30,
            population_size: 15,
            ..CompilerConfig::default()
        };
        compile(&target, &config)
    }

    #[test]
    fn test_crystal_to_analytics() {
        let c = make_si_crystal();
        let ev = atoms_crystal_to_analytics(&c);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.lattice_type, 3); // Diamond
        assert_eq!(ev.atom_count, 2);
        assert!(ev.volume_a3 > 0.0);
        assert!(ev.density_kg_m3 > 0.0);
    }

    #[test]
    fn test_band_to_analytics() {
        let c = make_si_crystal();
        let bs = compute_band_structure(&c);
        let ev = atoms_band_to_analytics(&bs);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.band_gap_ev > 0.0); // Si is semiconductor
        assert_eq!(ev.classification, 1); // semiconductor
    }

    #[test]
    fn test_band_to_analytics_metal() {
        let mut c = Crystal::new(LatticeType::FCC, [3.6, 3.6, 3.6], [PI / 2.0; 3]);
        c.add_atom(Atom::new(Element::Cu, [0.0, 0.0, 0.0]));
        let bs = compute_band_structure(&c);
        let ev = atoms_band_to_analytics(&bs);
        assert_eq!(ev.classification, 0); // metal
    }

    #[test]
    fn test_properties_to_analytics() {
        let c = make_si_crystal();
        let props = predict_properties(&c);
        let ev = atoms_properties_to_analytics(&props);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.hardness_gpa > 0.0);
        assert!(ev.melting_point_k > 300.0);
    }

    #[test]
    fn test_compilation_to_db() {
        let result = make_compilation();
        let rec = atoms_compilation_to_db(&result);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.fitness > 0.0);
        assert!(rec.iterations > 0);
        assert!(rec.atom_count > 0);
    }

    #[test]
    fn test_compilation_to_cache() {
        let result = make_compilation();
        let entry = atoms_compilation_to_cache(&result);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.fitness > 0.0);
        // TTL depends on convergence
        assert!(entry.ttl_secs == 60 || entry.ttl_secs == 600);
    }

    #[test]
    fn test_hash_determinism() {
        let c = make_si_crystal();
        let e1 = atoms_crystal_to_analytics(&c);
        let e2 = atoms_crystal_to_analytics(&c);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_empty_crystal_analytics() {
        let c = Crystal::new(LatticeType::FCC, [4.0, 4.0, 4.0], [PI / 2.0; 3]);
        let ev = atoms_crystal_to_analytics(&c);
        assert_eq!(ev.atom_count, 0);
        assert_eq!(ev.density_kg_m3, 0.0);
    }
}
