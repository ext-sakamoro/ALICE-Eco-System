# ALICE-Eco-System — Claude Code 設定

## プロジェクト概要

全ALICEクレートをAnalytics/DB/Edge/Cacheバックエンドに接続するブリッジクレート。

| 項目 | 値 |
|------|-----|
| リポジトリ | `ext-sakamoro/ALICE-Eco-System` |
| リモート | `origin` (`git@github.com:ext-sakamoro/ALICE-Eco-System.git`) |
| ブランチ | `main` |
| バージョン | v0.3.2 |
| ライセンス | MIT |
| ブリッジ数 | 1330 (238モジュール, 185クレート + ALICE-GameEngine + ALICE-Shader) |
| テスト数 | 2491 |

## コーディングルール

- コミットメッセージ: 日本語
- コード内コメント: 日本語（ただしドキュメントコメントは英語も可）
- 署名禁止: Co-Authored-By / Generated with Claude Code は一切追加しない
- 作成者名: `Moroya Sakamoto`

## ブリッジファイル設計ルール（必須）

### 1. ファイル構成パターン

各ブリッジファイルは以下の構造を必ず持つ:
- ファイル冒頭: `//!` ドキュメントコメント（ブリッジ数と接続先を明記）
- ファイルローカル `fnv1a()` 関数（`#[inline(always)]`）
- 5つのブリッジ変換関数
- `#[cfg(test)] mod tests` （最低8テスト）

### 2. FNV-1a ハッシュ

```rust
#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x0100_0000_01b3); }
    h
}
```
- 各ブリッジファイルにファイルローカルで定義（クレート共有にしない）
- 定数: basis=`0xcbf2_9ce4_8422_2325`, prime=`0x0100_0000_01b3`

### 3. ブリッジ構造体

- 全フィールドは `pub`
- 先頭フィールドは必ず `content_hash: u64`
- content_hash: 入力の決定的ハッシュ（同一入力→同一ハッシュ）

### 4. 変換関数

- `#[inline]` で修飾（`#[inline(always)]` ではない）
- 引数: 元クレートの型への参照 (`&SourceType`)
- 戻り値: ブリッジ構造体（`Option<T>` も可）
- 命名: `{domain}_{source}_to_{target}` パターン

### 5. Branchless TTL（Cache向け）

```rust
let condition = (some_check) as u32;
let ttl_secs = base - condition * delta;
```

### 6. Enum → u8 マッピング

`match` で明示的にマップ（`as u8` キャスト禁止）

### 7. テスト要件（最低8テスト/ファイル）

- 各変換関数の基本テスト（`content_hash != 0` + フィールド値検証）
- Cache TTLの正常値/異常値テスト
- ハッシュ決定性テスト

## 新クレート追加チェックリスト

1. **実際のAPIを確認**: `lib.rs` を読み、推測でブリッジを書かない
2. **ブリッジファイル作成**: `src/bridge_xxx.rs`（5 bridges/file）
3. **Cargo.toml更新**: private/proprietaryクレートは `optional = true`
4. **lib.rs更新**: `pub mod bridge_xxx;`（optionalは `#[cfg(feature)]` ガード）
5. **Path追加**: lib.rs冒頭docコメントにパイプラインパス追加
6. **re-export**: lib.rs末尾に代表的な型のre-export追加
7. **features更新**: Cargo.tomlの`[features]`セクション更新

## ALICE 品質基準

ALICE-KARIKARI.md「100/100品質基準」参照。clippy基準: `pedantic+nursery`

| 指標 | 値 |
|------|-----|
| clippy (pedantic+nursery) | 0 warnings |
| テスト数 | 2295 |
| fmt | clean |

## ALICE関連リポジトリとの連携

本クレートは全ALICEクレートのブリッジ。新しいALICEクレートが作成されたら:
1. そのクレートの`lib.rs`を読んでAPIを把握
2. ブリッジファイルを上記パターンで作成
3. Cargo.toml/lib.rsを更新

## 情報更新ルール

- 新ブリッジ追加時: ブリッジ数・テスト数・バージョンを更新
- 依存ALICEクレートのAPI変更時: 影響するブリッジの変更点をメモ
- 品質基準の達成状況が変わったら更新（warning数、テスト数等）
- optionalクレートのfeature追加時: チェックリストの「features更新」を実行
