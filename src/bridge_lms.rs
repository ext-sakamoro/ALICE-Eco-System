//! LMS bridges — ALICE-LMS ↔ DB, Cache, Analytics, Notify, API
//!
//! 5 bridges connecting learning management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: LMS → DB (course storage) ─────────────────────────────────

/// Course catalog record for ALICE-DB persistence.
pub struct LmsDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Number of courses in the catalog.
    pub course_count: u32,
    /// Total enrollment count across all courses.
    pub enrollment_count: u32,
    /// Total number of issued certificates.
    pub certificate_count: u32,
    /// Total content duration in seconds across all courses.
    pub total_duration_secs: u64,
    /// Catalogue version identifier.
    pub catalog_version: u32,
}

/// Convert LMS catalog state into an ALICE-DB record.
#[inline]
#[must_use]
pub fn lms_to_db_record(
    course_count: u32,
    enrollment_count: u32,
    certificate_count: u32,
    total_duration_secs: u64,
    catalog_version: u32,
) -> LmsDbRecord {
    let mut data = [0u8; 20];
    data[0..4].copy_from_slice(&course_count.to_le_bytes());
    data[4..8].copy_from_slice(&enrollment_count.to_le_bytes());
    data[8..12].copy_from_slice(&certificate_count.to_le_bytes());
    data[12..20].copy_from_slice(&total_duration_secs.to_le_bytes());
    LmsDbRecord {
        content_hash: fnv1a(&data),
        course_count,
        enrollment_count,
        certificate_count,
        total_duration_secs,
        catalog_version,
    }
}

// ── Bridge 2: LMS → Cache (progress cache) ──────────────────────────────

/// Learner progress cache entry for ALICE-Cache.
pub struct LmsCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Learner identifier.
    pub learner_id: u64,
    /// Course identifier.
    pub course_id: u64,
    /// Progress in basis points (0–10000 = 0%–100%).
    pub progress_bps: u16,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an ALICE-Cache progress entry from LMS learner state.
#[inline]
#[must_use]
pub fn lms_to_cache_entry(
    learner_id: u64,
    course_id: u64,
    progress_bps: u16,
    ttl_secs: u32,
) -> LmsCacheEntry {
    let mut data = [0u8; 18];
    data[0..8].copy_from_slice(&learner_id.to_le_bytes());
    data[8..16].copy_from_slice(&course_id.to_le_bytes());
    data[16..18].copy_from_slice(&progress_bps.to_le_bytes());
    LmsCacheEntry {
        content_hash: fnv1a(&data),
        learner_id,
        course_id,
        progress_bps,
        ttl_secs,
    }
}

// ── Bridge 3: LMS → Analytics (learning metrics) ────────────────────────

/// Learning metrics event for ALICE-Analytics ingestion.
pub struct LmsAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Course completion rate in basis points (0–10000).
    pub completion_rate_bps: u16,
    /// Average assessment score in basis points (0–10000).
    pub avg_score_bps: u16,
    /// Number of active learners in the observation window.
    pub active_learner_count: u32,
    /// Total time spent learning in the window, in seconds.
    pub total_learning_secs: u64,
    /// Observation window duration in days.
    pub window_days: u16,
}

/// Convert LMS learning KPIs into an ALICE-Analytics event.
#[inline]
#[must_use]
pub fn lms_to_analytics_event(
    completion_rate_bps: u16,
    avg_score_bps: u16,
    active_learner_count: u32,
    total_learning_secs: u64,
    window_days: u16,
) -> LmsAnalyticsEvent {
    let mut data = [0u8; 16];
    data[0..2].copy_from_slice(&completion_rate_bps.to_le_bytes());
    data[2..4].copy_from_slice(&avg_score_bps.to_le_bytes());
    data[4..8].copy_from_slice(&active_learner_count.to_le_bytes());
    data[8..16].copy_from_slice(&total_learning_secs.to_le_bytes());
    LmsAnalyticsEvent {
        content_hash: fnv1a(&data),
        completion_rate_bps,
        avg_score_bps,
        active_learner_count,
        total_learning_secs,
        window_days,
    }
}

// ── Bridge 4: LMS → Notify (enrollment alerts) ──────────────────────────

/// Enrollment alert payload for ALICE-Notify delivery.
pub struct LmsNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Course identifier that triggered the alert.
    pub course_id: u64,
    /// Current enrollment count for the course.
    pub enrollment_count: u32,
    /// Enrollment capacity limit (0 = unlimited).
    pub capacity_limit: u32,
    /// Alert severity level (0 = info, 1 = warning, 2 = critical).
    pub severity: u8,
    /// Deadline as Unix timestamp (seconds); 0 = no deadline.
    pub deadline_ts: u64,
}

/// Build an ALICE-Notify enrollment alert from LMS course state.
#[inline]
#[must_use]
pub fn lms_to_notify_alert(
    course_id: u64,
    enrollment_count: u32,
    capacity_limit: u32,
    severity: u8,
    deadline_ts: u64,
) -> LmsNotifyAlert {
    let mut data = [0u8; 17];
    data[0..8].copy_from_slice(&course_id.to_le_bytes());
    data[8..12].copy_from_slice(&enrollment_count.to_le_bytes());
    data[12..16].copy_from_slice(&capacity_limit.to_le_bytes());
    data[16] = severity;
    LmsNotifyAlert {
        content_hash: fnv1a(&data),
        course_id,
        enrollment_count,
        capacity_limit,
        severity,
        deadline_ts,
    }
}

// ── Bridge 5: LMS → API (integration) ───────────────────────────────────

/// API integration payload exposing LMS state to external consumers.
pub struct LmsApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Tenant / organisation identifier.
    pub tenant_id: u32,
    /// Number of published courses.
    pub published_course_count: u32,
    /// Total active enrollments.
    pub active_enrollment_count: u32,
    /// Number of certificates issued in the current period.
    pub certificates_issued: u32,
    /// Schema version of this payload.
    pub schema_version: u16,
}

/// Compose an ALICE-API payload from LMS catalog and enrollment state.
#[inline]
#[must_use]
pub fn lms_to_api_payload(
    tenant_id: u32,
    published_course_count: u32,
    active_enrollment_count: u32,
    certificates_issued: u32,
    schema_version: u16,
) -> LmsApiPayload {
    let mut data = [0u8; 18];
    data[0..4].copy_from_slice(&tenant_id.to_le_bytes());
    data[4..8].copy_from_slice(&published_course_count.to_le_bytes());
    data[8..12].copy_from_slice(&active_enrollment_count.to_le_bytes());
    data[12..16].copy_from_slice(&certificates_issued.to_le_bytes());
    data[16..18].copy_from_slice(&schema_version.to_le_bytes());
    LmsApiPayload {
        content_hash: fnv1a(&data),
        tenant_id,
        published_course_count,
        active_enrollment_count,
        certificates_issued,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = lms_to_db_record(80, 2000, 500, 360_000, 3);
        let b = lms_to_db_record(80, 2000, 500, 360_000, 3);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_hash_changes_on_course_count() {
        let a = lms_to_db_record(80, 2000, 500, 360_000, 3);
        let b = lms_to_db_record(81, 2000, 500, 360_000, 3);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_fields_preserved() {
        let r = lms_to_db_record(50, 1500, 300, 180_000, 2);
        assert_eq!(r.course_count, 50);
        assert_eq!(r.enrollment_count, 1500);
        assert_eq!(r.certificate_count, 300);
        assert_eq!(r.catalog_version, 2);
    }

    #[test]
    fn cache_entry_progress_clamped_representation() {
        let e = lms_to_cache_entry(111, 222, 7500, 1800);
        assert_eq!(e.progress_bps, 7500);
        assert!(e.progress_bps <= 10_000);
    }

    #[test]
    fn analytics_event_hash_is_deterministic() {
        let a = lms_to_analytics_event(6500, 7200, 400, 1_440_000, 30);
        let b = lms_to_analytics_event(6500, 7200, 400, 1_440_000, 30);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn analytics_event_score_range() {
        let ev = lms_to_analytics_event(8000, 8500, 200, 720_000, 7);
        assert!(ev.avg_score_bps <= 10_000);
        assert!(ev.completion_rate_bps <= 10_000);
    }

    #[test]
    fn notify_alert_severity_and_capacity() {
        let alert = lms_to_notify_alert(55, 98, 100, 1, 1_700_000_000);
        assert_eq!(alert.severity, 1);
        assert!(alert.enrollment_count <= alert.capacity_limit);
    }

    #[test]
    fn api_payload_hash_changes_on_certificates() {
        let a = lms_to_api_payload(1, 80, 2000, 500, 1);
        let b = lms_to_api_payload(1, 80, 2000, 501, 1);
        assert_ne!(a.content_hash, b.content_hash);
        assert_eq!(a.tenant_id, b.tenant_id);
    }
}
