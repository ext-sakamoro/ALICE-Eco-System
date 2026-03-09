//! Quantum bridges — ALICE-Quantum ↔ DB, Cache, Analytics, ML, Monitor
//!
//! 5 bridges connecting quantum circuit execution to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Quantum → DB (circuit execution record) ────────────────────

/// Circuit execution record for ALICE-DB persistence.
pub struct QuantumDbRecord {
    /// Content hash over the circuit execution snapshot.
    pub content_hash: u64,
    /// Number of qubits in the circuit.
    pub qubit_count: u16,
    /// Total number of gate operations.
    pub gate_count: u32,
    /// Depth of the circuit (longest path in the DAG).
    pub circuit_depth: u32,
    /// Number of measurement operations.
    pub measurement_count: u32,
    /// Execution fidelity in basis points (0–10 000).
    pub fidelity_bps: u16,
}

/// Serialize a quantum circuit execution for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn quantum_to_db_record(
    qubit_count: u16,
    gate_count: u32,
    circuit_depth: u32,
    measurement_count: u32,
    fidelity_bps: u16,
) -> QuantumDbRecord {
    let mut buf = [0u8; 18];
    buf[0..2].copy_from_slice(&qubit_count.to_le_bytes());
    buf[2..6].copy_from_slice(&gate_count.to_le_bytes());
    buf[6..10].copy_from_slice(&circuit_depth.to_le_bytes());
    buf[10..14].copy_from_slice(&measurement_count.to_le_bytes());
    buf[14..16].copy_from_slice(&fidelity_bps.to_le_bytes());
    QuantumDbRecord {
        content_hash: fnv1a(&buf[..16]),
        qubit_count,
        gate_count,
        circuit_depth,
        measurement_count,
        fidelity_bps,
    }
}

// ── Bridge 2: Quantum → Cache (statevector cache) ────────────────────────

/// Statevector cache entry for ALICE-Cache.
pub struct QuantumCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Number of qubits (statevector size = 2^qubit_count).
    pub qubit_count: u16,
    /// TTL for this cache entry in seconds.
    pub ttl_secs: u32,
    /// Byte size of the serialised statevector.
    pub state_bytes: u64,
    /// Hash of the circuit that produced this statevector.
    pub circuit_hash: u64,
}

/// Build a statevector cache entry for ALICE-Cache.
///
/// Small circuits (≤ 20 qubits) receive a longer TTL (600 s vs 60 s)
/// because their statevectors fit in memory without pressure.
#[inline]
#[must_use]
pub fn quantum_to_cache_entry(
    qubit_count: u16,
    state_bytes: u64,
    circuit_hash: u64,
) -> QuantumCacheEntry {
    let mut buf = [0u8; 18];
    buf[0..2].copy_from_slice(&qubit_count.to_le_bytes());
    buf[2..10].copy_from_slice(&state_bytes.to_le_bytes());
    buf[10..18].copy_from_slice(&circuit_hash.to_le_bytes());
    let large_circuit = (qubit_count > 20) as u32;
    let ttl_secs = 600 - large_circuit * 540;
    QuantumCacheEntry {
        content_hash: fnv1a(&buf),
        qubit_count,
        ttl_secs,
        state_bytes,
        circuit_hash,
    }
}

// ── Bridge 3: Quantum → Analytics (execution event) ──────────────────────

/// Execution analytics event for ALICE-Analytics ingestion.
pub struct QuantumAnalyticsEvent {
    /// Content hash over the event tuple.
    pub content_hash: u64,
    /// Number of qubits in the circuit.
    pub qubit_count: u16,
    /// Circuit execution time in microseconds.
    pub exec_time_us: u64,
    /// Execution fidelity in basis points (0–10 000).
    pub fidelity_bps: u16,
    /// Number of measurement shots.
    pub shot_count: u32,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build an execution analytics event for ALICE-Analytics.
#[inline]
#[must_use]
pub fn quantum_to_analytics_event(
    qubit_count: u16,
    exec_time_us: u64,
    fidelity_bps: u16,
    shot_count: u32,
    timestamp_ms: u64,
) -> QuantumAnalyticsEvent {
    let mut buf = [0u8; 26];
    buf[0..2].copy_from_slice(&qubit_count.to_le_bytes());
    buf[2..10].copy_from_slice(&exec_time_us.to_le_bytes());
    buf[10..12].copy_from_slice(&fidelity_bps.to_le_bytes());
    buf[12..16].copy_from_slice(&shot_count.to_le_bytes());
    buf[16..24].copy_from_slice(&timestamp_ms.to_le_bytes());
    QuantumAnalyticsEvent {
        content_hash: fnv1a(&buf[..24]),
        qubit_count,
        exec_time_us,
        fidelity_bps,
        shot_count,
        timestamp_ms,
    }
}

// ── Bridge 4: Quantum → ML (hybrid model descriptor) ─────────────────────

/// Hybrid quantum–classical model descriptor for ALICE-ML.
pub struct QuantumMlHybrid {
    /// Content hash over the model descriptor.
    pub content_hash: u64,
    /// Number of qubits in the quantum layer.
    pub qubit_count: u16,
    /// Number of classical trainable parameters.
    pub classical_params: u32,
    /// Number of variational quantum layers.
    pub quantum_layers: u16,
    /// Training loss multiplied by 10 000.
    pub loss_x10000: u32,
}

/// Build a hybrid model descriptor for ALICE-ML.
#[inline]
#[must_use]
pub fn quantum_to_ml_hybrid(
    qubit_count: u16,
    classical_params: u32,
    quantum_layers: u16,
    loss_x10000: u32,
) -> QuantumMlHybrid {
    let mut buf = [0u8; 12];
    buf[0..2].copy_from_slice(&qubit_count.to_le_bytes());
    buf[2..6].copy_from_slice(&classical_params.to_le_bytes());
    buf[6..8].copy_from_slice(&quantum_layers.to_le_bytes());
    buf[8..12].copy_from_slice(&loss_x10000.to_le_bytes());
    QuantumMlHybrid {
        content_hash: fnv1a(&buf),
        qubit_count,
        classical_params,
        quantum_layers,
        loss_x10000,
    }
}

// ── Bridge 5: Quantum → Monitor (hardware status) ────────────────────────

/// Hardware status report for ALICE-Monitor.
pub struct QuantumMonitorStatus {
    /// Content hash over the status snapshot.
    pub content_hash: u64,
    /// Number of physical qubits being monitored.
    pub qubit_count: u16,
    /// Gate error rate in basis points (0–10 000).
    pub error_rate_bps: u16,
    /// T2 coherence time in microseconds.
    pub coherence_time_us: u64,
    /// Whether the hardware has been recently calibrated.
    pub is_calibrated: bool,
    /// Wall-clock timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
}

/// Build a hardware status report for ALICE-Monitor.
#[inline]
#[must_use]
pub fn quantum_to_monitor_status(
    qubit_count: u16,
    error_rate_bps: u16,
    coherence_time_us: u64,
    is_calibrated: bool,
    timestamp_ms: u64,
) -> QuantumMonitorStatus {
    let mut buf = [0u8; 25];
    buf[0..2].copy_from_slice(&qubit_count.to_le_bytes());
    buf[2..4].copy_from_slice(&error_rate_bps.to_le_bytes());
    buf[4..12].copy_from_slice(&coherence_time_us.to_le_bytes());
    buf[12] = is_calibrated as u8;
    buf[13..21].copy_from_slice(&timestamp_ms.to_le_bytes());
    QuantumMonitorStatus {
        content_hash: fnv1a(&buf[..21]),
        qubit_count,
        error_rate_bps,
        coherence_time_us,
        is_calibrated,
        timestamp_ms,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_db_record_hash_nonzero() {
        let rec = quantum_to_db_record(127, 1_024, 40, 127, 9_800);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_quantum_db_record_fields() {
        let rec = quantum_to_db_record(20, 256, 15, 20, 9_500);
        assert_eq!(rec.qubit_count, 20);
        assert_eq!(rec.gate_count, 256);
        assert_eq!(rec.fidelity_bps, 9_500);
    }

    #[test]
    fn test_quantum_db_record_determinism() {
        let a = quantum_to_db_record(10, 100, 8, 10, 9_900);
        let b = quantum_to_db_record(10, 100, 8, 10, 9_900);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_quantum_cache_entry_small_circuit_ttl() {
        let entry = quantum_to_cache_entry(10, 8_192, 0xc1c2_1234);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600);
    }

    #[test]
    fn test_quantum_cache_entry_large_circuit_ttl() {
        let entry = quantum_to_cache_entry(30, 1_073_741_824, 0xc1c2_1234);
        assert_eq!(entry.ttl_secs, 60);
        assert_eq!(entry.qubit_count, 30);
    }

    #[test]
    fn test_quantum_analytics_event() {
        let ev = quantum_to_analytics_event(20, 150_000, 9_700, 1_024, 1_700_000_000_000);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.shot_count, 1_024);
        assert_eq!(ev.fidelity_bps, 9_700);
    }

    #[test]
    fn test_quantum_ml_hybrid() {
        let h = quantum_to_ml_hybrid(4, 256, 3, 1_250);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.quantum_layers, 3);
        assert_eq!(h.classical_params, 256);
    }

    #[test]
    fn test_quantum_monitor_status_calibrated() {
        let s = quantum_to_monitor_status(127, 50, 100_000, true, 1_700_000_000_000);
        assert_ne!(s.content_hash, 0);
        assert!(s.is_calibrated);
        assert_eq!(s.error_rate_bps, 50);
    }

    #[test]
    fn test_quantum_monitor_status_not_calibrated() {
        let s = quantum_to_monitor_status(127, 200, 80_000, false, 1_700_000_001_000);
        assert!(!s.is_calibrated);
        assert_eq!(s.qubit_count, 127);
    }
}
