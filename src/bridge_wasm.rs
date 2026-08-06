//! WASM bridges — ALICE-WASM ↔ DB, Cache, Analytics, CDN, Edge
//!
//! 5 bridges connecting the WebAssembly mini-runtime to the ALICE ecosystem.

use alice_wasm::{validate_program, Opcode, VmError, VM};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Encode an `Opcode` slice to a stable byte sequence for hashing.
///
/// Each opcode is encoded as a fixed 9-byte tag+value record so that
/// the hash is independent of the host `usize` width.
fn hash_program(program: &[Opcode]) -> u64 {
    let mut bytes = Vec::with_capacity(program.len() * 9);
    for op in program {
        let (tag, val): (u8, i64) = match *op {
            Opcode::Nop => (0, 0),
            Opcode::Push(v) => (1, v),
            Opcode::Pop => (2, 0),
            Opcode::Dup => (3, 0),
            Opcode::Add => (4, 0),
            Opcode::Sub => (5, 0),
            Opcode::Mul => (6, 0),
            Opcode::Div => (7, 0),
            Opcode::Mod => (8, 0),
            Opcode::Eq => (9, 0),
            Opcode::Lt => (10, 0),
            Opcode::Gt => (11, 0),
            Opcode::And => (12, 0),
            Opcode::Or => (13, 0),
            Opcode::Not => (14, 0),
            Opcode::Jump(a) => (15, a as i64),
            Opcode::JumpIf(a) => (16, a as i64),
            Opcode::Call(a) => (17, a as i64),
            Opcode::Ret => (18, 0),
            Opcode::Load(i) => (19, i as i64),
            Opcode::Store(i) => (20, i as i64),
            Opcode::Halt => (21, 0),
        };
        bytes.push(tag);
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    fnv1a(&bytes)
}

// ── Bridge 1: WASM → DB (module storage record) ───────────────────────────

/// WASM module storage record for ALICE-DB.
///
/// Persists a validated bytecode module alongside its validation status and
/// instruction-level statistics for later retrieval and replay.
pub struct WasmDbModuleRecord {
    /// FNV-1a hash of the encoded program bytecode (module identity key).
    pub content_hash: u64,
    /// Total number of instructions in the program.
    pub instruction_count: usize,
    /// Whether the program passed bytecode validation.
    pub is_valid: bool,
    /// Count of `Push` instructions (data-flow proxy).
    pub push_count: usize,
    /// Count of `Call` instructions (control-flow complexity proxy).
    pub call_count: usize,
    /// Count of `Halt` instructions (expected exit points).
    pub halt_count: usize,
}

/// Build a WASM module storage record for ALICE-DB.
///
/// Runs bytecode validation internally; `is_valid` is set accordingly.
#[inline]
#[must_use]
pub fn wasm_to_db_module_record(program: &[Opcode]) -> WasmDbModuleRecord {
    let content_hash = hash_program(program);
    let is_valid = validate_program(program).is_ok();

    let mut push_count = 0usize;
    let mut call_count = 0usize;
    let mut halt_count = 0usize;
    for op in program {
        match op {
            Opcode::Push(_) => push_count += 1,
            Opcode::Call(_) => call_count += 1,
            Opcode::Halt => halt_count += 1,
            _ => {}
        }
    }

    WasmDbModuleRecord {
        content_hash,
        instruction_count: program.len(),
        is_valid,
        push_count,
        call_count,
        halt_count,
    }
}

// ── Bridge 2: WASM → Cache (module cache entry) ───────────────────────────

/// WASM module cache entry for ALICE-Cache.
///
/// Caches a validated module's bytecode for fast re-instantiation.
/// TTL is shortened branchlessly for large modules.
pub struct WasmCacheEntry {
    /// FNV-1a hash of the program (cache key).
    pub content_hash: u64,
    /// Instruction count.
    pub instruction_count: usize,
    /// Whether the module is valid (invalid modules are cached with short TTL).
    pub is_valid: bool,
    /// Cache TTL in seconds (branchless: shorter for large or invalid modules).
    pub ttl_secs: u32,
}

/// Build a WASM module cache entry.
///
/// TTL rules (applied branchlessly):
/// - Base TTL: 3600 s.
/// - Large module (> 256 instructions): −1800 s.
/// - Invalid module: result is halved again (−900 s of the remaining).
#[inline]
#[must_use]
pub fn wasm_to_cache_entry(program: &[Opcode]) -> WasmCacheEntry {
    let content_hash = hash_program(program);
    let is_valid = validate_program(program).is_ok();

    // Branchless TTL computation.
    let large = (program.len() > 256) as u32;
    let invalid = (!is_valid) as u32;
    let ttl_secs = 3600 - large * 1800 - invalid * 900;

    WasmCacheEntry {
        content_hash,
        instruction_count: program.len(),
        is_valid,
        ttl_secs,
    }
}

// ── Bridge 3: WASM → Analytics (execution metrics) ────────────────────────

/// WASM execution metrics for ALICE-Analytics.
///
/// Records fuel consumption, stack depth, and exit status for every
/// executed module so the analytics layer can track runtime behaviour.
pub struct WasmAnalyticsMetrics {
    /// FNV-1a hash of the program (analytics stream key).
    pub content_hash: u64,
    /// Fuel consumed during execution.
    pub fuel_used: u64,
    /// Fuel budget provided to the VM.
    pub fuel_limit: u64,
    /// Fuel utilisation in permille of the budget.
    pub fuel_utilisation_permille: u32,
    /// Stack depth after execution (0 if execution errored).
    pub final_stack_depth: usize,
    /// Whether execution completed without error.
    pub execution_ok: bool,
    /// Exit code: 0=clean halt, 1=fuel exhausted, 2=stack underflow,
    /// 3=division by zero, 4=other error.
    pub exit_code: u8,
}

/// Execute a WASM program and capture analytics metrics.
///
/// `num_locals` and `fuel` are forwarded to `VM::new`.
/// The VM is consumed internally; the caller receives only the metrics.
#[inline]
#[must_use]
pub fn wasm_to_analytics_metrics(
    program: &[Opcode],
    num_locals: usize,
    fuel: u64,
) -> WasmAnalyticsMetrics {
    let content_hash = hash_program(program);
    let mut vm = VM::new(num_locals, fuel);
    let result = vm.execute(program);

    let execution_ok = result.is_ok();
    let exit_code = match &result {
        Ok(_) => 0u8,
        Err(VmError::FuelExhausted) => 1,
        Err(VmError::StackUnderflow) => 2,
        Err(VmError::DivisionByZero) => 3,
        Err(_) => 4,
    };
    let final_stack_depth = if execution_ok { vm.stack.len() } else { 0 };
    let fuel_used = vm.fuel_used;
    let fuel_safe = fuel.max(1);
    let fuel_utilisation_permille =
        (fuel_used.min(fuel_safe).wrapping_mul(1_000) / fuel_safe) as u32;

    WasmAnalyticsMetrics {
        content_hash,
        fuel_used,
        fuel_limit: fuel,
        fuel_utilisation_permille,
        final_stack_depth,
        execution_ok,
        exit_code,
    }
}

// ── Bridge 4: WASM → CDN (module delivery descriptor) ────────────────────

/// WASM module delivery descriptor for ALICE-CDN.
///
/// Packages module metadata for CDN edge serving so that edge nodes can
/// apply appropriate caching, content-type headers, and origin routing.
pub struct WasmCdnDescriptor {
    /// FNV-1a hash of the program (CDN asset fingerprint).
    pub content_hash: u64,
    /// Instruction count.
    pub instruction_count: usize,
    /// Estimated serialised module size in bytes (9 bytes per instruction).
    pub estimated_bytes: usize,
    /// Whether the module passed validation.
    pub is_valid: bool,
    /// Suggested CDN TTL in seconds.
    pub cdn_ttl_secs: u32,
}

/// Build a CDN delivery descriptor for a WASM module.
///
/// Valid modules receive a 24-hour TTL; invalid modules receive 0 (no cache).
#[inline]
#[must_use]
pub fn wasm_to_cdn_descriptor(program: &[Opcode]) -> WasmCdnDescriptor {
    let content_hash = hash_program(program);
    let is_valid = validate_program(program).is_ok();
    let instruction_count = program.len();
    let estimated_bytes = instruction_count * 9;

    // Valid → 86 400 s; invalid → 0 s (branchless).
    let cdn_ttl_secs = (is_valid as u32) * 86_400;

    WasmCdnDescriptor {
        content_hash,
        instruction_count,
        estimated_bytes,
        is_valid,
        cdn_ttl_secs,
    }
}

// ── Bridge 5: WASM → Edge (edge compute events) ───────────────────────────

/// Edge compute event for ALICE-Edge.
///
/// Emitted after a WASM module executes at an edge node, enabling the edge
/// layer to route results, trigger downstream actions, or quarantine
/// misbehaving modules.
pub struct WasmEdgeEvent {
    /// FNV-1a hash of the program (event correlation key).
    pub content_hash: u64,
    /// Event kind: 0=module_loaded, 1=execution_complete, 2=execution_failed,
    /// 3=fuel_exhausted, 4=module_evicted.
    pub event_kind: u8,
    /// Fuel consumed.
    pub fuel_used: u64,
    /// Whether execution succeeded.
    pub execution_ok: bool,
    /// Instruction count (proxy for module complexity).
    pub instruction_count: usize,
    /// Edge processing recommendation: 0=allow, 1=retry, 2=quarantine.
    pub edge_action: u8,
}

/// Build an edge compute event by executing a WASM program.
///
/// `event_kind`: 0=module_loaded, 1=execution_complete, 2=execution_failed,
/// 3=fuel_exhausted, 4=module_evicted.
/// `edge_action` is derived branchlessly: execution failures → quarantine (2),
/// fuel exhaustion → retry (1), success → allow (0).
#[inline]
#[must_use]
pub fn wasm_to_edge_event(program: &[Opcode], num_locals: usize, fuel: u64) -> WasmEdgeEvent {
    let content_hash = hash_program(program);
    let mut vm = VM::new(num_locals, fuel);
    let result = vm.execute(program);
    let execution_ok = result.is_ok();
    let fuel_used = vm.fuel_used;

    let fuel_exhausted = matches!(result, Err(VmError::FuelExhausted));
    let other_error = result.is_err() && !fuel_exhausted;

    // Branchless edge_action: other_error → 2, fuel_exhausted → 1, ok → 0.
    let edge_action = (other_error as u8) * 2 + (fuel_exhausted as u8);

    // Branchless event_kind: fuel_exhausted → 3, other_error → 2, ok → 1.
    let event_kind = if fuel_exhausted {
        3
    } else if other_error {
        2
    } else {
        1
    };

    WasmEdgeEvent {
        content_hash,
        event_kind,
        fuel_used,
        execution_ok,
        instruction_count: program.len(),
        edge_action,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_program() -> Vec<Opcode> {
        vec![Opcode::Push(3), Opcode::Push(4), Opcode::Add, Opcode::Halt]
    }

    fn invalid_program() -> Vec<Opcode> {
        vec![Opcode::Jump(999)] // out-of-bounds jump → invalid
    }

    #[test]
    fn test_db_module_record_valid() {
        let prog = simple_program();
        let rec = wasm_to_db_module_record(&prog);
        assert_ne!(rec.content_hash, 0);
        assert!(rec.is_valid);
        assert_eq!(rec.instruction_count, 4);
        assert_eq!(rec.push_count, 2);
        assert_eq!(rec.halt_count, 1);
        assert_eq!(rec.call_count, 0);
    }

    #[test]
    fn test_db_module_record_invalid() {
        let prog = invalid_program();
        let rec = wasm_to_db_module_record(&prog);
        assert!(!rec.is_valid);
    }

    #[test]
    fn test_db_module_record_hash_deterministic() {
        let prog = simple_program();
        let a = wasm_to_db_module_record(&prog);
        let b = wasm_to_db_module_record(&prog);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_cache_entry_valid_small() {
        // valid, ≤ 256 → 3600 s
        let prog = simple_program();
        let entry = wasm_to_cache_entry(&prog);
        assert_eq!(entry.ttl_secs, 3600);
        assert!(entry.is_valid);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_cache_entry_invalid_reduced_ttl() {
        // invalid → 3600 − 900 = 2700 s
        let prog = invalid_program();
        let entry = wasm_to_cache_entry(&prog);
        assert!(!entry.is_valid);
        assert_eq!(entry.ttl_secs, 2700);
    }

    #[test]
    fn test_analytics_metrics_success() {
        let prog = simple_program();
        let m = wasm_to_analytics_metrics(&prog, 0, 1000);
        assert_ne!(m.content_hash, 0);
        assert!(m.execution_ok);
        assert_eq!(m.exit_code, 0);
        assert!(m.fuel_used > 0);
        assert!(m.final_stack_depth > 0);
    }

    #[test]
    fn test_analytics_metrics_fuel_exhausted() {
        let prog = vec![
            Opcode::Push(1),
            Opcode::Push(2),
            Opcode::Push(3),
            Opcode::Push(4),
            Opcode::Halt,
        ];
        let m = wasm_to_analytics_metrics(&prog, 0, 2);
        assert!(!m.execution_ok);
        assert_eq!(m.exit_code, 1);
    }

    #[test]
    fn test_cdn_descriptor_valid() {
        let prog = simple_program();
        let desc = wasm_to_cdn_descriptor(&prog);
        assert_ne!(desc.content_hash, 0);
        assert!(desc.is_valid);
        assert_eq!(desc.cdn_ttl_secs, 86_400);
        assert_eq!(desc.estimated_bytes, 4 * 9);
    }

    #[test]
    fn test_cdn_descriptor_invalid_no_cache() {
        let prog = invalid_program();
        let desc = wasm_to_cdn_descriptor(&prog);
        assert!(!desc.is_valid);
        assert_eq!(desc.cdn_ttl_secs, 0);
    }

    #[test]
    fn test_edge_event_success() {
        let prog = simple_program();
        let ev = wasm_to_edge_event(&prog, 0, 1000);
        assert_ne!(ev.content_hash, 0);
        assert!(ev.execution_ok);
        assert_eq!(ev.event_kind, 1); // execution_complete
        assert_eq!(ev.edge_action, 0); // allow
    }

    #[test]
    fn test_edge_event_fuel_exhausted() {
        let prog = vec![
            Opcode::Push(1),
            Opcode::Push(2),
            Opcode::Push(3),
            Opcode::Halt,
        ];
        let ev = wasm_to_edge_event(&prog, 0, 1);
        assert!(!ev.execution_ok);
        assert_eq!(ev.event_kind, 3); // fuel_exhausted
        assert_eq!(ev.edge_action, 1); // retry
    }
}
