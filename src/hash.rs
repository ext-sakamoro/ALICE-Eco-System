//! Shared FNV-1a hash helper — single optimization point for all bridges.
//!
//! Used by every bridge for content hashing, cache keying, and deduplication.
//! `#[inline(always)]` ensures zero call overhead after LTO.

/// FNV-1a hash of a byte slice (64-bit).
///
/// Single implementation shared across all bridge files.
/// Marked `#[inline(always)]` for zero-overhead inlining into callers.
#[inline(always)]
#[must_use]
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_deterministic() {
        let a = fnv1a(b"hello");
        let b = fnv1a(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn test_fnv1a_different_inputs() {
        assert_ne!(fnv1a(b"hello"), fnv1a(b"world"));
    }

    #[test]
    fn test_fnv1a_empty() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
    }
}
