//! HRM bridges — ALICE-HRM ↔ DB, Cache, Analytics, Notify, API
//!
//! 5 bridges connecting human resource management to the ALICE ecosystem.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: HRM → DB (employee records) ───────────────────────────────

/// Employee record for ALICE-DB persistence.
pub struct HrmDbRecord {
    /// Content hash.
    pub content_hash: u64,
    /// Total number of employees.
    pub employee_count: u32,
    /// Number of departments.
    pub department_count: u16,
    /// Total monthly payroll in minor currency units (e.g. cents).
    pub payroll_total: u64,
    /// Average tenure in whole months.
    pub avg_tenure_months: u16,
    /// Organisational unit identifier this record covers.
    pub org_unit_id: u32,
}

/// Convert HRM workforce data into an ALICE-DB employee record.
#[inline]
#[must_use]
pub fn hrm_to_db_record(
    employee_count: u32,
    department_count: u16,
    payroll_total: u64,
    avg_tenure_months: u16,
    org_unit_id: u32,
) -> HrmDbRecord {
    let mut data = [0u8; 18];
    data[0..4].copy_from_slice(&employee_count.to_le_bytes());
    data[4..6].copy_from_slice(&department_count.to_le_bytes());
    data[6..14].copy_from_slice(&payroll_total.to_le_bytes());
    data[14..16].copy_from_slice(&avg_tenure_months.to_le_bytes());
    data[16..18].copy_from_slice(&org_unit_id.to_le_bytes()[0..2]);
    HrmDbRecord {
        content_hash: fnv1a(&data),
        employee_count,
        department_count,
        payroll_total,
        avg_tenure_months,
        org_unit_id,
    }
}

// ── Bridge 2: HRM → Cache (schedule cache) ──────────────────────────────

/// Schedule cache entry for ALICE-Cache.
pub struct HrmCacheEntry {
    /// Content hash used as cache key.
    pub content_hash: u64,
    /// Schedule period start as Unix timestamp (seconds).
    pub period_start_ts: u64,
    /// Schedule period end as Unix timestamp (seconds).
    pub period_end_ts: u64,
    /// Number of shifts covered in this schedule block.
    pub shift_count: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
}

/// Build an ALICE-Cache schedule entry from HRM schedule data.
#[inline]
#[must_use]
pub fn hrm_to_cache_entry(
    period_start_ts: u64,
    period_end_ts: u64,
    shift_count: u32,
    ttl_secs: u32,
) -> HrmCacheEntry {
    let mut data = [0u8; 20];
    data[0..8].copy_from_slice(&period_start_ts.to_le_bytes());
    data[8..16].copy_from_slice(&period_end_ts.to_le_bytes());
    data[16..20].copy_from_slice(&shift_count.to_le_bytes());
    HrmCacheEntry {
        content_hash: fnv1a(&data),
        period_start_ts,
        period_end_ts,
        shift_count,
        ttl_secs,
    }
}

// ── Bridge 3: HRM → Analytics (workforce metrics) ───────────────────────

/// Workforce metrics event for ALICE-Analytics ingestion.
pub struct HrmAnalyticsEvent {
    /// Content hash.
    pub content_hash: u64,
    /// Attendance rate in basis points (0–10000).
    pub attendance_rate_bps: u16,
    /// Turnover rate in basis points over the observation window.
    pub turnover_rate_bps: u16,
    /// Total accumulated leave balance in hours across all employees.
    pub leave_balance_hours: u32,
    /// Number of open positions (headcount gap).
    pub open_positions: u16,
    /// Observation window duration in days.
    pub window_days: u16,
}

/// Convert HRM workforce KPIs into an ALICE-Analytics event.
#[inline]
#[must_use]
pub fn hrm_to_analytics_event(
    attendance_rate_bps: u16,
    turnover_rate_bps: u16,
    leave_balance_hours: u32,
    open_positions: u16,
    window_days: u16,
) -> HrmAnalyticsEvent {
    let mut data = [0u8; 12];
    data[0..2].copy_from_slice(&attendance_rate_bps.to_le_bytes());
    data[2..4].copy_from_slice(&turnover_rate_bps.to_le_bytes());
    data[4..8].copy_from_slice(&leave_balance_hours.to_le_bytes());
    data[8..10].copy_from_slice(&open_positions.to_le_bytes());
    data[10..12].copy_from_slice(&window_days.to_le_bytes());
    HrmAnalyticsEvent {
        content_hash: fnv1a(&data),
        attendance_rate_bps,
        turnover_rate_bps,
        leave_balance_hours,
        open_positions,
        window_days,
    }
}

// ── Bridge 4: HRM → Notify (payroll alerts) ─────────────────────────────

/// Payroll alert payload for ALICE-Notify delivery.
pub struct HrmNotifyAlert {
    /// Content hash.
    pub content_hash: u64,
    /// Payroll run identifier.
    pub payroll_run_id: u64,
    /// Number of employees affected by the alert condition.
    pub affected_employee_count: u32,
    /// Discrepancy amount in minor currency units (absolute value).
    pub discrepancy_amount: u64,
    /// Alert severity level (0 = info, 1 = warning, 2 = critical).
    pub severity: u8,
    /// Payroll period end as Unix timestamp (seconds).
    pub period_end_ts: u64,
}

/// Build an ALICE-Notify payroll alert from HRM payroll data.
#[inline]
#[must_use]
pub fn hrm_to_notify_alert(
    payroll_run_id: u64,
    affected_employee_count: u32,
    discrepancy_amount: u64,
    severity: u8,
    period_end_ts: u64,
) -> HrmNotifyAlert {
    let mut data = [0u8; 21];
    data[0..8].copy_from_slice(&payroll_run_id.to_le_bytes());
    data[8..12].copy_from_slice(&affected_employee_count.to_le_bytes());
    data[12..20].copy_from_slice(&discrepancy_amount.to_le_bytes());
    data[20] = severity;
    HrmNotifyAlert {
        content_hash: fnv1a(&data),
        payroll_run_id,
        affected_employee_count,
        discrepancy_amount,
        severity,
        period_end_ts,
    }
}

// ── Bridge 5: HRM → API (integration) ───────────────────────────────────

/// API integration payload exposing HRM state to external consumers.
pub struct HrmApiPayload {
    /// Content hash.
    pub content_hash: u64,
    /// Tenant / organisation identifier.
    pub tenant_id: u32,
    /// Total headcount at time of export.
    pub headcount: u32,
    /// Number of active contractors.
    pub contractor_count: u32,
    /// Monthly payroll total in minor currency units.
    pub payroll_total: u64,
    /// Schema version of this payload.
    pub schema_version: u16,
}

/// Compose an ALICE-API payload from HRM headcount and payroll state.
#[inline]
#[must_use]
pub fn hrm_to_api_payload(
    tenant_id: u32,
    headcount: u32,
    contractor_count: u32,
    payroll_total: u64,
    schema_version: u16,
) -> HrmApiPayload {
    let mut data = [0u8; 20];
    data[0..4].copy_from_slice(&tenant_id.to_le_bytes());
    data[4..8].copy_from_slice(&headcount.to_le_bytes());
    data[8..12].copy_from_slice(&contractor_count.to_le_bytes());
    data[12..20].copy_from_slice(&payroll_total.to_le_bytes());
    data[18..20].copy_from_slice(&schema_version.to_le_bytes());
    HrmApiPayload {
        content_hash: fnv1a(&data),
        tenant_id,
        headcount,
        contractor_count,
        payroll_total,
        schema_version,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_record_hash_is_deterministic() {
        let a = hrm_to_db_record(500, 12, 25_000_000, 36, 7);
        let b = hrm_to_db_record(500, 12, 25_000_000, 36, 7);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_hash_changes_on_employee_count() {
        let a = hrm_to_db_record(500, 12, 25_000_000, 36, 7);
        let b = hrm_to_db_record(501, 12, 25_000_000, 36, 7);
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn db_record_fields_preserved() {
        let r = hrm_to_db_record(200, 8, 10_000_000, 24, 3);
        assert_eq!(r.employee_count, 200);
        assert_eq!(r.department_count, 8);
        assert_eq!(r.avg_tenure_months, 24);
        assert_eq!(r.org_unit_id, 3);
    }

    #[test]
    fn cache_entry_period_ordering() {
        let e = hrm_to_cache_entry(1_000_000, 1_604_800, 21, 86400);
        assert!(e.period_end_ts > e.period_start_ts);
        assert_eq!(e.shift_count, 21);
    }

    #[test]
    fn analytics_event_hash_is_deterministic() {
        let a = hrm_to_analytics_event(9500, 300, 8000, 5, 30);
        let b = hrm_to_analytics_event(9500, 300, 8000, 5, 30);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn analytics_event_attendance_rate_range() {
        let ev = hrm_to_analytics_event(9800, 150, 12000, 3, 30);
        assert!(ev.attendance_rate_bps <= 10_000);
    }

    #[test]
    fn notify_alert_severity_preserved() {
        let alert = hrm_to_notify_alert(42, 10, 50_000, 2, 1_700_000_000);
        assert_eq!(alert.severity, 2);
        assert_eq!(alert.payroll_run_id, 42);
    }

    #[test]
    fn api_payload_hash_changes_on_headcount() {
        let a = hrm_to_api_payload(1, 300, 20, 90_000_000, 1);
        let b = hrm_to_api_payload(1, 301, 20, 90_000_000, 1);
        assert_ne!(a.content_hash, b.content_hash);
        assert_eq!(a.tenant_id, b.tenant_id);
    }
}
