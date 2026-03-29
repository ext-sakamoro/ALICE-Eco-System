//! Shader bridges — ALICE-Shader ↔ DB, Cache, Analytics, Render, CDN
//!
//! 5 bridges connecting unified shader library (GLSL+WGSL) to the ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Shader → DB (shader source persistence) ──────────────────

/// シェーダーソースのDB永続化レコード。
pub struct ShaderDbRecord {
    pub content_hash: u64,
    pub name_hash: u64,
    pub lang: u8,
    pub source_bytes: u64,
    pub line_count: u64,
    pub timestamp_ms: u64,
}

/// シェーダーソースをDBレコードに変換。
#[inline]
#[must_use]
pub fn shader_to_db_record(
    name: &str,
    lang: u8,
    source_bytes: u64,
    line_count: u64,
    timestamp_ms: u64,
) -> ShaderDbRecord {
    let name_hash = fnv1a(name.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&source_bytes.to_le_bytes());
    ShaderDbRecord {
        content_hash: fnv1a(&buf),
        name_hash,
        lang,
        source_bytes,
        line_count,
        timestamp_ms,
    }
}

// ── Bridge 2: Shader → Cache (compiled shader caching) ─────────────────

/// コンパイル済みシェーダーのキャッシュエントリ。
pub struct ShaderCacheEntry {
    pub content_hash: u64,
    pub name_hash: u64,
    pub compiled_bytes: u64,
    pub ttl_secs: u32,
}

/// シェーダーキャッシュエントリを生成。
#[inline]
#[must_use]
pub fn shader_to_cache_entry(name: &str, compiled_bytes: u64) -> ShaderCacheEntry {
    let name_hash = fnv1a(name.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&compiled_bytes.to_le_bytes());
    // シェーダーは頻繁に変わらないので長TTL
    let ttl_secs = 86400_u32;
    ShaderCacheEntry {
        content_hash: fnv1a(&buf),
        name_hash,
        compiled_bytes,
        ttl_secs,
    }
}

// ── Bridge 3: Shader → Analytics (usage tracking) ──────────────────────

/// シェーダー使用状況のアナリティクスレコード。
pub struct ShaderAnalyticsRecord {
    pub content_hash: u64,
    pub name_hash: u64,
    pub compile_count: u64,
    pub total_compile_time_us: u64,
    pub error_count: u64,
}

/// シェーダー使用状況をアナリティクスレコードに変換。
#[inline]
#[must_use]
pub fn shader_to_analytics_record(
    name: &str,
    compile_count: u64,
    total_compile_time_us: u64,
    error_count: u64,
) -> ShaderAnalyticsRecord {
    let name_hash = fnv1a(name.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&compile_count.to_le_bytes());
    ShaderAnalyticsRecord {
        content_hash: fnv1a(&buf),
        name_hash,
        compile_count,
        total_compile_time_us,
        error_count,
    }
}

// ── Bridge 4: Shader → Render (pipeline binding) ───────────────────────

/// レンダーパイプラインへのシェーダーバインド情報。
pub struct ShaderRenderRecord {
    pub content_hash: u64,
    pub vertex_hash: u64,
    pub fragment_hash: u64,
    pub pipeline_id: u64,
    pub bind_group_count: u64,
}

/// シェーダーペアをレンダーパイプラインレコードに変換。
#[inline]
#[must_use]
pub fn shader_to_render_record(
    vertex_name: &str,
    fragment_name: &str,
    pipeline_id: u64,
    bind_group_count: u64,
) -> ShaderRenderRecord {
    let vertex_hash = fnv1a(vertex_name.as_bytes());
    let fragment_hash = fnv1a(fragment_name.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&vertex_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&fragment_hash.to_le_bytes());
    ShaderRenderRecord {
        content_hash: fnv1a(&buf),
        vertex_hash,
        fragment_hash,
        pipeline_id,
        bind_group_count,
    }
}

// ── Bridge 5: Shader → CDN (shader distribution) ──────────────────────

/// CDN配信用シェーダーレコード。
pub struct ShaderCdnRecord {
    pub content_hash: u64,
    pub name_hash: u64,
    pub compressed_bytes: u64,
    pub version: u64,
}

/// シェーダーをCDN配信レコードに変換。
#[inline]
#[must_use]
pub fn shader_to_cdn_record(name: &str, compressed_bytes: u64, version: u64) -> ShaderCdnRecord {
    let name_hash = fnv1a(name.as_bytes());
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&compressed_bytes.to_le_bytes());
    buf[16..24].copy_from_slice(&version.to_le_bytes());
    ShaderCdnRecord {
        content_hash: fnv1a(&buf),
        name_hash,
        compressed_bytes,
        version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_nonzero() {
        let rec = shader_to_db_record("sky", 0, 1024, 50, 1_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.lang, 0);
    }

    #[test]
    fn db_record_deterministic() {
        let a = shader_to_db_record("pbr", 1, 2048, 100, 0);
        let b = shader_to_db_record("pbr", 1, 2048, 100, 0);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn cache_entry_ttl() {
        let entry = shader_to_cache_entry("gbuffer", 4096);
        assert_eq!(entry.ttl_secs, 86400);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn analytics_record_fields() {
        let rec = shader_to_analytics_record("sdf_raymarch", 10, 50_000, 1);
        assert_eq!(rec.compile_count, 10);
        assert_eq!(rec.error_count, 1);
    }

    #[test]
    fn render_record_fields() {
        let rec = shader_to_render_record("gbuffer_vertex", "gbuffer_fragment", 42, 2);
        assert_eq!(rec.pipeline_id, 42);
        assert_ne!(rec.vertex_hash, rec.fragment_hash);
    }

    #[test]
    fn cdn_record_version() {
        let rec = shader_to_cdn_record("sky", 512, 3);
        assert_eq!(rec.version, 3);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn different_shaders_different_hashes() {
        let a = shader_to_db_record("sky", 0, 100, 10, 0);
        let b = shader_to_db_record("pbr", 0, 100, 10, 0);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn name_hash_consistency() {
        let a = shader_to_db_record("noise", 0, 50, 5, 0);
        let b = shader_to_cache_entry("noise", 50);
        assert_eq!(a.name_hash, b.name_hash);
    }
}
