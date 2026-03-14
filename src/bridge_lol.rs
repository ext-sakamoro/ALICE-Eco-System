//! LOL bridges — ALICE-LOL DSL ↔ SDF, View, Print, Animation, Analytics
//!
//! 5 bridges connecting the Law-Oriented Language DSL layer to the
//! ALICE ecosystem. LOL テキストから SDF ノードへの変換を起点に、
//! View表示、3Dプリント、アニメーション、解析メトリクスに接続する。

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LOL → SDF (DSL パース結果) ─────────────────────────────────

/// LOL テキストから SDF ツリーへの変換メタデータ
pub struct LolToSdfBridge {
    pub content_hash: u64,
    /// 入力テキストのバイト数
    pub source_bytes: u32,
    /// パース後の SdfNode ノード数
    pub node_count: u32,
    /// トークン効率 (bytes_per_node)
    pub token_efficiency: f32,
}

/// LOL テキスト → SDF 変換メタデータを生成
#[inline]
pub fn lol_to_sdf(lol_text: &str, node_count: u32) -> LolToSdfBridge {
    let src_bytes = lol_text.len() as u32;
    let efficiency = if node_count > 0 {
        src_bytes as f32 / node_count as f32
    } else {
        0.0
    };
    LolToSdfBridge {
        content_hash: fnv1a(lol_text.as_bytes()),
        source_bytes: src_bytes,
        node_count,
        token_efficiency: efficiency,
    }
}

// ── Bridge 2: LOL → View (WGSL トランスパイル記述子) ────────────────────

/// LOL → View パイプライン記述子
pub struct LolToViewBridge {
    pub content_hash: u64,
    /// 生成された WGSL のバイト数
    pub wgsl_bytes: u32,
    /// ヘルパー関数数
    pub helper_count: u32,
    /// マテリアル関数あり
    pub has_material: bool,
    /// Cache TTL (秒)
    pub ttl_secs: u32,
}

/// LOL → View 記述子を生成
#[inline]
pub fn lol_to_view(
    lol_text: &str,
    wgsl_bytes: u32,
    helper_count: u32,
    has_material: bool,
) -> LolToViewBridge {
    // マテリアル付きは変更頻度低い → 長TTL
    let has_mat = has_material as u32;
    let ttl_secs = 300 + has_mat * 600;
    LolToViewBridge {
        content_hash: fnv1a(lol_text.as_bytes()),
        wgsl_bytes,
        helper_count,
        has_material,
        ttl_secs,
    }
}

// ── Bridge 3: LOL → Print (スライス入力記述子) ──────────────────────────

/// LOL → Print パイプライン記述子
pub struct LolToPrintBridge {
    pub content_hash: u64,
    /// ノード数（スライス複雑度の指標）
    pub node_count: u32,
    /// 推定スライス時間カテゴリ (0=instant, 1=fast, 2=moderate, 3=slow)
    pub complexity_tier: u8,
    /// Cache TTL (秒)
    pub ttl_secs: u32,
}

/// LOL → Print 記述子を生成
#[inline]
pub fn lol_to_print(lol_text: &str, node_count: u32) -> LolToPrintBridge {
    let tier = if node_count < 10 {
        0
    } else if node_count < 50 {
        1
    } else if node_count < 200 {
        2
    } else {
        3
    };
    let ttl_secs = 600 - (tier as u32) * 100;
    LolToPrintBridge {
        content_hash: fnv1a(lol_text.as_bytes()),
        node_count,
        complexity_tier: tier,
        ttl_secs,
    }
}

// ── Bridge 4: LOL → Animation (アクター記述子) ──────────────────────────

/// LOL → Animation アクター記述子
pub struct LolToAnimationBridge {
    pub content_hash: u64,
    /// アクター名
    pub actor_name_hash: u64,
    /// ノード数
    pub node_count: u32,
    /// Cache TTL (秒)
    pub ttl_secs: u32,
}

/// LOL → Animation 記述子を生成
#[inline]
pub fn lol_to_animation(lol_text: &str, actor_name: &str, node_count: u32) -> LolToAnimationBridge {
    LolToAnimationBridge {
        content_hash: fnv1a(lol_text.as_bytes()),
        actor_name_hash: fnv1a(actor_name.as_bytes()),
        node_count,
        ttl_secs: 120,
    }
}

// ── Bridge 5: LOL → Analytics (DSL 使用メトリクス) ──────────────────────

/// LOL → Analytics 使用メトリクス
pub struct LolToAnalyticsBridge {
    pub content_hash: u64,
    /// 入力テキストバイト数
    pub source_bytes: u32,
    /// ノード数
    pub node_count: u32,
    /// 使用構文カテゴリ (bitmask: 1=primitives, 2=operations, 4=transforms, 8=modifiers, 16=time, 32=laws)
    pub syntax_categories: u8,
    /// マテリアル使用数
    pub material_count: u8,
}

/// LOL → Analytics メトリクスを生成
#[inline]
pub fn lol_to_analytics(
    lol_text: &str,
    node_count: u32,
    material_count: u8,
) -> LolToAnalyticsBridge {
    // テキストから使用カテゴリを推定
    let src = lol_text.as_bytes();
    let mut cats: u8 = 0;
    // 簡易検出: キーワード出現で判定
    if lol_text.contains("sphere") || lol_text.contains("box3d") || lol_text.contains("cylinder") {
        cats |= 1;
    }
    if lol_text.contains("union")
        || lol_text.contains("subtract")
        || lol_text.contains("intersection")
    {
        cats |= 2;
    }
    if lol_text.contains("translate") || lol_text.contains("rotate") || lol_text.contains("scale") {
        cats |= 4;
    }
    if lol_text.contains("twist")
        || lol_text.contains("bend")
        || lol_text.contains("mirror")
        || lol_text.contains("round")
    {
        cats |= 8;
    }
    if lol_text.contains("animate") || lol_text.contains("morph") {
        cats |= 16;
    }
    if lol_text.contains("NonOverlap")
        || lol_text.contains("Containment")
        || lol_text.contains("MinThickness")
    {
        cats |= 32;
    }
    LolToAnalyticsBridge {
        content_hash: fnv1a(src),
        source_bytes: lol_text.len() as u32,
        node_count,
        syntax_categories: cats,
        material_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lol_to_sdf_basic() {
        let b = lol_to_sdf("sphere(1.0)", 1);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.source_bytes, 11);
        assert_eq!(b.node_count, 1);
        assert!(b.token_efficiency > 0.0);
    }

    #[test]
    fn test_lol_to_sdf_hash_deterministic() {
        let h1 = lol_to_sdf("sphere(1.0)", 1).content_hash;
        let h2 = lol_to_sdf("sphere(1.0)", 1).content_hash;
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_lol_to_view() {
        let b = lol_to_view("sphere(1.0)", 256, 0, false);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.wgsl_bytes, 256);
        assert_eq!(b.ttl_secs, 300); // no material
    }

    #[test]
    fn test_lol_to_view_with_material() {
        let b = lol_to_view("with_material(1.0, sphere(1.0))", 512, 1, true);
        assert_eq!(b.ttl_secs, 900); // 300 + 600
        assert!(b.has_material);
    }

    #[test]
    fn test_lol_to_print() {
        let b = lol_to_print("sphere(1.0)", 1);
        assert_eq!(b.complexity_tier, 0); // instant
        assert_eq!(b.ttl_secs, 600);
    }

    #[test]
    fn test_lol_to_print_complex() {
        let b = lol_to_print("complex scene", 250);
        assert_eq!(b.complexity_tier, 3); // slow
        assert_eq!(b.ttl_secs, 300);
    }

    #[test]
    fn test_lol_to_animation() {
        let b = lol_to_animation("sphere(1.0)", "hero", 1);
        assert_ne!(b.content_hash, 0);
        assert_ne!(b.actor_name_hash, 0);
        assert_eq!(b.ttl_secs, 120);
    }

    #[test]
    fn test_lol_to_analytics() {
        let b = lol_to_analytics(
            "union(translate(0.0, 1.0, 0.0, sphere(1.0)), box3d(0.5, 0.5, 0.5))",
            3,
            0,
        );
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.syntax_categories & 1, 1); // primitives
        assert_eq!(b.syntax_categories & 2, 2); // operations
        assert_eq!(b.syntax_categories & 4, 4); // transforms
    }

    #[test]
    fn test_lol_to_analytics_empty() {
        let b = lol_to_analytics("custom_thing(1.0)", 1, 0);
        assert_eq!(b.syntax_categories, 0);
    }
}
