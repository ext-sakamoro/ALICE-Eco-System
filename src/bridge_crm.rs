//! CRM bridges — ALICE-CRM ↔ DB, Cache, Analytics, Notify, API
//!
//! 5 bridges connecting customer relationship management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: CRM → DB (contact persistence) ─────────────────────────────

/// Contact record for ALICE-DB persistence.
pub struct CrmDbRecord {
    /// Content hash over the contact fields.
    pub content_hash: u64,
    /// Total number of contacts.
    pub contact_count: u64,
    /// Total number of deals.
    pub deal_count: u64,
    /// Total pipeline value in the account's base currency (smallest unit, e.g. cents).
    pub pipeline_value_cents: u64,
    /// Lead score (0–100).
    pub lead_score: u8,
    /// Last activity timestamp (Unix seconds).
    pub last_activity_ts: u64,
    /// Account tier: 0 = free, 1 = starter, 2 = growth, 3 = enterprise.
    pub account_tier: u8,
}

/// Serialize contact data for ALICE-DB persistence.
#[inline]
#[must_use]
pub fn crm_to_db_record(
    contact_count: u64,
    deal_count: u64,
    pipeline_value_cents: u64,
    lead_score: u8,
    last_activity_ts: u64,
    account_tier: u8,
) -> CrmDbRecord {
    let mut key = [0u8; 34];
    key[0..8].copy_from_slice(&contact_count.to_le_bytes());
    key[8..16].copy_from_slice(&deal_count.to_le_bytes());
    key[16..24].copy_from_slice(&pipeline_value_cents.to_le_bytes());
    key[24..32].copy_from_slice(&last_activity_ts.to_le_bytes());
    key[32] = lead_score;
    key[33] = account_tier;
    CrmDbRecord {
        content_hash: fnv1a(&key),
        contact_count,
        deal_count,
        pipeline_value_cents,
        lead_score,
        last_activity_ts,
        account_tier,
    }
}

// ── Bridge 2: CRM → Cache (pipeline cache) ────────────────────────────────

/// Pipeline summary cache entry for ALICE-Cache.
pub struct CrmCacheEntry {
    /// Content hash for cache key derivation.
    pub content_hash: u64,
    /// Number of open deals.
    pub deal_count: u64,
    /// Total pipeline value in cents.
    pub pipeline_value_cents: u64,
    /// Deal conversion rate (0.0–1.0).
    pub conversion_rate: f32,
    /// Average deal value in cents.
    pub avg_deal_value_cents: u64,
    /// TTL in seconds (branchless: shorter for high-velocity pipelines).
    pub ttl_secs: u32,
}

/// Cache pipeline summary for ALICE-Cache.
#[inline]
#[must_use]
pub fn crm_to_cache_entry(
    deal_count: u64,
    pipeline_value_cents: u64,
    closed_deal_count: u64,
    total_deal_count: u64,
) -> CrmCacheEntry {
    let rcp_deals = 1.0 / deal_count.max(1) as f64;
    let avg_deal_value_cents = (pipeline_value_cents as f64 * rcp_deals) as u64;

    let rcp_total = 1.0 / total_deal_count.max(1) as f32;
    let conversion_rate = closed_deal_count as f32 * rcp_total;

    // Branchless TTL: 300 s normally, 60 s for high-velocity (>1000 deals).
    let high_velocity = (deal_count > 1_000) as u32;
    let ttl_secs = 300_u32 - high_velocity * 240;

    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&deal_count.to_le_bytes());
    key[8..16].copy_from_slice(&pipeline_value_cents.to_le_bytes());
    CrmCacheEntry {
        content_hash: fnv1a(&key),
        deal_count,
        pipeline_value_cents,
        conversion_rate,
        avg_deal_value_cents,
        ttl_secs,
    }
}

// ── Bridge 3: CRM → Analytics (sales metrics) ─────────────────────────────

/// Sales metrics for ALICE-Analytics ingestion.
pub struct CrmAnalyticsMetrics {
    /// Content hash over the metric values.
    pub content_hash: u64,
    /// Total contacts in the system.
    pub contact_count: u64,
    /// Total deals in the system.
    pub deal_count: u64,
    /// Total pipeline value in cents.
    pub pipeline_value_cents: u64,
    /// Deal conversion rate (0.0–1.0).
    pub conversion_rate: f32,
    /// Average lead score (0.0–100.0).
    pub avg_lead_score: f32,
    /// Average deal cycle length in days.
    pub avg_cycle_days: f32,
    /// Number of deals closed in the reporting period.
    pub closed_this_period: u64,
}

/// Build sales metrics for ALICE-Analytics ingestion.
#[inline]
#[must_use]
pub fn crm_to_analytics_metrics(
    contact_count: u64,
    deal_count: u64,
    pipeline_value_cents: u64,
    closed_deal_count: u64,
    total_lead_score: u64,
    total_cycle_days: u64,
    closed_this_period: u64,
) -> CrmAnalyticsMetrics {
    let rcp_deals = 1.0 / deal_count.max(1) as f32;
    let conversion_rate = closed_deal_count as f32 * rcp_deals;
    let avg_lead_score = total_lead_score as f32 * rcp_deals;
    let avg_cycle_days = total_cycle_days as f32 * rcp_deals;

    let mut key = [0u8; 24];
    key[0..8].copy_from_slice(&contact_count.to_le_bytes());
    key[8..16].copy_from_slice(&deal_count.to_le_bytes());
    key[16..24].copy_from_slice(&pipeline_value_cents.to_le_bytes());
    CrmAnalyticsMetrics {
        content_hash: fnv1a(&key),
        contact_count,
        deal_count,
        pipeline_value_cents,
        conversion_rate,
        avg_lead_score,
        avg_cycle_days,
        closed_this_period,
    }
}

// ── Bridge 4: CRM → Notify (follow-up) ───────────────────────────────────

/// Follow-up notification payload for ALICE-Notify dispatch.
pub struct CrmNotifyFollowUp {
    /// Content hash for deduplication.
    pub content_hash: u64,
    /// Internal contact identifier.
    pub contact_id: u64,
    /// Lead score (0–100) at time of notification.
    pub lead_score: u8,
    /// Days since last activity.
    pub days_since_activity: u32,
    /// Priority: 0 = low, 1 = medium, 2 = high, 3 = urgent.
    pub priority: u8,
    /// Deal count associated with this contact.
    pub deal_count: u32,
    /// Scheduled follow-up timestamp (Unix seconds).
    pub follow_up_ts: u64,
}

/// Build follow-up notification for ALICE-Notify.
#[inline]
#[must_use]
pub fn crm_to_notify_follow_up(
    contact_id: u64,
    lead_score: u8,
    days_since_activity: u32,
    deal_count: u32,
    follow_up_ts: u64,
) -> CrmNotifyFollowUp {
    // Priority escalation based on days since last activity.
    let priority = match days_since_activity {
        0..=6 => 0u8,
        7..=13 => 1u8,
        14..=29 => 2u8,
        _ => 3u8,
    };

    let mut key = [0u8; 20];
    key[0..8].copy_from_slice(&contact_id.to_le_bytes());
    key[8..12].copy_from_slice(&days_since_activity.to_le_bytes());
    key[12..20].copy_from_slice(&follow_up_ts.to_le_bytes());
    CrmNotifyFollowUp {
        content_hash: fnv1a(&key),
        contact_id,
        lead_score,
        days_since_activity,
        priority,
        deal_count,
        follow_up_ts,
    }
}

// ── Bridge 5: CRM → API (integration) ─────────────────────────────────────

/// Integration payload for ALICE-API exposure.
pub struct CrmApiPayload {
    /// Content hash for ETag generation.
    pub content_hash: u64,
    /// Total contact count.
    pub contact_count: u64,
    /// Total deal count.
    pub deal_count: u64,
    /// Total pipeline value in cents.
    pub pipeline_value_cents: u64,
    /// Conversion rate (0.0–1.0).
    pub conversion_rate: f32,
    /// Average lead score (0.0–100.0).
    pub avg_lead_score: f32,
    /// API schema version (for versioned endpoints).
    pub schema_version: u16,
}

/// Build integration payload for ALICE-API.
#[inline]
#[must_use]
pub fn crm_to_api_payload(
    contact_count: u64,
    deal_count: u64,
    pipeline_value_cents: u64,
    closed_deal_count: u64,
    total_lead_score: u64,
    schema_version: u16,
) -> CrmApiPayload {
    let rcp_deals = 1.0 / deal_count.max(1) as f32;
    let conversion_rate = closed_deal_count as f32 * rcp_deals;
    let avg_lead_score = total_lead_score as f32 * rcp_deals;

    let mut key = [0u8; 26];
    key[0..8].copy_from_slice(&contact_count.to_le_bytes());
    key[8..16].copy_from_slice(&deal_count.to_le_bytes());
    key[16..24].copy_from_slice(&pipeline_value_cents.to_le_bytes());
    key[24..26].copy_from_slice(&schema_version.to_le_bytes());
    CrmApiPayload {
        content_hash: fnv1a(&key),
        contact_count,
        deal_count,
        pipeline_value_cents,
        conversion_rate,
        avg_lead_score,
        schema_version,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crm_to_db_record_hash_nonzero() {
        let rec = crm_to_db_record(500, 120, 5_000_000, 75, 1_700_000_000, 2);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.contact_count, 500);
        assert_eq!(rec.account_tier, 2);
    }

    #[test]
    fn test_crm_to_db_record_deterministic() {
        let a = crm_to_db_record(100, 30, 1_000_000, 60, 0, 1);
        let b = crm_to_db_record(100, 30, 1_000_000, 60, 0, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_crm_to_cache_entry_avg_deal_value() {
        let entry = crm_to_cache_entry(10, 1_000_000, 4, 20);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.avg_deal_value_cents, 100_000);
        assert!((entry.conversion_rate - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_crm_to_cache_entry_ttl_normal_vs_high_velocity() {
        let normal = crm_to_cache_entry(500, 50_000_000, 100, 200);
        assert_eq!(normal.ttl_secs, 300); // 500 deals <= 1000 threshold

        let fast = crm_to_cache_entry(2_000, 200_000_000, 800, 3_000);
        assert_eq!(fast.ttl_secs, 60); // 2000 deals > 1000 threshold
    }

    #[test]
    fn test_crm_to_analytics_metrics_conversion() {
        let m = crm_to_analytics_metrics(1_000, 200, 10_000_000, 50, 12_000, 6_000, 10);
        assert_ne!(m.content_hash, 0);
        assert!((m.conversion_rate - 0.25).abs() < 0.001);
        assert!((m.avg_lead_score - 60.0).abs() < 0.001);
        assert!((m.avg_cycle_days - 30.0).abs() < 0.001);
        assert_eq!(m.closed_this_period, 10);
    }

    #[test]
    fn test_crm_to_notify_follow_up_priority() {
        let low = crm_to_notify_follow_up(1, 50, 3, 2, 1_700_000_000);
        assert_eq!(low.priority, 0);

        let med = crm_to_notify_follow_up(2, 60, 10, 3, 1_700_000_001);
        assert_eq!(med.priority, 1);

        let high = crm_to_notify_follow_up(3, 70, 20, 5, 1_700_000_002);
        assert_eq!(high.priority, 2);

        let urgent = crm_to_notify_follow_up(4, 80, 45, 7, 1_700_000_003);
        assert_eq!(urgent.priority, 3);
        assert_ne!(urgent.content_hash, 0);
    }

    #[test]
    fn test_crm_to_api_payload_fields() {
        let p = crm_to_api_payload(800, 150, 7_500_000, 60, 9_000, 2);
        assert_ne!(p.content_hash, 0);
        assert_eq!(p.schema_version, 2);
        assert!((p.conversion_rate - 0.4).abs() < 0.001);
        assert!((p.avg_lead_score - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_crm_to_api_payload_zero_deals() {
        // Zero-deal case must not panic (rcp_deals uses max(1)).
        let p = crm_to_api_payload(0, 0, 0, 0, 0, 1);
        assert_eq!(p.deal_count, 0);
        assert_eq!(p.conversion_rate, 0.0);
        assert_ne!(p.content_hash, 0);
    }
}
