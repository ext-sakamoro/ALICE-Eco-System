//! Cross-domain bridges — ALICE-WASM ↔ Container, FFI
//!
//! 5 bridges connecting Wasm module/function/memory descriptors to
//! Container image/resource specs, FFI symbol descriptors, and Cache.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Wasm module → Container image descriptor ─────────────────

/// Container image descriptor derived from a Wasm module.
///
/// Maps a Wasm module's bytecode hash, instruction count, and fuel
/// limit into a container image descriptor so the Container runtime
/// can provision an appropriately-sized sandbox without directly
/// depending on the Wasm crate.
pub struct WasmContainerImage {
    /// FNV-1a hash over module_hash + instruction_count + fuel bytes.
    pub content_hash: u64,
    /// Hash of the Wasm bytecode — used as the image ID.
    pub module_hash: u64,
    /// Number of instructions (opcodes) in the module.
    pub instruction_count: usize,
    /// Fuel limit from the VM configuration.
    pub fuel_limit: u64,
    /// Estimated CPU quota in microseconds (1 us per instruction, capped).
    pub cpu_quota_us: u64,
    /// Default memory limit for the container (64 KB per Wasm page).
    pub memory_limit_bytes: u64,
}

/// Convert a Wasm module into a Container image descriptor.
///
/// `bytecode`: the raw opcode slice representing the module.
/// `fuel_limit`: the gas/fuel budget for the VM.
/// `memory_pages`: number of Wasm memory pages (each 64 KB).
#[inline]
#[must_use]
pub fn wasm_module_to_container_image(
    bytecode: &[u8],
    instruction_count: usize,
    fuel_limit: u64,
    memory_pages: u32,
) -> WasmContainerImage {
    let module_hash = fnv1a(bytecode);
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&module_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(instruction_count as u64).to_le_bytes());
    key[16..24].copy_from_slice(&fuel_limit.to_le_bytes());

    // 1 us per instruction, capped at default CPU period (100ms)
    let cpu_quota_us = (instruction_count as u64).min(100_000);
    // 64 KB per Wasm page
    let memory_limit_bytes = memory_pages as u64 * 65_536;

    WasmContainerImage {
        content_hash: fnv1a(&key),
        module_hash,
        instruction_count,
        fuel_limit,
        cpu_quota_us,
        memory_limit_bytes,
    }
}

// ── Bridge 2: Wasm function export → FFI symbol descriptor ─────────────

/// FFI symbol descriptor derived from a Wasm function export.
///
/// Maps a Wasm function's name and address (instruction offset) into
/// an FFI symbol so the FFI layer can register host callbacks and
/// resolve function pointers without direct Wasm crate coupling.
pub struct WasmFfiSymbol {
    /// FNV-1a hash over function name + address + arity bytes.
    pub content_hash: u64,
    /// Hash of the function name — used for FFI symbol lookup.
    pub name_hash: u64,
    /// Function entry point (instruction offset in the bytecode).
    pub entry_offset: usize,
    /// Number of parameters the function accepts.
    pub param_count: u32,
    /// FFI calling convention: 0=CDecl (default for Wasm exports).
    pub calling_convention: u8,
    /// Estimated callback registration ID (hash-based).
    pub callback_id: u32,
}

/// Convert a Wasm function export into an FFI symbol descriptor.
#[inline]
#[must_use]
pub fn wasm_function_to_ffi_symbol(
    function_name: &str,
    entry_offset: usize,
    param_count: u32,
) -> WasmFfiSymbol {
    let name_hash = fnv1a(function_name.as_bytes());
    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&name_hash.to_le_bytes());
    key[8..16].copy_from_slice(&(entry_offset as u64).to_le_bytes());
    key[16..20].copy_from_slice(&param_count.to_le_bytes());

    // callback_id: lower 32 bits of name hash
    let callback_id = name_hash as u32;

    WasmFfiSymbol {
        content_hash: fnv1a(&key),
        name_hash,
        entry_offset,
        param_count,
        calling_convention: 0, // CDecl
        callback_id,
    }
}

// ── Bridge 3: Wasm memory limits → Container resource limits ───────────

/// Container resource limits derived from Wasm memory configuration.
///
/// Maps a Wasm sandbox's memory size and fuel limit into container
/// cgroup resource limits (memory.max, cpu.max) so the Container
/// runtime can enforce consistent resource boundaries.
pub struct WasmContainerResource {
    /// FNV-1a hash over memory_bytes + fuel + locals bytes.
    pub content_hash: u64,
    /// Wasm sandbox memory size in bytes.
    pub wasm_memory_bytes: usize,
    /// Container memory limit (wasm_memory_bytes + 1 MB overhead).
    pub container_memory_bytes: u64,
    /// Fuel limit from the VM.
    pub fuel_limit: u64,
    /// Container CPU quota in microseconds (fuel / 10, capped at 100ms).
    pub cpu_quota_us: u64,
    /// Number of Wasm local variable slots.
    pub local_count: usize,
}

/// Convert Wasm memory and fuel limits into Container resource limits.
#[inline]
#[must_use]
pub fn wasm_memory_to_container_resource(
    memory_bytes: usize,
    fuel_limit: u64,
    local_count: usize,
) -> WasmContainerResource {
    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&(memory_bytes as u64).to_le_bytes());
    key[8..16].copy_from_slice(&fuel_limit.to_le_bytes());
    key[16..24].copy_from_slice(&(local_count as u64).to_le_bytes());

    // Container memory = Wasm memory + 1 MB overhead for runtime
    let container_memory_bytes = memory_bytes as u64 + 1_048_576;
    // CPU quota: fuel / 10, capped at default period (100_000 us)
    let cpu_quota_us = (fuel_limit / 10).min(100_000);

    WasmContainerResource {
        content_hash: fnv1a(&key),
        wasm_memory_bytes: memory_bytes,
        container_memory_bytes,
        fuel_limit,
        cpu_quota_us,
        local_count,
    }
}

// ── Bridge 4: Container config → Wasm runtime config ───────────────────

/// Wasm runtime configuration derived from Container config.
///
/// Maps container CPU/memory limits back into Wasm VM parameters
/// (fuel, memory pages, local slots) so a Wasm module can be
/// launched with limits consistent with its container.
pub struct ContainerWasmConfig {
    /// FNV-1a hash over cpu_quota + memory_limit + network bytes.
    pub content_hash: u64,
    /// Fuel budget derived from CPU quota (quota_us * 10).
    pub fuel: u64,
    /// Number of Wasm memory pages (container memory / 64KB, capped at 1024).
    pub memory_pages: u32,
    /// Number of local variable slots (fixed at 256 for container VMs).
    pub num_locals: usize,
    /// CPU quota from container config in microseconds.
    pub cpu_quota_us: u64,
    /// Memory limit from container config in bytes.
    pub memory_limit_bytes: u64,
}

/// Convert Container config parameters into a Wasm runtime config.
#[inline]
#[must_use]
pub fn container_image_to_wasm_config(
    cpu_quota_us: u64,
    memory_limit_bytes: u64,
) -> ContainerWasmConfig {
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&cpu_quota_us.to_le_bytes());
    key[8..16].copy_from_slice(&memory_limit_bytes.to_le_bytes());

    // Fuel = cpu_quota * 10 (inverse of the resource bridge mapping)
    let fuel = cpu_quota_us * 10;
    // Memory pages = memory_limit / 64KB, capped at 1024 pages (64 MB)
    let memory_pages = ((memory_limit_bytes / 65_536) as u32).min(1024);

    ContainerWasmConfig {
        content_hash: fnv1a(&key),
        fuel,
        memory_pages,
        num_locals: 256,
        cpu_quota_us,
        memory_limit_bytes,
    }
}

// ── Bridge 5: Wasm module → Cache ──────────────────────────────────────

/// Cache entry for a compiled Wasm module.
///
/// Caches the compilation result of a Wasm module with branchless
/// TTL based on module size: large modules (>1000 instructions) get
/// longer TTL because recompilation is expensive.
pub struct WasmModuleCache {
    /// FNV-1a hash over module bytecode.
    pub content_hash: u64,
    /// Number of instructions in the module.
    pub instruction_count: usize,
    /// Fuel limit for the module.
    pub fuel_limit: u64,
    /// Cache TTL in seconds (branchless: large=600s, small=60s).
    pub ttl_secs: u32,
    /// Estimated cache entry size in bytes.
    pub entry_bytes: usize,
}

/// Build a cache entry for a compiled Wasm module.
///
/// `large_threshold`: modules with more instructions than this get
/// longer TTL. Computed branchlessly: `base - condition * delta`.
#[inline]
#[must_use]
pub fn wasm_module_to_cache(
    bytecode: &[u8],
    instruction_count: usize,
    fuel_limit: u64,
    large_threshold: usize,
) -> WasmModuleCache {
    let content_hash = fnv1a(bytecode);

    // Branchless TTL: large modules → 600s, small → 60s
    // base=600, delta=540 → large(1): 600 - 0*540 = 600, small(0): 600 - 1*540 = 60
    let is_large = (instruction_count > large_threshold) as u32;
    let ttl_secs = 600u32 - (1 - is_large) * 540u32;

    WasmModuleCache {
        content_hash,
        instruction_count,
        fuel_limit,
        ttl_secs,
        // 各 opcode ≈ 16 bytes in memory + overhead
        entry_bytes: instruction_count * 16 + 64,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Bridge 1: Wasm module → container image

    #[test]
    fn test_wasm_to_container_image_basic() {
        let bytecode = b"wasm_bytecode_sample";
        let img = wasm_module_to_container_image(bytecode, 500, 10_000, 4);
        assert_ne!(img.content_hash, 0);
        assert_eq!(img.instruction_count, 500);
        assert_eq!(img.fuel_limit, 10_000);
        assert_eq!(img.cpu_quota_us, 500); // min(500, 100_000)
        assert_eq!(img.memory_limit_bytes, 4 * 65_536);
    }

    #[test]
    fn test_wasm_to_container_image_cpu_cap() {
        let bytecode = b"large_module";
        let img = wasm_module_to_container_image(bytecode, 200_000, 1_000_000, 16);
        // CPU capped at 100_000 us
        assert_eq!(img.cpu_quota_us, 100_000);
    }

    #[test]
    fn test_wasm_to_container_image_deterministic() {
        let bytecode = b"determinism_test";
        let a = wasm_module_to_container_image(bytecode, 100, 5000, 2);
        let b = wasm_module_to_container_image(bytecode, 100, 5000, 2);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.module_hash, b.module_hash);
    }

    // Bridge 2: Wasm function → FFI symbol

    #[test]
    fn test_wasm_to_ffi_symbol_basic() {
        let sym = wasm_function_to_ffi_symbol("my_export", 42, 3);
        assert_ne!(sym.content_hash, 0);
        assert_eq!(sym.entry_offset, 42);
        assert_eq!(sym.param_count, 3);
        assert_eq!(sym.calling_convention, 0); // CDecl
    }

    #[test]
    fn test_wasm_to_ffi_symbol_callback_id() {
        let sym = wasm_function_to_ffi_symbol("init", 0, 0);
        // callback_id should be lower 32 bits of name_hash
        assert_eq!(sym.callback_id, sym.name_hash as u32);
    }

    // Bridge 3: Wasm memory → container resource

    #[test]
    fn test_wasm_memory_to_resource_basic() {
        let res = wasm_memory_to_container_resource(65_536, 50_000, 16);
        assert_ne!(res.content_hash, 0);
        assert_eq!(res.wasm_memory_bytes, 65_536);
        // container memory = 65536 + 1MB
        assert_eq!(res.container_memory_bytes, 65_536 + 1_048_576);
        // cpu = 50000/10 = 5000, below cap
        assert_eq!(res.cpu_quota_us, 5_000);
        assert_eq!(res.local_count, 16);
    }

    #[test]
    fn test_wasm_memory_to_resource_cpu_cap() {
        let res = wasm_memory_to_container_resource(1024, 5_000_000, 8);
        // cpu = 5_000_000/10 = 500_000, capped at 100_000
        assert_eq!(res.cpu_quota_us, 100_000);
    }

    // Bridge 4: Container config → Wasm config

    #[test]
    fn test_container_to_wasm_config_basic() {
        let cfg = container_image_to_wasm_config(50_000, 256 * 65_536);
        assert_ne!(cfg.content_hash, 0);
        assert_eq!(cfg.fuel, 500_000); // 50000 * 10
        assert_eq!(cfg.memory_pages, 256);
        assert_eq!(cfg.num_locals, 256);
        assert_eq!(cfg.cpu_quota_us, 50_000);
    }

    #[test]
    fn test_container_to_wasm_config_page_cap() {
        // 128 MB → 2048 pages, but capped at 1024
        let cfg = container_image_to_wasm_config(100_000, 128 * 1024 * 1024);
        assert_eq!(cfg.memory_pages, 1024);
    }

    // Bridge 5: Wasm module → cache

    #[test]
    fn test_wasm_module_cache_large_ttl() {
        let bytecode = b"large_module_bytecode";
        let entry = wasm_module_to_cache(bytecode, 2000, 100_000, 1000);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 600); // large → 600s
        assert_eq!(entry.instruction_count, 2000);
    }

    #[test]
    fn test_wasm_module_cache_small_ttl() {
        let bytecode = b"tiny_module";
        let entry = wasm_module_to_cache(bytecode, 50, 500, 1000);
        assert_eq!(entry.ttl_secs, 60); // small → 60s
    }

    #[test]
    fn test_wasm_module_cache_deterministic() {
        let bytecode = b"cache_test";
        let a = wasm_module_to_cache(bytecode, 100, 1000, 500);
        let b = wasm_module_to_cache(bytecode, 100, 1000, 500);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
