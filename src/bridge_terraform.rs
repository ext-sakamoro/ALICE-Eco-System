//! Terraform bridges — Terraform ↔ DB, Cache, Analytics, Monitor, Notify
//!
//! 5 bridges connecting infrastructure-as-code state management to the ALICE ecosystem.
//! Covers state persistence, plan caching, infrastructure metrics,
//! drift detection, and change-alert notifications.

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Terraform → DB (state storage) ─────────────────────────────

/// Terraform state storage record for ALICE-DB.
///
/// One record per `terraform apply` outcome. `content_hash` covers
/// `workspace_hash + run_id` so each apply is individually addressable.
pub struct TerraformDbStateRecord {
    /// FNV-1a hash of workspace_hash + run_id — deduplication key.
    pub content_hash: u64,
    /// FNV-1a hash of the workspace name string.
    pub workspace_hash: u64,
    /// Monotonically increasing run identifier.
    pub run_id: u64,
    /// Total number of managed resources after this apply.
    pub resource_count: u32,
    /// Number of resources changed (created + updated + destroyed).
    pub change_count: u32,
    /// Number of resources created in this apply.
    pub create_count: u32,
    /// Number of resources destroyed in this apply.
    pub destroy_count: u32,
    /// FNV-1a hash of the serialised state file for integrity checks.
    pub state_hash: u64,
    /// Apply duration in milliseconds.
    pub apply_duration_ms: u64,
    /// Whether the apply completed without errors.
    pub success: bool,
}

/// Build a `TerraformDbStateRecord` for a completed apply run.
#[inline]
#[must_use]
pub fn terraform_to_db_state_record(
    workspace: &str,
    run_id: u64,
    resource_count: u32,
    change_count: u32,
    create_count: u32,
    destroy_count: u32,
    state_hash: u64,
    apply_duration_ms: u64,
    success: bool,
) -> TerraformDbStateRecord {
    let workspace_hash = fnv1a(workspace.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&workspace_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&run_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    TerraformDbStateRecord {
        content_hash,
        workspace_hash,
        run_id,
        resource_count,
        change_count,
        create_count,
        destroy_count,
        state_hash,
        apply_duration_ms,
        success,
    }
}

// ── Bridge 2: Terraform → Cache (plan cache) ─────────────────────────────

/// Terraform plan cache entry for ALICE-Cache.
///
/// Caches the output of `terraform plan` keyed on workspace + config hash,
/// so CI pipelines can skip re-planning when no config has changed.
/// TTL is very short — plans expire quickly as provider APIs evolve.
pub struct TerraformPlanCacheEntry {
    /// FNV-1a hash of workspace_hash + config_hash — cache lookup key.
    pub content_hash: u64,
    /// FNV-1a hash of the workspace name.
    pub workspace_hash: u64,
    /// FNV-1a hash of the configuration files.
    pub config_hash: u64,
    /// Number of resources that would change.
    pub planned_change_count: u32,
    /// Number of resources that would be created.
    pub planned_create_count: u32,
    /// Number of resources that would be destroyed.
    pub planned_destroy_count: u32,
    /// Plan computation time in milliseconds.
    pub plan_time_ms: u32,
    /// Cache TTL in seconds.
    pub ttl_secs: u32,
    /// Whether the plan contains any destructive actions.
    pub has_destroys: bool,
}

/// Build a `TerraformPlanCacheEntry` for a computed plan.
///
/// TTL is 60 s when `has_destroys` (conservative — re-validate soon)
/// and 300 s otherwise (branchless).
#[inline]
#[must_use]
pub fn terraform_to_plan_cache_entry(
    workspace: &str,
    config_hash: u64,
    planned_change_count: u32,
    planned_create_count: u32,
    planned_destroy_count: u32,
    plan_time_ms: u32,
) -> TerraformPlanCacheEntry {
    let workspace_hash = fnv1a(workspace.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&workspace_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&config_hash.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let has_destroys = planned_destroy_count > 0;
    // Branchless TTL: has destroys → 60s, no destroys → 300s.
    let has_dest_u32 = has_destroys as u32;
    let ttl_secs = 300 - has_dest_u32 * 240;
    TerraformPlanCacheEntry {
        content_hash,
        workspace_hash,
        config_hash,
        planned_change_count,
        planned_create_count,
        planned_destroy_count,
        plan_time_ms,
        ttl_secs,
        has_destroys,
    }
}

// ── Bridge 3: Terraform → Analytics (infra metrics) ──────────────────────

/// Infrastructure analytics event for ALICE-Analytics.
///
/// Emitted after each apply to track infrastructure churn, deployment
/// frequency, and error rates over time.
pub struct TerraformAnalyticsInfraMetrics {
    /// FNV-1a hash of workspace_hash + run_id — deduplication key.
    pub content_hash: u64,
    /// FNV-1a hash of the workspace name.
    pub workspace_hash: u64,
    /// Run identifier.
    pub run_id: u64,
    /// Total managed resources post-apply.
    pub resource_count: u32,
    /// Resources changed.
    pub change_count: u32,
    /// Resources created.
    pub create_count: u32,
    /// Resources destroyed.
    pub destroy_count: u32,
    /// Apply duration in milliseconds.
    pub apply_duration_ms: u64,
    /// Provider API call count during apply.
    pub api_call_count: u32,
    /// Whether the apply succeeded.
    pub success: bool,
}

/// Build a `TerraformAnalyticsInfraMetrics` event for an apply run.
#[inline]
#[must_use]
pub fn terraform_to_analytics_infra_metrics(
    workspace: &str,
    run_id: u64,
    resource_count: u32,
    change_count: u32,
    create_count: u32,
    destroy_count: u32,
    apply_duration_ms: u64,
    api_call_count: u32,
    success: bool,
) -> TerraformAnalyticsInfraMetrics {
    let workspace_hash = fnv1a(workspace.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&workspace_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&run_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    TerraformAnalyticsInfraMetrics {
        content_hash,
        workspace_hash,
        run_id,
        resource_count,
        change_count,
        create_count,
        destroy_count,
        apply_duration_ms,
        api_call_count,
        success,
    }
}

// ── Bridge 4: Terraform → Monitor (drift detection) ──────────────────────

/// Drift detection event for ALICE-Monitor.
///
/// Emitted when the live infrastructure state diverges from the last
/// known Terraform state. Severity is determined by the destroy count
/// (destructive drift is highest severity).
pub struct TerraformMonitorDriftEvent {
    /// FNV-1a hash of workspace_hash + detection_timestamp_us — event key.
    pub content_hash: u64,
    /// FNV-1a hash of the workspace name.
    pub workspace_hash: u64,
    /// Timestamp of drift detection in microseconds since epoch.
    pub detection_timestamp_us: u64,
    /// Number of resources found to have drifted.
    pub drifted_resource_count: u32,
    /// Number of drifted resources that represent unexpected destroys.
    pub unexpected_destroy_count: u32,
    /// Number of drifted resources that represent unexpected creates.
    pub unexpected_create_count: u32,
    /// FNV-1a hash of the expected state at time of check.
    pub expected_state_hash: u64,
    /// Severity level: 0 = info, 1 = warning, 2 = critical.
    pub severity: u8,
    /// Whether automated remediation has been triggered.
    pub remediation_triggered: bool,
}

/// Build a `TerraformMonitorDriftEvent` for a detected drift.
///
/// `severity` is 2 (critical) if `unexpected_destroy_count > 0`,
/// 1 (warning) if `drifted_resource_count > 0`, else 0 (info) — branchless.
#[inline]
#[must_use]
pub fn terraform_to_monitor_drift_event(
    workspace: &str,
    detection_timestamp_us: u64,
    drifted_resource_count: u32,
    unexpected_destroy_count: u32,
    unexpected_create_count: u32,
    expected_state_hash: u64,
    remediation_triggered: bool,
) -> TerraformMonitorDriftEvent {
    let workspace_hash = fnv1a(workspace.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&workspace_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&detection_timestamp_us.to_le_bytes());
    let content_hash = fnv1a(&buf);
    // Branchless severity: any unexpected destroy → 2, any drift → 1, else 0.
    let has_drift = (drifted_resource_count > 0) as u8;
    let has_destroy = (unexpected_destroy_count > 0) as u8;
    let severity = has_drift + has_destroy;
    TerraformMonitorDriftEvent {
        content_hash,
        workspace_hash,
        detection_timestamp_us,
        drifted_resource_count,
        unexpected_destroy_count,
        unexpected_create_count,
        expected_state_hash,
        severity,
        remediation_triggered,
    }
}

// ── Bridge 5: Terraform → Notify (change alerts) ─────────────────────────

/// Change alert notification for ALICE-Notify.
///
/// Dispatched to notification channels (Slack, PagerDuty, email) when an
/// apply or drift event meets the alert threshold.
pub struct TerraformNotifyChangeAlert {
    /// FNV-1a hash of workspace_hash + run_id — notification identity key.
    pub content_hash: u64,
    /// FNV-1a hash of the workspace name.
    pub workspace_hash: u64,
    /// Run or drift-event identifier.
    pub run_id: u64,
    /// Number of changed resources triggering this alert.
    pub change_count: u32,
    /// Number of destroyed resources (drives urgency).
    pub destroy_count: u32,
    /// Alert urgency: 0 = low, 1 = medium, 2 = high, 3 = critical.
    pub urgency: u8,
    /// Notification channel bitmask (bit 0 = Slack, bit 1 = email, bit 2 = PagerDuty).
    pub channel_mask: u8,
    /// Whether this alert requires acknowledgement before auto-closing.
    pub requires_ack: bool,
    /// FNV-1a hash of the notification template identifier string.
    pub template_hash: u64,
}

/// Build a `TerraformNotifyChangeAlert` for an apply or drift event.
///
/// `urgency` is 3 if `destroy_count > 10`, 2 if `destroy_count > 0`,
/// 1 if `change_count > 0`, else 0 — branchless via saturating arithmetic.
/// `requires_ack` is true when urgency >= 2.
#[inline]
#[must_use]
pub fn terraform_to_notify_change_alert(
    workspace: &str,
    run_id: u64,
    change_count: u32,
    destroy_count: u32,
    channel_mask: u8,
    template: &str,
) -> TerraformNotifyChangeAlert {
    let workspace_hash = fnv1a(workspace.as_bytes());
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&workspace_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&run_id.to_le_bytes());
    let content_hash = fnv1a(&buf);
    let template_hash = fnv1a(template.as_bytes());
    // Branchless urgency: each tier adds 1.
    let has_changes = (change_count > 0) as u8;
    let has_destroys = (destroy_count > 0) as u8;
    let many_destroys = (destroy_count > 10) as u8;
    let urgency = (has_changes + has_destroys + many_destroys).min(3);
    let requires_ack = urgency >= 2;
    TerraformNotifyChangeAlert {
        content_hash,
        workspace_hash,
        run_id,
        change_count,
        destroy_count,
        urgency,
        channel_mask,
        requires_ack,
        template_hash,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terraform_db_state_record_hash_nonzero() {
        let rec = terraform_to_db_state_record("prod", 1, 100, 5, 3, 1, 0xabc, 30_000, true);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.workspace_hash, 0);
    }

    #[test]
    fn test_terraform_db_state_record_deterministic() {
        let a = terraform_to_db_state_record("staging", 42, 50, 2, 1, 0, 0xdead, 5_000, true);
        let b = terraform_to_db_state_record("staging", 42, 50, 2, 1, 0, 0xdead, 5_000, true);
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.workspace_hash, b.workspace_hash);
    }

    #[test]
    fn test_terraform_plan_cache_no_destroy_ttl() {
        let entry = terraform_to_plan_cache_entry("dev", 0xbeef, 3, 3, 0, 1500);
        assert!(!entry.has_destroys);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_terraform_plan_cache_with_destroy_ttl() {
        let entry = terraform_to_plan_cache_entry("prod", 0xcafe, 5, 2, 2, 2000);
        assert!(entry.has_destroys);
        assert_eq!(entry.ttl_secs, 60);
    }

    #[test]
    fn test_terraform_analytics_infra_metrics_fields() {
        let m = terraform_to_analytics_infra_metrics("prod", 10, 200, 8, 5, 1, 45_000, 320, true);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.resource_count, 200);
        assert!(m.success);
    }

    #[test]
    fn test_terraform_monitor_drift_severity_critical() {
        let ev = terraform_to_monitor_drift_event("prod", 0, 3, 2, 1, 0xabc, false);
        assert_eq!(ev.severity, 2);
    }

    #[test]
    fn test_terraform_monitor_drift_severity_warning() {
        let ev = terraform_to_monitor_drift_event("prod", 0, 1, 0, 1, 0xabc, false);
        assert_eq!(ev.severity, 1);
    }

    #[test]
    fn test_terraform_notify_change_alert_urgency_and_ack() {
        // 15 destroys → many_destroys tier → urgency = 3, requires_ack = true.
        let alert = terraform_to_notify_change_alert("prod", 99, 15, 15, 0b111, "tmpl-critical");
        assert_eq!(alert.urgency, 3);
        assert!(alert.requires_ack);
        assert_ne!(alert.template_hash, 0);

        // No changes → urgency = 0, no ack needed.
        let quiet = terraform_to_notify_change_alert("dev", 1, 0, 0, 0b001, "tmpl-info");
        assert_eq!(quiet.urgency, 0);
        assert!(!quiet.requires_ack);
    }
}
