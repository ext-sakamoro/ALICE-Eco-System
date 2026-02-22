//! Cross-domain bridges — ALICE-Energy ↔ Physics, RTOS, ML, Sync
//!
//! 4 bridges connecting power grid simulation data to physics rigid body
//! representations, RTOS real-time monitoring tasks, ML battery degradation
//! features, and sync phase correction events.

use alice_energy::{
    BatteryChemistry, BatteryState, NodeKind, PhaseCorrection, PowerGrid, PowerNode,
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

// ── Bridge 1: PowerNode → Physics (rigid body representation) ──────────

/// Physics rigid body representation derived from a power grid node.
///
/// Maps electrical node properties to virtual mass and spatial position
/// so the Physics layer can simulate grid topology as a spring-mass
/// system for layout optimisation and stability analysis.
pub struct EnergyPhysicsBody {
    /// FNV-1a hash over `node_id`, kind, `power_mw`, `mass_equivalent`, `position_x` bytes.
    pub content_hash: u64,
    /// Node identifier.
    pub node_id: u64,
    /// Node kind as u8 discriminant.
    pub kind: u8,
    /// Current power output/consumption in megawatts.
    pub power_mw: f64,
    /// Virtual mass for physics simulation: `power_mw` * 1000.0.
    pub mass_equivalent: f64,
    /// Spatial X position: `node_id` as f64 * 10.0 (linear layout).
    pub position_x: f64,
}

/// Convert a power node into a physics rigid body representation.
#[inline]
#[must_use]
pub fn energy_node_to_physics_body(node: &PowerNode) -> EnergyPhysicsBody {
    let kind_byte = match node.kind {
        NodeKind::Generator => 0,
        NodeKind::Consumer => 1,
        NodeKind::Storage => 2,
        NodeKind::Transformer => 3,
        NodeKind::Relay => 4,
    };

    let mass_equivalent = node.current_output_mw * 1000.0;
    let position_x = node.id.0 as f64 * 10.0;

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&node.id.0.to_le_bytes());
    key[8] = kind_byte;
    key[9..17].copy_from_slice(&node.current_output_mw.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&mass_equivalent.to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&position_x.to_bits().to_le_bytes());

    EnergyPhysicsBody {
        content_hash: fnv1a(&key),
        node_id: node.id.0,
        kind: kind_byte,
        power_mw: node.current_output_mw,
        mass_equivalent,
        position_x,
    }
}

// ── Bridge 2: PowerGrid → RTOS (real-time monitoring task) ─────────────

/// RTOS real-time monitoring task derived from a power grid.
///
/// Encodes grid identity, node count, monitoring period, and supply-demand
/// balance so the RTOS kernel can schedule grid monitoring at the correct
/// frequency without accessing grid internals at runtime.
pub struct EnergyRtosTask {
    /// FNV-1a hash over `grid_id`, `node_count`, `period_us`, priority, `balance_mw` bytes.
    pub content_hash: u64,
    /// Grid identifier.
    pub grid_id: u64,
    /// Number of nodes in the grid.
    pub node_count: usize,
    /// Monitoring period in microseconds: `1_000_000` / 50 = `20_000us` for 50Hz grid.
    pub period_us: u64,
    /// Task priority: always high (1).
    pub priority: u8,
    /// Current supply-demand balance in megawatts.
    pub balance_mw: f64,
}

/// Convert a power grid into an RTOS monitoring task descriptor.
#[inline]
#[must_use]
pub fn energy_grid_to_rtos_task(grid: &PowerGrid) -> EnergyRtosTask {
    let node_count = grid.node_count();
    let nominal = grid.nominal_frequency_hz;
    let period_us = if nominal > 0.0 {
        (1_000_000.0 / nominal) as u64
    } else {
        20_000 // default 50Hz
    };
    let balance_mw = grid.supply_demand_balance();

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&grid.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&(node_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&period_us.to_le_bytes());
    key[24] = 1; // priority
    key[25..33].copy_from_slice(&balance_mw.to_bits().to_le_bytes());

    EnergyRtosTask {
        content_hash: fnv1a(&key),
        grid_id: grid.id.0,
        node_count,
        period_us,
        priority: 1,
        balance_mw,
    }
}

// ── Bridge 3: BatteryState → ML (feature vector) ──────────────────────

/// ML feature vector derived from battery state for degradation prediction.
///
/// Encodes battery chemistry, charge state, temperature, and cycle count
/// as a normalised feature set so the ML layer can train degradation
/// models without accessing raw battery internals.
pub struct EnergyMlFeatures {
    /// FNV-1a hash over `battery_id`, chemistry, soc, voltage, temperature, `cycle_count`, degradation bytes.
    pub content_hash: u64,
    /// Battery identifier.
    pub battery_id: u64,
    /// Battery chemistry as u8 discriminant.
    pub chemistry: u8,
    /// Current state of charge (0.0 to 1.0).
    pub soc: f64,
    /// Current voltage (approximated from `SoC`: 3.0 + soc * 1.2 for Li-ion range).
    pub voltage_v: f64,
    /// Current temperature in Celsius.
    pub temperature_c: f64,
    /// Total charge/discharge cycles.
    pub cycle_count: u32,
    /// Normalised degradation index: `cycle_count` / 5000.0.
    pub degradation_index: f64,
}

/// Convert a battery state into ML feature metadata.
#[inline]
#[must_use]
pub fn energy_battery_to_ml_features(battery: &BatteryState) -> EnergyMlFeatures {
    let chemistry_byte = match battery.chemistry {
        BatteryChemistry::LithiumIon => 0,
        BatteryChemistry::LithiumIronPhosphate => 1,
        BatteryChemistry::SolidState => 2,
        BatteryChemistry::SodiumIon => 3,
        BatteryChemistry::FlowBattery => 4,
    };

    // Approximate voltage from SoC (Li-ion nominal range 3.0V - 4.2V)
    let voltage_v = 3.0 + battery.state_of_charge * 1.2;
    let degradation_index = battery.cycle_count as f64 / 5000.0;

    let mut key = [0u8; 41];
    key[0..8].copy_from_slice(&battery.id.0.to_le_bytes());
    key[8] = chemistry_byte;
    key[9..17].copy_from_slice(&battery.state_of_charge.to_bits().to_le_bytes());
    key[17..25].copy_from_slice(&voltage_v.to_bits().to_le_bytes());
    key[25..33].copy_from_slice(&battery.temperature_c.to_bits().to_le_bytes());
    key[33..37].copy_from_slice(&battery.cycle_count.to_le_bytes());
    key[37..41].copy_from_slice(&(degradation_index.to_bits() as u32).to_le_bytes());

    EnergyMlFeatures {
        content_hash: fnv1a(&key),
        battery_id: battery.id.0,
        chemistry: chemistry_byte,
        soc: battery.state_of_charge,
        voltage_v,
        temperature_c: battery.temperature_c,
        cycle_count: battery.cycle_count,
        degradation_index,
    }
}

// ── Bridge 4: PhaseCorrection → Sync (sync event) ─────────────────────

/// Sync event derived from a phase correction for state replication.
///
/// Encodes frequency correction magnitude and criticality flag so the
/// Sync layer can prioritise replication of large deviations without
/// parsing the full correction payload.
pub struct EnergySyncEvent {
    /// FNV-1a hash over `node_id`, `timestamp_ns`, `correction_mw`, `deviation_hz`, `is_critical` bytes.
    pub content_hash: u64,
    /// Node receiving the correction.
    pub node_id: u64,
    /// Correction timestamp in Unix nanoseconds.
    pub timestamp_ns: u64,
    /// Frequency correction magnitude in Hz (mapped from `correction_hz`).
    pub correction_mw: f64,
    /// Frequency deviation in Hz.
    pub deviation_hz: f64,
    /// True if the absolute deviation exceeds 0.5 Hz (safety-critical).
    pub is_critical: bool,
    /// Fixed wire size: 24 bytes (`node_id:8` + timestamp:8 + correction:8).
    pub wire_bytes: usize,
}

/// Convert a phase correction into a sync event.
#[inline]
#[must_use]
pub fn energy_phase_to_sync_event(correction: &PhaseCorrection) -> EnergySyncEvent {
    let deviation_hz = correction.correction_hz;
    let is_critical = deviation_hz.abs() > 0.5;

    let mut key = [0u8; 33];
    key[0..8].copy_from_slice(&correction.node_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&correction.timestamp_ns.to_le_bytes());
    key[16..24].copy_from_slice(&correction.correction_hz.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&deviation_hz.to_bits().to_le_bytes());
    key[32] = is_critical as u8;

    EnergySyncEvent {
        content_hash: fnv1a(&key),
        node_id: correction.node_id.0,
        timestamp_ns: correction.timestamp_ns,
        correction_mw: correction.correction_hz,
        deviation_hz,
        is_critical,
        wire_bytes: 24,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_energy::{
        BatteryChemistry, BatteryId, BatteryState, NodeId, NodeKind, PhaseCorrection, PowerGrid,
        PowerNode,
    };

    // ── Bridge 1: node → physics body ───────────────────────────────────

    #[test]
    fn test_energy_node_to_physics_body() {
        let node = PowerNode::new(42, NodeKind::Generator, 500.0, 50.0, 220.0);
        let body = energy_node_to_physics_body(&node);
        assert_ne!(body.content_hash, 0);
        assert_eq!(body.node_id, 42);
        assert_eq!(body.kind, 0); // Generator
        assert!((body.power_mw - 500.0).abs() < 1e-6);
        assert!((body.mass_equivalent - 500_000.0).abs() < 1e-6);
        assert!((body.position_x - 420.0).abs() < 1e-6); // 42 * 10
    }

    #[test]
    fn test_energy_node_to_physics_body_consumer() {
        let node = PowerNode::new(10, NodeKind::Consumer, 80.0, 50.0, 220.0);
        let body = energy_node_to_physics_body(&node);
        assert_eq!(body.kind, 1); // Consumer
        assert!((body.power_mw - 80.0).abs() < 1e-6);
        assert!((body.position_x - 100.0).abs() < 1e-6); // 10 * 10
    }

    #[test]
    fn test_energy_node_to_physics_body_deterministic() {
        let node = PowerNode::new(1, NodeKind::Storage, 200.0, 50.0, 110.0);
        let b1 = energy_node_to_physics_body(&node);
        let b2 = energy_node_to_physics_body(&node);
        assert_eq!(b1.content_hash, b2.content_hash);
    }

    #[test]
    fn test_energy_node_to_physics_body_all_kinds() {
        let kinds = [
            (NodeKind::Generator, 0u8),
            (NodeKind::Consumer, 1),
            (NodeKind::Storage, 2),
            (NodeKind::Transformer, 3),
            (NodeKind::Relay, 4),
        ];
        for (kind, expected_byte) in &kinds {
            let node = PowerNode::new(1, *kind, 10.0, 50.0, 220.0);
            let body = energy_node_to_physics_body(&node);
            assert_eq!(body.kind, *expected_byte);
        }
    }

    // ── Bridge 2: grid → RTOS task ──────────────────────────────────────

    #[test]
    fn test_energy_grid_to_rtos_task() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));

        let task = energy_grid_to_rtos_task(&grid);
        assert_ne!(task.content_hash, 0);
        assert_eq!(task.grid_id, 1);
        assert_eq!(task.node_count, 2);
        assert_eq!(task.period_us, 20_000); // 1_000_000 / 50
        assert_eq!(task.priority, 1);
        assert!((task.balance_mw - 40.0).abs() < 1e-6); // 100 - 60
    }

    #[test]
    fn test_energy_grid_to_rtos_task_60hz() {
        let grid = PowerGrid::new(2, 60.0);
        let task = energy_grid_to_rtos_task(&grid);
        assert_eq!(task.period_us, 16_666); // 1_000_000 / 60 = 16666
        assert_eq!(task.node_count, 0);
    }

    #[test]
    fn test_energy_grid_to_rtos_task_deterministic() {
        let mut grid = PowerGrid::new(5, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 200.0, 50.0, 220.0));
        let t1 = energy_grid_to_rtos_task(&grid);
        let t2 = energy_grid_to_rtos_task(&grid);
        assert_eq!(t1.content_hash, t2.content_hash);
    }

    // ── Bridge 3: battery → ML features ─────────────────────────────────

    #[test]
    fn test_energy_battery_to_ml_features() {
        let mut battery = BatteryState::new(10, BatteryChemistry::LithiumIon, 100.0, 5000);
        battery.state_of_charge = 0.8;
        battery.temperature_c = 35.0;
        for _ in 0..1000 {
            battery.complete_cycle();
        }

        let features = energy_battery_to_ml_features(&battery);
        assert_ne!(features.content_hash, 0);
        assert_eq!(features.battery_id, 10);
        assert_eq!(features.chemistry, 0); // LithiumIon
        assert!((features.soc - 0.8).abs() < 1e-6);
        assert!((features.voltage_v - (3.0 + 0.8 * 1.2)).abs() < 1e-6); // 3.96V
        assert!((features.temperature_c - 35.0).abs() < 1e-6);
        assert_eq!(features.cycle_count, 1000);
        assert!((features.degradation_index - 0.2).abs() < 1e-6); // 1000 / 5000
    }

    #[test]
    fn test_energy_battery_to_ml_features_all_chemistries() {
        let chemistries = [
            (BatteryChemistry::LithiumIon, 0u8),
            (BatteryChemistry::LithiumIronPhosphate, 1),
            (BatteryChemistry::SolidState, 2),
            (BatteryChemistry::SodiumIon, 3),
            (BatteryChemistry::FlowBattery, 4),
        ];
        for (chem, expected_byte) in &chemistries {
            let battery = BatteryState::new(1, *chem, 50.0, 2000);
            let features = energy_battery_to_ml_features(&battery);
            assert_eq!(features.chemistry, *expected_byte);
        }
    }

    #[test]
    fn test_energy_battery_to_ml_features_deterministic() {
        let mut battery = BatteryState::new(7, BatteryChemistry::SolidState, 200.0, 5000);
        battery.state_of_charge = 0.5;
        let f1 = energy_battery_to_ml_features(&battery);
        let f2 = energy_battery_to_ml_features(&battery);
        assert_eq!(f1.content_hash, f2.content_hash);
    }

    // ── Bridge 4: phase correction → sync event ─────────────────────────

    #[test]
    fn test_energy_phase_to_sync_event_critical() {
        let correction = PhaseCorrection {
            node_id: NodeId(5),
            correction_rad: 0.1,
            correction_hz: 0.8, // |0.8| > 0.5 → critical
            timestamp_ns: 1_000_000,
        };
        let ev = energy_phase_to_sync_event(&correction);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.node_id, 5);
        assert_eq!(ev.timestamp_ns, 1_000_000);
        assert!((ev.correction_mw - 0.8).abs() < 1e-10);
        assert!((ev.deviation_hz - 0.8).abs() < 1e-10);
        assert!(ev.is_critical);
        assert_eq!(ev.wire_bytes, 24);
    }

    #[test]
    fn test_energy_phase_to_sync_event_not_critical() {
        let correction = PhaseCorrection {
            node_id: NodeId(10),
            correction_rad: 0.01,
            correction_hz: 0.1, // |0.1| < 0.5 → not critical
            timestamp_ns: 2_000_000,
        };
        let ev = energy_phase_to_sync_event(&correction);
        assert!(!ev.is_critical);
        assert_eq!(ev.wire_bytes, 24);
    }

    #[test]
    fn test_energy_phase_to_sync_event_negative_deviation() {
        let correction = PhaseCorrection {
            node_id: NodeId(3),
            correction_rad: -0.05,
            correction_hz: -0.6, // |-0.6| > 0.5 → critical
            timestamp_ns: 500,
        };
        let ev = energy_phase_to_sync_event(&correction);
        assert!(ev.is_critical);
        assert!((ev.deviation_hz - (-0.6)).abs() < 1e-10);
    }

    #[test]
    fn test_energy_phase_to_sync_event_deterministic() {
        let correction = PhaseCorrection {
            node_id: NodeId(1),
            correction_rad: 0.05,
            correction_hz: 0.005,
            timestamp_ns: 1000,
        };
        let e1 = energy_phase_to_sync_event(&correction);
        let e2 = energy_phase_to_sync_event(&correction);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    #[test]
    fn test_energy_phase_to_sync_event_boundary() {
        // Exactly 0.5 Hz should NOT be critical (> 0.5, not >=)
        let correction = PhaseCorrection {
            node_id: NodeId(1),
            correction_rad: 0.0,
            correction_hz: 0.5,
            timestamp_ns: 0,
        };
        let ev = energy_phase_to_sync_event(&correction);
        assert!(!ev.is_critical); // 0.5 is not > 0.5
    }
}
