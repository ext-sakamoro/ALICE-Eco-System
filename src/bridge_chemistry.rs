//! Chemistry bridges — ALICE-Chemistry ↔ DB, Cache, Analytics, ML, Physics
//!
//! 5 bridges connecting molecular simulation to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Chemistry → DB (reaction log) ───────────────────────────────

/// Reaction log record for ALICE-DB persistence.
pub struct ChemistryDbRecord {
    /// Content hash over the reaction fields.
    pub content_hash: u64,
    /// Number of atoms in the simulation.
    pub atom_count: u32,
    /// Number of bonds in the simulation.
    pub bond_count: u32,
    /// Temperature in Kelvin.
    pub temperature_k: f64,
    /// Total system energy in kJ/mol.
    pub energy_kj: f64,
    /// Reaction rate in mol/L/s.
    pub reaction_rate: f64,
    /// Simulation step index.
    pub step_index: u64,
}

/// Serialize reaction data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn chemistry_to_db_record(
    atom_count: u32,
    bond_count: u32,
    temperature_k: f64,
    energy_kj: f64,
    reaction_rate: f64,
    step_index: u64,
) -> ChemistryDbRecord {
    let mut key = [0u8; 24];
    key[0..4].copy_from_slice(&atom_count.to_le_bytes());
    key[4..8].copy_from_slice(&bond_count.to_le_bytes());
    key[8..16].copy_from_slice(&energy_kj.to_le_bytes());
    key[16..24].copy_from_slice(&step_index.to_le_bytes());
    ChemistryDbRecord {
        content_hash: fnv1a(&key),
        atom_count,
        bond_count,
        temperature_k,
        energy_kj,
        reaction_rate,
        step_index,
    }
}

// ── Bridge 2: Chemistry → Cache (element cache) ───────────────────────────

/// Element property cache entry for ALICE-Cache.
pub struct ChemistryCacheEntry {
    /// Content hash for cache key derivation.
    pub content_hash: u64,
    /// Atomic number of the element.
    pub atomic_number: u16,
    /// Atomic mass in unified atomic mass units.
    pub atomic_mass_u: f64,
    /// Electronegativity (Pauling scale).
    pub electronegativity: f32,
    /// Covalent radius in picometres.
    pub covalent_radius_pm: f32,
    /// TTL in seconds (branchless: longer for stable noble-gas elements).
    pub ttl_secs: u32,
}

/// Cache element properties for ALICE-Cache.
#[inline]
#[must_use]
pub fn chemistry_to_cache_entry(
    atomic_number: u16,
    atomic_mass_u: f64,
    electronegativity: f32,
    covalent_radius_pm: f32,
    is_noble_gas: bool,
) -> ChemistryCacheEntry {
    // Branchless TTL: 3600 s for noble gases, 300 s otherwise.
    let noble = is_noble_gas as u32;
    let ttl_secs = 300_u32 + noble * 3300;

    let mut key = [0u8; 10];
    key[0..2].copy_from_slice(&atomic_number.to_le_bytes());
    key[2..10].copy_from_slice(&atomic_mass_u.to_le_bytes());
    ChemistryCacheEntry {
        content_hash: fnv1a(&key),
        atomic_number,
        atomic_mass_u,
        electronegativity,
        covalent_radius_pm,
        ttl_secs,
    }
}

// ── Bridge 3: Chemistry → Analytics (simulation metrics) ──────────────────

/// Simulation metrics for ALICE-Analytics ingestion.
pub struct ChemistryAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total simulation steps completed.
    pub step_count: u64,
    /// Average temperature across steps in Kelvin.
    pub avg_temperature_k: f64,
    /// Average energy per step in kJ/mol.
    pub avg_energy_kj: f64,
    /// Total reaction events recorded.
    pub reaction_events: u64,
    /// Average reaction rate in mol/L/s.
    pub avg_reaction_rate: f64,
    /// Peak atom count observed.
    pub peak_atom_count: u32,
}

/// Build simulation metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn chemistry_to_analytics_metrics(
    step_count: u64,
    total_temperature_k: f64,
    total_energy_kj: f64,
    reaction_events: u64,
    total_reaction_rate: f64,
    peak_atom_count: u32,
) -> ChemistryAnalyticsMetrics {
    let rcp = 1.0 / step_count.max(1) as f64;
    let avg_temperature_k = total_temperature_k * rcp;
    let avg_energy_kj = total_energy_kj * rcp;
    let avg_reaction_rate = total_reaction_rate * rcp;

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&step_count.to_le_bytes());
    key[8..16].copy_from_slice(&reaction_events.to_le_bytes());
    ChemistryAnalyticsMetrics {
        content_hash: fnv1a(&key),
        step_count,
        avg_temperature_k,
        avg_energy_kj,
        reaction_events,
        avg_reaction_rate,
        peak_atom_count,
    }
}

// ── Bridge 4: Chemistry → ML (molecular features) ─────────────────────────

/// Molecular feature vector for ALICE-ML training and inference.
pub struct ChemistryMlFeatures {
    /// Content hash for feature deduplication.
    pub content_hash: u64,
    /// Number of atoms.
    pub atom_count: u32,
    /// Number of bonds.
    pub bond_count: u32,
    /// Temperature in Kelvin.
    pub temperature_k: f64,
    /// System energy in kJ/mol.
    pub energy_kj: f64,
    /// Reaction rate in mol/L/s.
    pub reaction_rate: f64,
    /// Bond-to-atom ratio (derived feature).
    pub bond_atom_ratio: f64,
}

/// Extract molecular features for ALICE-ML.
#[inline]
#[must_use]
pub fn chemistry_to_ml_features(
    atom_count: u32,
    bond_count: u32,
    temperature_k: f64,
    energy_kj: f64,
    reaction_rate: f64,
) -> ChemistryMlFeatures {
    let rcp_atoms = 1.0 / atom_count.max(1) as f64;
    let bond_atom_ratio = bond_count as f64 * rcp_atoms;

    let mut key = [0u8; 24];
    key[0..4].copy_from_slice(&atom_count.to_le_bytes());
    key[4..8].copy_from_slice(&bond_count.to_le_bytes());
    key[8..16].copy_from_slice(&energy_kj.to_le_bytes());
    key[16..24].copy_from_slice(&reaction_rate.to_le_bytes());
    ChemistryMlFeatures {
        content_hash: fnv1a(&key),
        atom_count,
        bond_count,
        temperature_k,
        energy_kj,
        reaction_rate,
        bond_atom_ratio,
    }
}

// ── Bridge 5: Chemistry → Physics (force field) ───────────────────────────

/// Force field parameters for ALICE-Physics integration.
pub struct ChemistryPhysicsForceField {
    /// Content hash over the force field parameters.
    pub content_hash: u64,
    /// Lennard-Jones epsilon well depth in kJ/mol.
    pub lj_epsilon_kj: f32,
    /// Lennard-Jones sigma (zero-crossing distance) in Angstroms.
    pub lj_sigma_ang: f32,
    /// Coulomb partial charge on the particle (elementary charge units).
    pub partial_charge_e: f32,
    /// Bond spring constant in kJ/mol/Å².
    pub bond_k: f32,
    /// Equilibrium bond length in Angstroms.
    pub bond_length_eq_ang: f32,
    /// Number of atoms in the force field model.
    pub atom_count: u32,
}

/// Build force field parameters for ALICE-Physics.
#[inline]
#[must_use]
pub fn chemistry_to_physics_force_field(
    lj_epsilon_kj: f32,
    lj_sigma_ang: f32,
    partial_charge_e: f32,
    bond_k: f32,
    bond_length_eq_ang: f32,
    atom_count: u32,
) -> ChemistryPhysicsForceField {
    let mut key = [0u8; 24];
    key[0..4].copy_from_slice(&lj_epsilon_kj.to_le_bytes());
    key[4..8].copy_from_slice(&lj_sigma_ang.to_le_bytes());
    key[8..12].copy_from_slice(&partial_charge_e.to_le_bytes());
    key[12..16].copy_from_slice(&bond_k.to_le_bytes());
    key[16..20].copy_from_slice(&bond_length_eq_ang.to_le_bytes());
    key[20..24].copy_from_slice(&atom_count.to_le_bytes());
    ChemistryPhysicsForceField {
        content_hash: fnv1a(&key),
        lj_epsilon_kj,
        lj_sigma_ang,
        partial_charge_e,
        bond_k,
        bond_length_eq_ang,
        atom_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chemistry_to_db_record_hash_nonzero() {
        let rec = chemistry_to_db_record(100, 120, 298.15, -450.0, 0.05, 1_000);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.atom_count, 100);
        assert_eq!(rec.bond_count, 120);
    }

    #[test]
    fn test_chemistry_to_db_record_deterministic() {
        let a = chemistry_to_db_record(50, 60, 300.0, -200.0, 0.01, 42);
        let b = chemistry_to_db_record(50, 60, 300.0, -200.0, 0.01, 42);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_chemistry_to_cache_entry_noble_gas_ttl() {
        let noble = chemistry_to_cache_entry(2, 4.0026, 0.0, 31.0, true);
        assert_eq!(noble.ttl_secs, 3600);
        assert_ne!(noble.content_hash, 0);
    }

    #[test]
    fn test_chemistry_to_cache_entry_non_noble_ttl() {
        let carbon = chemistry_to_cache_entry(6, 12.011, 2.55, 77.0, false);
        assert_eq!(carbon.ttl_secs, 300);
        assert_eq!(carbon.atomic_number, 6);
    }

    #[test]
    fn test_chemistry_to_analytics_metrics_averages() {
        let m = chemistry_to_analytics_metrics(10, 2981.5, 4500.0, 20, 0.5, 200);
        assert_ne!(m.content_hash, 0);
        assert!((m.avg_temperature_k - 298.15).abs() < 0.01);
        assert!((m.avg_energy_kj - 450.0).abs() < 0.01);
        assert_eq!(m.peak_atom_count, 200);
    }

    #[test]
    fn test_chemistry_to_analytics_metrics_zero_steps() {
        let m = chemistry_to_analytics_metrics(0, 0.0, 0.0, 0, 0.0, 0);
        assert_eq!(m.step_count, 0);
        assert_eq!(m.avg_energy_kj, 0.0);
    }

    #[test]
    fn test_chemistry_to_ml_features_bond_atom_ratio() {
        let f = chemistry_to_ml_features(100, 150, 300.0, -500.0, 0.1);
        assert_ne!(f.content_hash, 0);
        assert!((f.bond_atom_ratio - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_chemistry_to_physics_force_field() {
        let ff = chemistry_to_physics_force_field(0.65, 3.4, -0.834, 450.0, 1.0, 3);
        assert_ne!(ff.content_hash, 0);
        assert_eq!(ff.atom_count, 3);
        assert!((ff.lj_epsilon_kj - 0.65).abs() < 0.001);
        assert!((ff.bond_length_eq_ang - 1.0).abs() < 0.001);
    }
}
