//! Energy bridges — ALICE-Energy ↔ Analytics, DB, Edge
//!
//! 5 bridges connecting power grid simulation data to the ALICE ecosystem.

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

// ── Bridge 1: PowerGrid → Analytics (grid balance metrics) ──────────────

/// Grid balance metrics for ALICE-Analytics ingestion.
pub struct EnergyAnalyticsGridEvent {
    /// Content hash over generation, consumption, and balance bytes.
    pub content_hash: u64,
    /// Inner u64 of the grid ID.
    pub grid_id: u64,
    /// Total generation capacity (MW).
    pub total_generation_mw: f64,
    /// Total consumption (MW).
    pub total_consumption_mw: f64,
    /// Supply-demand balance (MW). Positive = surplus.
    pub balance_mw: f64,
    /// Number of nodes in the grid.
    pub node_count: usize,
    /// Maximum frequency deviation (Hz) across the grid.
    pub max_freq_deviation_hz: f64,
}

/// Convert a power grid into a grid balance analytics event.
#[inline]
#[must_use]
pub fn energy_grid_to_analytics(grid: &PowerGrid) -> EnergyAnalyticsGridEvent {
    let gen = grid.total_generation();
    let con = grid.total_consumption();
    let balance = grid.supply_demand_balance();
    let freq_dev = grid.frequency_deviation();

    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&gen.to_bits().to_le_bytes());
    key[8..16].copy_from_slice(&con.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&balance.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&grid.id.0.to_le_bytes());

    EnergyAnalyticsGridEvent {
        content_hash: fnv1a(&key),
        grid_id: grid.id.0,
        total_generation_mw: gen,
        total_consumption_mw: con,
        balance_mw: balance,
        node_count: grid.node_count(),
        max_freq_deviation_hz: freq_dev,
    }
}

// ── Bridge 2: PowerNode → Analytics (node utilization metrics) ──────────

/// Node utilization metrics for ALICE-Analytics ingestion.
pub struct EnergyAnalyticsNodeEvent {
    /// Content hash over node ID, kind, and utilization bytes.
    pub content_hash: u64,
    /// Inner u64 of the node ID.
    pub node_id: u64,
    /// Node kind: 0=Generator, 1=Consumer, 2=Storage, 3=Transformer, 4=Relay.
    pub node_kind: u8,
    /// Current utilization ratio (0.0 to 1.0+).
    pub utilization: f64,
    /// Whether the node is overloaded.
    pub is_overloaded: bool,
    /// Current output power (MW).
    pub current_output_mw: f64,
    /// Maximum capacity (MW).
    pub capacity_mw: f64,
}

/// Convert a power node into a utilization analytics event.
#[inline]
#[must_use]
pub fn energy_node_to_analytics(node: &PowerNode) -> EnergyAnalyticsNodeEvent {
    let kind_byte = match node.kind {
        NodeKind::Generator => 0,
        NodeKind::Consumer => 1,
        NodeKind::Storage => 2,
        NodeKind::Transformer => 3,
        NodeKind::Relay => 4,
    };
    let utilization = node.utilization();

    let mut key = [0u8; 17];
    key[0..8].copy_from_slice(&node.id.0.to_le_bytes());
    key[8] = kind_byte;
    key[9..17].copy_from_slice(&utilization.to_bits().to_le_bytes());

    EnergyAnalyticsNodeEvent {
        content_hash: fnv1a(&key),
        node_id: node.id.0,
        node_kind: kind_byte,
        utilization,
        is_overloaded: node.is_overloaded(),
        current_output_mw: node.current_output_mw,
        capacity_mw: node.capacity_mw,
    }
}

// ── Bridge 3: BatteryState → DB (battery health record) ────────────────

/// Battery health record for ALICE-DB persistence.
pub struct EnergyDbBatteryRecord {
    /// Content hash over battery ID, `SoC`, cycle count, and health bytes.
    pub content_hash: u64,
    /// Inner u64 of the battery ID.
    pub battery_id: u64,
    /// Chemistry type: 0=LithiumIon, 1=LithiumIronPhosphate, 2=SolidState, 3=SodiumIon, 4=FlowBattery.
    pub chemistry: u8,
    /// Current state of charge (0.0 to 1.0).
    pub soc: f64,
    /// Total cycle count.
    pub cycle_count: u32,
    /// Health percentage (0-100).
    pub health_pct: f64,
    /// Current temperature (Celsius).
    pub temperature_c: f64,
    /// Estimated time to replacement (days at 1 cycle/day, 20% min health).
    pub time_to_replacement_days: f64,
}

/// Convert a battery state into a DB health record.
#[inline]
#[must_use]
pub fn energy_battery_to_db(battery: &BatteryState) -> EnergyDbBatteryRecord {
    let chemistry_byte = match battery.chemistry {
        BatteryChemistry::LithiumIon => 0,
        BatteryChemistry::LithiumIronPhosphate => 1,
        BatteryChemistry::SolidState => 2,
        BatteryChemistry::SodiumIon => 3,
        BatteryChemistry::FlowBattery => 4,
    };
    let health = battery.health_percentage();
    let ttr = alice_energy::time_to_replacement(battery, 1.0, 20.0);

    let mut key = [0u8; 25];
    key[0..8].copy_from_slice(&battery.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&battery.state_of_charge.to_bits().to_le_bytes());
    key[16..20].copy_from_slice(&battery.cycle_count.to_le_bytes());
    key[20..24].copy_from_slice(&(health as u32).to_le_bytes());
    key[24] = chemistry_byte;

    EnergyDbBatteryRecord {
        content_hash: fnv1a(&key),
        battery_id: battery.id.0,
        chemistry: chemistry_byte,
        soc: battery.state_of_charge,
        cycle_count: battery.cycle_count,
        health_pct: health,
        temperature_c: battery.temperature_c,
        time_to_replacement_days: ttr,
    }
}

// ── Bridge 4: PhaseCorrection → Edge (real-time correction telemetry) ───

/// Phase correction telemetry for ALICE-Edge ingestion.
pub struct EnergyEdgePhaseTelemetry {
    /// Content hash over node ID and correction bytes.
    pub content_hash: u64,
    /// Inner u64 of the node ID receiving the correction.
    pub node_id: u64,
    /// Phase correction value (radians).
    pub correction_rad: f64,
    /// Frequency correction (Hz).
    pub correction_hz: f64,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Convert a phase correction into an edge telemetry event.
#[inline]
#[must_use]
pub fn energy_phase_to_edge(correction: &PhaseCorrection) -> EnergyEdgePhaseTelemetry {
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(&correction.node_id.0.to_le_bytes());
    key[8..16].copy_from_slice(&correction.correction_rad.to_bits().to_le_bytes());
    key[16..24].copy_from_slice(&correction.correction_hz.to_bits().to_le_bytes());
    key[24..32].copy_from_slice(&correction.timestamp_ns.to_le_bytes());

    EnergyEdgePhaseTelemetry {
        content_hash: fnv1a(&key),
        node_id: correction.node_id.0,
        correction_rad: correction.correction_rad,
        correction_hz: correction.correction_hz,
        timestamp_ns: correction.timestamp_ns,
    }
}

// ── Bridge 5: BatteryState → Cache (real-time SoC lookup) ──────────────

/// Battery `SoC` cache entry for ALICE-Cache real-time lookup.
pub struct EnergyCacheBattery {
    /// Content hash over battery ID and `SoC` bytes.
    pub content_hash: u64,
    /// Inner u64 of the battery ID.
    pub battery_id: u64,
    /// Current state of charge (0.0 to 1.0).
    pub soc: f64,
    /// Health percentage.
    pub health_pct: f64,
    /// Cache TTL: 5s if critical (`SoC` < 0.1 or > 0.95), else 30s.
    pub ttl_secs: u32,
}

/// Convert a battery state into a cache entry with adaptive TTL.
#[inline]
#[must_use]
pub fn energy_battery_to_cache(battery: &BatteryState) -> EnergyCacheBattery {
    let soc = battery.state_of_charge;
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&battery.id.0.to_le_bytes());
    key[8..16].copy_from_slice(&soc.to_bits().to_le_bytes());
    // Branchless TTL: critical=1 → 30-25=5, normal=0 → 30-0=30.
    let critical = !(0.1..=0.95).contains(&soc) as u32;
    let ttl_secs = 30 - critical * 25;

    EnergyCacheBattery {
        content_hash: fnv1a(&key),
        battery_id: battery.id.0,
        soc,
        health_pct: battery.health_percentage(),
        ttl_secs,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_energy::{
        BatteryChemistry, BatteryState, NodeId, NodeKind, PhaseCorrection, PowerGrid, PowerNode,
    };

    #[test]
    fn test_grid_to_analytics() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 80.0, 50.0, 220.0));
        let ev = energy_grid_to_analytics(&grid);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.grid_id, 1);
        assert_eq!(ev.node_count, 2);
        assert!(ev.total_generation_mw > 0.0);
    }

    #[test]
    fn test_node_to_analytics() {
        let mut node = PowerNode::new(42, NodeKind::Generator, 500.0, 50.0, 220.0);
        node.set_output(250.0);
        let ev = energy_node_to_analytics(&node);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.node_id, 42);
        assert_eq!(ev.node_kind, 0); // Generator
        assert!((ev.utilization - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_battery_to_db() {
        let mut battery = BatteryState::new(10, BatteryChemistry::LithiumIon, 100.0, 1000);
        battery.state_of_charge = 0.8;
        let rec = energy_battery_to_db(&battery);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.battery_id, 10);
        assert_eq!(rec.chemistry, 0); // LithiumIon
        assert!((rec.soc - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_phase_to_edge() {
        let correction = PhaseCorrection {
            node_id: NodeId(5),
            correction_rad: 0.05,
            correction_hz: 0.005,
            timestamp_ns: 1_000_000,
        };
        let ev = energy_phase_to_edge(&correction);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.node_id, 5);
        assert!((ev.correction_rad - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_battery_to_cache_normal() {
        let mut battery = BatteryState::new(7, BatteryChemistry::SolidState, 200.0, 5000);
        battery.state_of_charge = 0.5;
        let entry = energy_battery_to_cache(&battery);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 30); // 0.5 is normal
    }

    #[test]
    fn test_battery_to_cache_critical_low() {
        let mut battery = BatteryState::new(8, BatteryChemistry::FlowBattery, 50.0, 2000);
        battery.state_of_charge = 0.05;
        let entry = energy_battery_to_cache(&battery);
        assert_eq!(entry.ttl_secs, 5); // 0.05 < 0.1, critical
    }

    #[test]
    fn test_battery_to_cache_critical_high() {
        let mut battery = BatteryState::new(9, BatteryChemistry::LithiumIon, 100.0, 1000);
        battery.state_of_charge = 0.98;
        let entry = energy_battery_to_cache(&battery);
        assert_eq!(entry.ttl_secs, 5); // 0.98 > 0.95, critical
    }

    #[test]
    fn test_hash_determinism() {
        let node = PowerNode::new(1, NodeKind::Relay, 10.0, 50.0, 110.0);
        let e1 = energy_node_to_analytics(&node);
        let e2 = energy_node_to_analytics(&node);
        assert_eq!(e1.content_hash, e2.content_hash);
    }
}
