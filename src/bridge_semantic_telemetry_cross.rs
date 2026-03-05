//! Semantic Telemetry cross-domain bridges — ALICE-Semantic-Telemetry ↔ Risk, Legal, CDN,
//!                                            Settlement, Container
//!
//! 5 bridges connecting semantic telemetry to cross-domain ALICE subsystems.

use alice_semantic_telemetry::{EventKind, SemanticRing, Severity};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Telemetry → Risk (異常検知からリスクアラートを生成) ─────────

/// Risk alert record derived from a semantic telemetry anomaly batch.
pub struct TelemetryRiskAlert {
    /// Content hash (FNV-1a of severity + anomaly_score bits + sensor_count).
    pub content_hash: u64,
    /// Alert severity level (0 = low … 255 = critical).
    pub severity_level: u8,
    /// Normalized anomaly score in [0.0, 1.0].
    pub anomaly_score: f64,
    /// Number of sensors that contributed to this alert.
    pub sensor_count: u32,
    /// Timestamp of the alert generation (nanoseconds).
    pub timestamp_ns: u64,
}

/// Build a risk alert from a semantic ring's anomaly statistics.
///
/// `severity_level` is caller-supplied (e.g. derived from SLA tier).
/// `anomaly_score` should be in [0.0, 1.0].
#[inline]
#[must_use]
pub fn telemetry_ring_to_risk_alert(
    ring: &SemanticRing,
    severity_level: u8,
    anomaly_score: f64,
    sensor_count: u32,
    timestamp_ns: u64,
) -> TelemetryRiskAlert {
    // リングの異常イベント数をコンテンツハッシュに組み込む
    let kind_counts = ring.count_by_kind();
    let anomaly_count = kind_counts[EventKind::AnomalyDetected as usize];
    let score_bits = anomaly_score.to_bits();
    let mut hash_data = [0u8; 8 + 8 + 8 + 4 + 4];
    hash_data[..8].copy_from_slice(&score_bits.to_le_bytes());
    hash_data[8..16].copy_from_slice(&anomaly_count.to_le_bytes());
    hash_data[16..24].copy_from_slice(&timestamp_ns.to_le_bytes());
    hash_data[24..28].copy_from_slice(&sensor_count.to_le_bytes());
    hash_data[28..32].copy_from_slice(&(severity_level as u32).to_le_bytes());
    TelemetryRiskAlert {
        content_hash: fnv1a(&hash_data),
        severity_level,
        anomaly_score,
        sensor_count,
        timestamp_ns,
    }
}

// ── Bridge 2: Telemetry → Legal (テレメトリイベントからコンプライアンス監査レコードを生成) ──

/// Compliance audit record derived from a batch of semantic telemetry events.
pub struct TelemetryLegalAuditRecord {
    /// Content hash (FNV-1a of event_count + first_timestamp + last_timestamp + source_system_hash).
    pub content_hash: u64,
    /// Total number of events captured in the audit window.
    pub event_count: u64,
    /// Timestamp of the first event in the audit window (nanoseconds).
    pub first_timestamp_ns: u64,
    /// Timestamp of the last event in the audit window (nanoseconds).
    pub last_timestamp_ns: u64,
    /// FNV-1a hash identifying the source system.
    pub source_system_hash: u64,
}

/// Build a legal audit record by scanning the semantic ring.
///
/// The audit window is defined by the events currently in `ring`.
/// `source_system_hash` should be computed by the caller (e.g. fnv1a of crate name bytes).
#[inline]
#[must_use]
pub fn telemetry_ring_to_legal_audit_record(
    ring: &SemanticRing,
    source_system_hash: u64,
) -> TelemetryLegalAuditRecord {
    // イベント数をカウントし、最初と最後のタイムスタンプをスキャンする
    let mut event_count: u64 = 0;
    let mut first_ts: u64 = u64::MAX;
    let mut last_ts: u64 = 0;
    for ev in ring.iter() {
        event_count += 1;
        if ev.timestamp_ns < first_ts {
            first_ts = ev.timestamp_ns;
        }
        if ev.timestamp_ns > last_ts {
            last_ts = ev.timestamp_ns;
        }
    }
    // リングが空の場合はタイムスタンプをゼロに正規化
    if event_count == 0 {
        first_ts = 0;
        last_ts = 0;
    }
    let mut hash_data = [0u8; 8 + 8 + 8 + 8];
    hash_data[..8].copy_from_slice(&event_count.to_le_bytes());
    hash_data[8..16].copy_from_slice(&first_ts.to_le_bytes());
    hash_data[16..24].copy_from_slice(&last_ts.to_le_bytes());
    hash_data[24..32].copy_from_slice(&source_system_hash.to_le_bytes());
    TelemetryLegalAuditRecord {
        content_hash: fnv1a(&hash_data),
        event_count,
        first_timestamp_ns: first_ts,
        last_timestamp_ns: last_ts,
        source_system_hash,
    }
}

// ── Bridge 3: Telemetry → CDN (スループット測定から帯域幅レポートを生成) ──

/// CDN bandwidth report derived from semantic telemetry throughput data.
pub struct TelemetryCdnBandwidthReport {
    /// Content hash (FNV-1a of bytes_per_sec + event_rate_hz bits + node_count).
    pub content_hash: u64,
    /// Observed data throughput in bytes per second.
    pub bytes_per_sec: u64,
    /// Event processing rate in Hertz.
    pub event_rate_hz: f32,
    /// Number of CDN nodes that processed the telemetry stream.
    pub node_count: u32,
    /// Cache TTL in seconds (branchless: 60 if node_count > 0, else 0).
    pub ttl_secs: u32,
}

/// Build a CDN bandwidth report from raw throughput measurements.
///
/// `ttl_secs` is computed branchlessly: 60 when `node_count > 0`, else 0.
#[inline]
#[must_use]
pub fn telemetry_event_to_cdn_bandwidth_report(
    bytes_per_sec: u64,
    event_rate_hz: f32,
    node_count: u32,
) -> TelemetryCdnBandwidthReport {
    // node_count > 0 の場合は TTL=60、それ以外は TTL=0（ブランチレス）
    let has_nodes = (node_count > 0) as u32;
    let ttl_secs = has_nodes * 60;
    let rate_bits = event_rate_hz.to_bits();
    let mut hash_data = [0u8; 8 + 4 + 4];
    hash_data[..8].copy_from_slice(&bytes_per_sec.to_le_bytes());
    hash_data[8..12].copy_from_slice(&rate_bits.to_le_bytes());
    hash_data[12..16].copy_from_slice(&node_count.to_le_bytes());
    TelemetryCdnBandwidthReport {
        content_hash: fnv1a(&hash_data),
        bytes_per_sec,
        event_rate_hz,
        node_count,
        ttl_secs,
    }
}

// ── Bridge 4: Telemetry → Settlement (データスループット課金レコードを生成) ─

/// Data throughput settlement record for ALICE-Settlement billing.
pub struct TelemetrySettlementRecord {
    /// Content hash (FNV-1a of total_events + billable_bytes + period range).
    pub content_hash: u64,
    /// Total number of telemetry events in the billing period.
    pub total_events: u64,
    /// Billable byte volume for the billing period.
    pub billable_bytes: u64,
    /// Start of the billing period (nanoseconds).
    pub period_start_ns: u64,
    /// End of the billing period (nanoseconds).
    pub period_end_ns: u64,
}

/// Build a settlement record from semantic ring statistics and billing metadata.
#[inline]
#[must_use]
pub fn telemetry_ring_to_settlement_record(
    ring: &SemanticRing,
    billable_bytes: u64,
    period_start_ns: u64,
    period_end_ns: u64,
) -> TelemetrySettlementRecord {
    // リング内の全イベント数を集計して課金ベースとする
    let kind_counts = ring.count_by_kind();
    let total_events: u64 = kind_counts.iter().sum();
    let mut hash_data = [0u8; 8 + 8 + 8 + 8];
    hash_data[..8].copy_from_slice(&total_events.to_le_bytes());
    hash_data[8..16].copy_from_slice(&billable_bytes.to_le_bytes());
    hash_data[16..24].copy_from_slice(&period_start_ns.to_le_bytes());
    hash_data[24..32].copy_from_slice(&period_end_ns.to_le_bytes());
    TelemetrySettlementRecord {
        content_hash: fnv1a(&hash_data),
        total_events,
        billable_bytes,
        period_start_ns,
        period_end_ns,
    }
}

// ── Bridge 5: Telemetry → Container (リソース使用率レコードを生成) ────────

/// Container resource utilization record derived from semantic telemetry.
pub struct TelemetryContainerResourceRecord {
    /// Content hash (FNV-1a of cpu_usage_pct bits + memory_bytes + event_throughput + container_hash).
    pub content_hash: u64,
    /// CPU usage percentage [0.0, 100.0].
    pub cpu_usage_pct: f32,
    /// Memory consumption in bytes.
    pub memory_bytes: u64,
    /// Events processed per second by the container.
    pub event_throughput: u32,
    /// FNV-1a hash identifying the container instance.
    pub container_hash: u64,
}

/// Build a container resource record from telemetry-derived measurements.
#[inline]
#[must_use]
pub fn telemetry_event_to_container_resource_record(
    cpu_usage_pct: f32,
    memory_bytes: u64,
    event_throughput: u32,
    container_hash: u64,
) -> TelemetryContainerResourceRecord {
    // CPU・メモリ・スループットをすべてハッシュ入力に含める
    let cpu_bits = cpu_usage_pct.to_bits();
    let mut hash_data = [0u8; 4 + 8 + 4 + 8];
    hash_data[..4].copy_from_slice(&cpu_bits.to_le_bytes());
    hash_data[4..12].copy_from_slice(&memory_bytes.to_le_bytes());
    hash_data[12..16].copy_from_slice(&event_throughput.to_le_bytes());
    hash_data[16..24].copy_from_slice(&container_hash.to_le_bytes());
    TelemetryContainerResourceRecord {
        content_hash: fnv1a(&hash_data),
        cpu_usage_pct,
        memory_bytes,
        event_throughput,
        container_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_semantic_telemetry::{SemanticEvent, SemanticRing};

    /// テスト用のリングを作成するヘルパー
    fn make_ring_with_events(count: usize) -> SemanticRing {
        let mut ring = SemanticRing::new(64);
        for i in 0..count {
            ring.push(SemanticEvent {
                timestamp_ns: 1_000_000 * i as u64 + 1,
                source_id: 0xABCD,
                kind: EventKind::AnomalyDetected,
                severity: Severity::Warn,
                payload: i as u64,
                payload2: 0,
            });
        }
        ring
    }

    // ── Bridge 1: TelemetryRiskAlert ────────────────────────────────────

    #[test]
    fn test_telemetry_ring_to_risk_alert_basic() {
        // 基本フィールド検証
        let ring = make_ring_with_events(3);
        let alert = telemetry_ring_to_risk_alert(&ring, 5, 0.75, 10, 9_000_000_000);
        assert_ne!(alert.content_hash, 0);
        assert_eq!(alert.severity_level, 5);
        assert_eq!(alert.sensor_count, 10);
        assert_eq!(alert.timestamp_ns, 9_000_000_000);
        assert!((alert.anomaly_score - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_telemetry_ring_to_risk_alert_determinism() {
        // 同一入力→同一ハッシュ（決定性）
        let ring = make_ring_with_events(2);
        let a1 = telemetry_ring_to_risk_alert(&ring, 3, 0.5, 4, 1_000);
        let a2 = telemetry_ring_to_risk_alert(&ring, 3, 0.5, 4, 1_000);
        assert_eq!(a1.content_hash, a2.content_hash);
    }

    #[test]
    fn test_telemetry_ring_to_risk_alert_different_score_differs() {
        // 異常スコアが異なればハッシュも異なる
        let ring = make_ring_with_events(1);
        let a1 = telemetry_ring_to_risk_alert(&ring, 1, 0.1, 2, 100);
        let a2 = telemetry_ring_to_risk_alert(&ring, 1, 0.9, 2, 100);
        assert_ne!(a1.content_hash, a2.content_hash);
    }

    // ── Bridge 2: TelemetryLegalAuditRecord ─────────────────────────────

    #[test]
    fn test_telemetry_ring_to_legal_audit_record_basic() {
        // イベント数とタイムスタンプ範囲の検証
        let ring = make_ring_with_events(5);
        let rec = telemetry_ring_to_legal_audit_record(&ring, 0xDEAD_BEEF);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.event_count, 5);
        assert_eq!(rec.source_system_hash, 0xDEAD_BEEF);
        assert!(rec.first_timestamp_ns > 0);
        assert!(rec.last_timestamp_ns >= rec.first_timestamp_ns);
    }

    #[test]
    fn test_telemetry_ring_to_legal_audit_record_empty_ring() {
        // 空リングではイベント数ゼロ、タイムスタンプ両方ゼロ
        let ring = SemanticRing::new(16);
        let rec = telemetry_ring_to_legal_audit_record(&ring, 0x1234);
        assert_eq!(rec.event_count, 0);
        assert_eq!(rec.first_timestamp_ns, 0);
        assert_eq!(rec.last_timestamp_ns, 0);
    }

    #[test]
    fn test_telemetry_ring_to_legal_audit_record_determinism() {
        // 決定性テスト
        let ring = make_ring_with_events(3);
        let r1 = telemetry_ring_to_legal_audit_record(&ring, 0xCAFE);
        let r2 = telemetry_ring_to_legal_audit_record(&ring, 0xCAFE);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 3: TelemetryCdnBandwidthReport ───────────────────────────

    #[test]
    fn test_telemetry_event_to_cdn_bandwidth_report_basic() {
        // 基本フィールドとTTLの検証
        let rep = telemetry_event_to_cdn_bandwidth_report(1_000_000, 500.0, 4);
        assert_ne!(rep.content_hash, 0);
        assert_eq!(rep.bytes_per_sec, 1_000_000);
        assert_eq!(rep.node_count, 4);
        assert_eq!(rep.ttl_secs, 60); // node_count > 0 → TTL=60
    }

    #[test]
    fn test_telemetry_event_to_cdn_bandwidth_report_zero_nodes_ttl() {
        // node_count=0 のとき TTL はゼロ（ブランチレス検証）
        let rep = telemetry_event_to_cdn_bandwidth_report(500_000, 100.0, 0);
        assert_eq!(rep.ttl_secs, 0);
    }

    #[test]
    fn test_telemetry_event_to_cdn_bandwidth_report_determinism() {
        // 決定性テスト
        let r1 = telemetry_event_to_cdn_bandwidth_report(2_000_000, 1000.0, 8);
        let r2 = telemetry_event_to_cdn_bandwidth_report(2_000_000, 1000.0, 8);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_telemetry_event_to_cdn_bandwidth_report_different_rate_differs() {
        // イベントレートが異なればハッシュも異なる
        let r1 = telemetry_event_to_cdn_bandwidth_report(1_000, 100.0, 2);
        let r2 = telemetry_event_to_cdn_bandwidth_report(1_000, 200.0, 2);
        assert_ne!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 4: TelemetrySettlementRecord ─────────────────────────────

    #[test]
    fn test_telemetry_ring_to_settlement_record_basic() {
        // イベント合計・課金バイト・期間フィールドの検証
        let ring = make_ring_with_events(4);
        let rec =
            telemetry_ring_to_settlement_record(&ring, 8_000_000, 1_000_000_000, 2_000_000_000);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.total_events, 4);
        assert_eq!(rec.billable_bytes, 8_000_000);
        assert_eq!(rec.period_start_ns, 1_000_000_000);
        assert_eq!(rec.period_end_ns, 2_000_000_000);
    }

    #[test]
    fn test_telemetry_ring_to_settlement_record_determinism() {
        // 決定性テスト
        let ring = make_ring_with_events(2);
        let r1 = telemetry_ring_to_settlement_record(&ring, 100, 0, 1_000);
        let r2 = telemetry_ring_to_settlement_record(&ring, 100, 0, 1_000);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    // ── Bridge 5: TelemetryContainerResourceRecord ───────────────────────

    #[test]
    fn test_telemetry_event_to_container_resource_record_basic() {
        // 全フィールドの基本検証
        let rec = telemetry_event_to_container_resource_record(45.5, 512_000_000, 1_200, 0xC0FFEE);
        assert_ne!(rec.content_hash, 0);
        assert!((rec.cpu_usage_pct - 45.5).abs() < f32::EPSILON);
        assert_eq!(rec.memory_bytes, 512_000_000);
        assert_eq!(rec.event_throughput, 1_200);
        assert_eq!(rec.container_hash, 0xC0FFEE);
    }

    #[test]
    fn test_telemetry_event_to_container_resource_record_determinism() {
        // 決定性テスト
        let r1 = telemetry_event_to_container_resource_record(10.0, 256_000, 400, 0xABCD);
        let r2 = telemetry_event_to_container_resource_record(10.0, 256_000, 400, 0xABCD);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_telemetry_event_to_container_resource_record_different_cpu_differs() {
        // CPU使用率が異なればハッシュも異なる
        let r1 = telemetry_event_to_container_resource_record(10.0, 128_000, 200, 0x1234);
        let r2 = telemetry_event_to_container_resource_record(90.0, 128_000, 200, 0x1234);
        assert_ne!(r1.content_hash, r2.content_hash);
    }
}
