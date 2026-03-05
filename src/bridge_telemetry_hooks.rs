//! Bridge telemetry hooks — optional instrumentation for bridge calls.
//!
//! Records per-bridge invocation count and cumulative latency.
//! Enable for production monitoring, disable for zero-overhead in benchmarks.

use std::collections::HashMap;

// ── BridgeTelemetry ───────────────────────────────────────────────────────

/// Per-bridge invocation counter and cumulative latency recorder.
///
/// Tracks call counts and total latency (in nanoseconds) keyed by bridge name.
/// Designed for low-overhead production monitoring; all operations are O(1) amortized.
pub struct BridgeTelemetry {
    /// ブリッジ名ごとの呼び出し回数
    call_counts: HashMap<String, u64>,
    /// ブリッジ名ごとの累積レイテンシ（ナノ秒）
    total_latency_ns: HashMap<String, u64>,
}

impl BridgeTelemetry {
    /// Create an empty telemetry recorder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            call_counts: HashMap::new(),
            total_latency_ns: HashMap::new(),
        }
    }

    /// Record a single bridge invocation with its observed latency.
    ///
    /// `bridge_name` — identifier of the bridge being instrumented.
    /// `latency_ns` — wall-clock duration of the call in nanoseconds.
    pub fn record_call(&mut self, bridge_name: &str, latency_ns: u64) {
        // 呼び出し回数をインクリメント
        *self.call_counts.entry(bridge_name.to_string()).or_insert(0) += 1;
        // 累積レイテンシを加算
        *self
            .total_latency_ns
            .entry(bridge_name.to_string())
            .or_insert(0) += latency_ns;
    }

    /// Retrieve statistics for a single bridge.
    ///
    /// Returns `Some((call_count, total_latency_ns))` if the bridge has been
    /// recorded at least once, or `None` if it has never been called.
    #[must_use]
    pub fn get_stats(&self, bridge_name: &str) -> Option<(u64, u64)> {
        let count = self.call_counts.get(bridge_name).copied()?;
        let latency = self.total_latency_ns.get(bridge_name).copied().unwrap_or(0);
        Some((count, latency))
    }

    /// Return the top `n` bridges ranked by call count (descending).
    ///
    /// Each element is `(bridge_name, call_count, total_latency_ns)`.
    /// If fewer than `n` bridges have been recorded, all are returned.
    #[must_use]
    pub fn top_bridges(&self, n: usize) -> Vec<(String, u64, u64)> {
        // 呼び出し回数の降順でソートした上位Nブリッジを返す
        let mut entries: Vec<(String, u64, u64)> = self
            .call_counts
            .iter()
            .map(|(name, &count)| {
                let latency = self.total_latency_ns.get(name).copied().unwrap_or(0);
                (name.clone(), count, latency)
            })
            .collect();
        // 呼び出し回数の降順でソート（同数の場合はブリッジ名で安定ソート）
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(n);
        entries
    }

    /// Reset all counters and latency accumulators.
    pub fn reset(&mut self) {
        self.call_counts.clear();
        self.total_latency_ns.clear();
    }

    /// Serialize statistics to a compact JSON string.
    ///
    /// Format: `{"bridges":[{"name":"...","calls":N,"total_ns":M},...]}`.
    /// Bridges are sorted by call count descending for deterministic output.
    #[must_use]
    pub fn to_json(&self) -> String {
        // 呼び出し回数の降順でブリッジをソートして JSON 文字列を生成
        let mut entries: Vec<(&String, u64, u64)> = self
            .call_counts
            .iter()
            .map(|(name, &count)| {
                let latency = self.total_latency_ns.get(name).copied().unwrap_or(0);
                (name, count, latency)
            })
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let mut json = String::from("{\"bridges\":[");
        for (i, (name, count, latency)) in entries.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            // JSON特殊文字のエスケープ（ブリッジ名は英数字+アンダースコアのみ想定）
            json.push_str(&format!(
                "{{\"name\":\"{name}\",\"calls\":{count},\"total_ns\":{latency}}}"
            ));
        }
        json.push_str("]}");
        json
    }
}

impl Default for BridgeTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_call_increments_count() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_edge", 500);
        t.record_call("bridge_edge", 300);
        let (count, total_ns) = t.get_stats("bridge_edge").unwrap();
        assert_eq!(count, 2);
        assert_eq!(total_ns, 800);
    }

    #[test]
    fn test_get_stats_none_for_unknown_bridge() {
        let t = BridgeTelemetry::new();
        assert!(t.get_stats("bridge_nonexistent").is_none());
    }

    #[test]
    fn test_get_stats_single_call() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_auth", 1_000_000);
        let (count, total_ns) = t.get_stats("bridge_auth").unwrap();
        assert_eq!(count, 1);
        assert_eq!(total_ns, 1_000_000);
    }

    #[test]
    fn test_top_bridges_ranked_by_call_count() {
        let mut t = BridgeTelemetry::new();
        // bridge_cdn を最多呼び出し
        t.record_call("bridge_cdn", 100);
        t.record_call("bridge_cdn", 200);
        t.record_call("bridge_cdn", 150);
        // bridge_db を2番目
        t.record_call("bridge_db", 50);
        t.record_call("bridge_db", 75);
        // bridge_auth を最少
        t.record_call("bridge_auth", 10);

        let top = t.top_bridges(2);
        assert_eq!(top.len(), 2);
        // 最多呼び出しが先頭
        assert_eq!(top[0].0, "bridge_cdn");
        assert_eq!(top[0].1, 3); // 3回呼び出し
        assert_eq!(top[0].2, 450); // 累積レイテンシ
        assert_eq!(top[1].0, "bridge_db");
        assert_eq!(top[1].1, 2);
    }

    #[test]
    fn test_top_bridges_fewer_than_n() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_only", 999);
        // n=5 を要求しても登録済みブリッジ数（1件）しか返さない
        let top = t.top_bridges(5);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, "bridge_only");
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_sdf", 1_000);
        t.record_call("bridge_physics", 2_000);
        t.reset();
        assert!(t.get_stats("bridge_sdf").is_none());
        assert!(t.get_stats("bridge_physics").is_none());
        assert!(t.top_bridges(10).is_empty());
    }

    #[test]
    fn test_to_json_structure() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_a", 100);
        t.record_call("bridge_a", 200);
        t.record_call("bridge_b", 50);
        let json = t.to_json();
        // JSON形式の検証
        assert!(json.starts_with("{\"bridges\":["));
        assert!(json.ends_with("]}"));
        // 両ブリッジが含まれる
        assert!(json.contains("bridge_a"));
        assert!(json.contains("bridge_b"));
        // 呼び出し回数が含まれる
        assert!(json.contains("\"calls\":2"));
        assert!(json.contains("\"calls\":1"));
    }

    #[test]
    fn test_to_json_empty() {
        let t = BridgeTelemetry::new();
        // 空のテレメトリは空配列を返す
        assert_eq!(t.to_json(), "{\"bridges\":[]}");
    }

    #[test]
    fn test_multiple_bridges_independent() {
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_x", 1_000);
        t.record_call("bridge_y", 2_000);
        t.record_call("bridge_x", 500);

        let (cx, lx) = t.get_stats("bridge_x").unwrap();
        let (cy, ly) = t.get_stats("bridge_y").unwrap();
        assert_eq!(cx, 2);
        assert_eq!(lx, 1_500);
        assert_eq!(cy, 1);
        assert_eq!(ly, 2_000);
    }

    #[test]
    fn test_zero_latency_call() {
        // レイテンシ0のコールも正しく記録できる
        let mut t = BridgeTelemetry::new();
        t.record_call("bridge_fast", 0);
        t.record_call("bridge_fast", 0);
        let (count, total_ns) = t.get_stats("bridge_fast").unwrap();
        assert_eq!(count, 2);
        assert_eq!(total_ns, 0);
    }
}
