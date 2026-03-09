//! VM bridges — VM ↔ DB, Cache, Analytics, Monitor, API
//!
//! 5 bridges connecting the virtual machine runtime to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: VM → DB (bytecode storage) ─────────────────────────────────

/// Bytecode persistence record for ALICE-DB.
pub struct VmDbRecord {
    /// Content hash (FNV-1a of instruction_count + stack_depth + heap_size).
    pub content_hash: u64,
    /// Number of instructions in the bytecode.
    pub instruction_count: u32,
    /// Maximum stack depth observed during execution.
    pub stack_depth: u32,
    /// Heap allocation size in bytes.
    pub heap_size: u64,
    /// Number of execution cycles consumed.
    pub execution_cycles: u64,
    /// Number of registers used.
    pub register_count: u16,
}

/// Serialize VM execution state for ALICE-DB bytecode storage.
#[inline]
#[must_use]
pub fn vm_to_db_record(
    instruction_count: u32,
    stack_depth: u32,
    heap_size: u64,
    execution_cycles: u64,
    register_count: u16,
) -> VmDbRecord {
    let mut buf = [0u8; 4 + 4 + 8 + 8 + 2];
    buf[..4].copy_from_slice(&instruction_count.to_le_bytes());
    buf[4..8].copy_from_slice(&stack_depth.to_le_bytes());
    buf[8..16].copy_from_slice(&heap_size.to_le_bytes());
    buf[16..24].copy_from_slice(&execution_cycles.to_le_bytes());
    buf[24..26].copy_from_slice(&register_count.to_le_bytes());
    VmDbRecord {
        content_hash: fnv1a(&buf),
        instruction_count,
        stack_depth,
        heap_size,
        execution_cycles,
        register_count,
    }
}

// ── Bridge 2: VM → Cache (execution cache) ───────────────────────────────

/// Execution cache entry for ALICE-Cache.
pub struct VmCacheEntry {
    /// Content hash (FNV-1a of instruction_count + execution_cycles).
    pub content_hash: u64,
    /// Number of instructions (cache key component).
    pub instruction_count: u32,
    /// Execution cycles (used to size TTL).
    pub execution_cycles: u64,
    /// Cache TTL in seconds (branchless: 60 if cycles < threshold, else 10).
    pub ttl_secs: u32,
    /// Register count.
    pub register_count: u16,
}

/// Build an execution cache entry for ALICE-Cache.
///
/// `ttl_secs` is computed branchlessly: 60 when `execution_cycles < 1_000_000`, else 10.
#[inline]
#[must_use]
pub fn vm_to_cache_entry(
    instruction_count: u32,
    execution_cycles: u64,
    register_count: u16,
) -> VmCacheEntry {
    let mut buf = [0u8; 4 + 8 + 2];
    buf[..4].copy_from_slice(&instruction_count.to_le_bytes());
    buf[4..12].copy_from_slice(&execution_cycles.to_le_bytes());
    buf[12..14].copy_from_slice(&register_count.to_le_bytes());
    // ブランチレスTTL: サイクル数が閾値未満なら60秒、それ以外は10秒
    let is_fast = (execution_cycles < 1_000_000) as u32;
    let ttl_secs = 10 + is_fast * 50;
    VmCacheEntry {
        content_hash: fnv1a(&buf),
        instruction_count,
        execution_cycles,
        ttl_secs,
        register_count,
    }
}

// ── Bridge 3: VM → Analytics (runtime metrics) ───────────────────────────

/// Runtime metrics for ALICE-Analytics.
pub struct VmAnalyticsMetrics {
    /// Content hash (FNV-1a of all metric fields).
    pub content_hash: u64,
    /// Number of instructions executed.
    pub instruction_count: u32,
    /// Maximum stack depth.
    pub stack_depth: u32,
    /// Heap size in bytes.
    pub heap_size: u64,
    /// Total execution cycles.
    pub execution_cycles: u64,
    /// Register count.
    pub register_count: u16,
    /// Number of call frames opened.
    pub call_frame_count: u32,
}

/// Extract runtime metrics for ALICE-Analytics.
#[inline]
#[must_use]
pub fn vm_to_analytics_metrics(
    instruction_count: u32,
    stack_depth: u32,
    heap_size: u64,
    execution_cycles: u64,
    register_count: u16,
    call_frame_count: u32,
) -> VmAnalyticsMetrics {
    let mut buf = [0u8; 4 + 4 + 8 + 8 + 2 + 4];
    buf[..4].copy_from_slice(&instruction_count.to_le_bytes());
    buf[4..8].copy_from_slice(&stack_depth.to_le_bytes());
    buf[8..16].copy_from_slice(&heap_size.to_le_bytes());
    buf[16..24].copy_from_slice(&execution_cycles.to_le_bytes());
    buf[24..26].copy_from_slice(&register_count.to_le_bytes());
    buf[26..30].copy_from_slice(&call_frame_count.to_le_bytes());
    VmAnalyticsMetrics {
        content_hash: fnv1a(&buf),
        instruction_count,
        stack_depth,
        heap_size,
        execution_cycles,
        register_count,
        call_frame_count,
    }
}

// ── Bridge 4: VM → Monitor (health) ─────────────────────────────────────

/// VM health snapshot for ALICE-Monitor.
pub struct VmMonitorHealth {
    /// Content hash (FNV-1a of heap_size + execution_cycles + stack_depth).
    pub content_hash: u64,
    /// Heap size in bytes.
    pub heap_size: u64,
    /// Execution cycles since last reset.
    pub execution_cycles: u64,
    /// Current stack depth.
    pub stack_depth: u32,
    /// Number of active call frames.
    pub call_frame_count: u32,
    /// Is the VM in a halted state (0 = running, 1 = halted).
    pub is_halted: u8,
}

/// Build a VM health snapshot for ALICE-Monitor.
#[inline]
#[must_use]
pub fn vm_to_monitor_health(
    heap_size: u64,
    execution_cycles: u64,
    stack_depth: u32,
    call_frame_count: u32,
    is_halted: bool,
) -> VmMonitorHealth {
    let mut buf = [0u8; 8 + 8 + 4 + 4 + 1];
    buf[..8].copy_from_slice(&heap_size.to_le_bytes());
    buf[8..16].copy_from_slice(&execution_cycles.to_le_bytes());
    buf[16..20].copy_from_slice(&stack_depth.to_le_bytes());
    buf[20..24].copy_from_slice(&call_frame_count.to_le_bytes());
    buf[24] = is_halted as u8;
    VmMonitorHealth {
        content_hash: fnv1a(&buf),
        heap_size,
        execution_cycles,
        stack_depth,
        call_frame_count,
        is_halted: is_halted as u8,
    }
}

// ── Bridge 5: VM → API (execution service) ───────────────────────────────

/// Execution service descriptor for ALICE-API.
pub struct VmApiDescriptor {
    /// Content hash (FNV-1a of instruction_count + register_count + heap_size).
    pub content_hash: u64,
    /// Instruction count reported by the VM.
    pub instruction_count: u32,
    /// Register count.
    pub register_count: u16,
    /// Heap size in bytes.
    pub heap_size: u64,
    /// Execution cycles.
    pub execution_cycles: u64,
    /// Stack depth at export time.
    pub stack_depth: u32,
}

/// Build an execution service descriptor for ALICE-API.
#[inline]
#[must_use]
pub fn vm_to_api_descriptor(
    instruction_count: u32,
    register_count: u16,
    heap_size: u64,
    execution_cycles: u64,
    stack_depth: u32,
) -> VmApiDescriptor {
    let mut buf = [0u8; 4 + 2 + 8 + 8 + 4];
    buf[..4].copy_from_slice(&instruction_count.to_le_bytes());
    buf[4..6].copy_from_slice(&register_count.to_le_bytes());
    buf[6..14].copy_from_slice(&heap_size.to_le_bytes());
    buf[14..22].copy_from_slice(&execution_cycles.to_le_bytes());
    buf[22..26].copy_from_slice(&stack_depth.to_le_bytes());
    VmApiDescriptor {
        content_hash: fnv1a(&buf),
        instruction_count,
        register_count,
        heap_size,
        execution_cycles,
        stack_depth,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_to_db_record_basic() {
        let rec = vm_to_db_record(100, 32, 4096, 200_000, 16);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.instruction_count, 100);
        assert_eq!(rec.stack_depth, 32);
        assert_eq!(rec.heap_size, 4096);
        assert_eq!(rec.execution_cycles, 200_000);
        assert_eq!(rec.register_count, 16);
    }

    #[test]
    fn test_vm_to_db_record_determinism() {
        let r1 = vm_to_db_record(10, 5, 512, 1000, 8);
        let r2 = vm_to_db_record(10, 5, 512, 1000, 8);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_vm_to_cache_entry_fast_ttl() {
        // execution_cycles < 1_000_000 → ttl_secs = 60
        let e = vm_to_cache_entry(50, 500_000, 8);
        assert_eq!(e.ttl_secs, 60);
        assert_ne!(e.content_hash, 0);
    }

    #[test]
    fn test_vm_to_cache_entry_slow_ttl() {
        // execution_cycles >= 1_000_000 → ttl_secs = 10
        let e = vm_to_cache_entry(50, 2_000_000, 8);
        assert_eq!(e.ttl_secs, 10);
    }

    #[test]
    fn test_vm_to_analytics_metrics_basic() {
        let m = vm_to_analytics_metrics(200, 64, 8192, 400_000, 32, 10);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.instruction_count, 200);
        assert_eq!(m.call_frame_count, 10);
    }

    #[test]
    fn test_vm_to_monitor_health_running() {
        let h = vm_to_monitor_health(4096, 100_000, 16, 5, false);
        assert_ne!(h.content_hash, 0);
        assert_eq!(h.is_halted, 0);
        assert_eq!(h.stack_depth, 16);
    }

    #[test]
    fn test_vm_to_monitor_health_halted() {
        let h = vm_to_monitor_health(4096, 100_000, 16, 5, true);
        assert_eq!(h.is_halted, 1);
    }

    #[test]
    fn test_vm_to_api_descriptor_basic() {
        let d = vm_to_api_descriptor(128, 16, 2048, 300_000, 24);
        assert_ne!(d.content_hash, 0);
        assert_eq!(d.instruction_count, 128);
        assert_eq!(d.register_count, 16);
        assert_eq!(d.heap_size, 2048);
    }
}
