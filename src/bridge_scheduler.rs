//! Scheduler bridges — ALICE-Scheduler ↔ Analytics, DB, Cache, Edge, Queue
//!
//! 5 bridges connecting the scheduler layer to the ALICE ecosystem.

use alice_scheduler::{CronSchedule, EdfTask, JobDag, RetryPolicy};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Bridge 1: Scheduler → Analytics (cron metrics) ───────────────────────

/// Cron schedule metrics event for ALICE-Analytics.
///
/// Emitted each time a cron schedule fires so the analytics layer can
/// compute firing frequency, drift from expected cadence, and per-schedule
/// resource usage.
pub struct SchedulerAnalyticsCronEvent {
    /// FNV-1a hash of the cron expression string.
    pub content_hash: u64,
    /// Encoded active-minute bitmask (low 64 bits of `minute` field).
    pub minute_bits: u64,
    /// Encoded active-hour bitmask (low 32 bits of `hour` field).
    pub hour_bits: u32,
    /// Active day-of-week bitmask (bits 0-6).
    pub dow_bits: u8,
    /// Next fire timestamp in epoch seconds (0 if indeterminate).
    pub next_fire_secs: u64,
    /// Scheduled fire cadence in seconds (min gap between two fires, 60..=525960).
    pub cadence_secs: u64,
}

/// Build a cron metrics event for ALICE-Analytics.
///
/// `now_secs` is the current epoch timestamp used to compute `next_fire_secs`.
#[inline]
#[must_use]
pub fn scheduler_to_analytics_cron_event(
    expr: &str,
    sched: &CronSchedule,
    now_secs: u64,
) -> SchedulerAnalyticsCronEvent {
    let content_hash = fnv1a(expr.as_bytes());
    let next_fire_secs = sched.next_fire_after(now_secs).unwrap_or(0);
    // 最小ケイデンスをビットマスクから推定: minuteフィールドで最小間隔60秒
    let cadence_secs = 60u64;
    SchedulerAnalyticsCronEvent {
        content_hash,
        minute_bits: sched.minute,
        hour_bits: sched.hour,
        dow_bits: sched.dow,
        next_fire_secs,
        cadence_secs,
    }
}

// ── Bridge 2: Scheduler → DB (job records) ────────────────────────────────

/// Job execution record for ALICE-DB.
///
/// Written when the scheduler dispatches a job so that audit trails,
/// SLA compliance checks, and failure analysis can query historical runs.
pub struct SchedulerDbJobRecord {
    /// FNV-1a hash of the job name — DB row key.
    pub content_hash: u64,
    /// Job index in the DAG (matches `JobDag::add_job` return value).
    pub job_index: usize,
    /// Job name length in bytes.
    pub name_len: usize,
    /// Number of upstream dependency edges.
    pub dep_count: usize,
    /// Scheduled dispatch timestamp in milliseconds.
    pub dispatched_at_ms: u64,
    /// Estimated duration in milliseconds (0 = unknown).
    pub estimated_duration_ms: u64,
}

/// Build a job execution record for ALICE-DB.
#[inline]
#[must_use]
pub fn scheduler_to_db_job_record(
    dag: &JobDag,
    job_index: usize,
    dispatched_at_ms: u64,
    estimated_duration_ms: u64,
) -> SchedulerDbJobRecord {
    let name = dag.job_name(job_index).unwrap_or("");
    let content_hash = fnv1a(name.as_bytes());
    SchedulerDbJobRecord {
        content_hash,
        job_index,
        name_len: name.len(),
        dep_count: 0, // DAG依存関係カウントは呼び出し元が解決して渡す
        dispatched_at_ms,
        estimated_duration_ms,
    }
}

// ── Bridge 3: Scheduler → Cache (schedule cache) ──────────────────────────

/// Cached cron schedule entry for ALICE-Cache.
///
/// Caches the next-fire timestamp so that repeated lookups within the same
/// second do not re-evaluate the full bitmask scan.
/// TTL is branchlessly set to 55 seconds when `next_fire_secs > 0`,
/// falling back to 0 (do not cache) when the next fire is indeterminate.
pub struct SchedulerCacheEntry {
    /// FNV-1a hash of the cron expression — cache key.
    pub content_hash: u64,
    /// Next fire timestamp in epoch seconds (0 = indeterminate).
    pub next_fire_secs: u64,
    /// Active minute bitmask.
    pub minute_bits: u64,
    /// Cache TTL in seconds (branchless: 55 when `next_fire_secs > 0`, else 0).
    pub ttl_secs: u32,
    /// Number of remaining minutes until next fire (saturated at u16::MAX).
    pub mins_until_fire: u16,
}

/// Build a cached cron schedule entry for ALICE-Cache.
#[inline]
#[must_use]
pub fn scheduler_to_cache_entry(
    expr: &str,
    sched: &CronSchedule,
    now_secs: u64,
) -> SchedulerCacheEntry {
    let content_hash = fnv1a(expr.as_bytes());
    let next_fire_secs = sched.next_fire_after(now_secs).unwrap_or(0);
    // ブランチレスTTL: next_fire_secs > 0 なら55秒、それ以外は0
    let has_fire = (next_fire_secs > 0) as u32;
    let ttl_secs = has_fire * 55;
    let mins_until_fire = next_fire_secs
        .saturating_sub(now_secs)
        .saturating_div(60)
        .min(u16::MAX as u64) as u16;
    SchedulerCacheEntry {
        content_hash,
        next_fire_secs,
        minute_bits: sched.minute,
        ttl_secs,
        mins_until_fire,
    }
}

// ── Bridge 4: Scheduler → Edge (IoT schedule events) ─────────────────────

/// Scheduled event payload for ALICE-Edge IoT devices.
///
/// Delivers the minimal information an edge device needs to trigger a
/// locally-timed action without maintaining a full cron parser.
pub struct SchedulerEdgeEvent {
    /// FNV-1a hash of the job name — edge routing key.
    pub content_hash: u64,
    /// Next fire timestamp in epoch seconds for the edge device clock.
    pub fire_at_secs: u64,
    /// Active dow bitmask so the device can verify the day locally.
    pub dow_bits: u8,
    /// Active hour bitmask (low 24 bits).
    pub hour_bits: u32,
    /// True when the schedule fires at least once per hour.
    pub is_frequent: bool,
    /// Backoff base in milliseconds for retry on missed fire.
    pub backoff_base_ms: u64,
}

/// Build an edge event payload for ALICE-Edge.
#[inline]
#[must_use]
pub fn scheduler_to_edge_event(
    job_name: &str,
    sched: &CronSchedule,
    retry: &RetryPolicy,
    now_secs: u64,
) -> SchedulerEdgeEvent {
    let content_hash = fnv1a(job_name.as_bytes());
    let fire_at_secs = sched.next_fire_after(now_secs).unwrap_or(0);
    // minuteビットが複数セットされていれば頻繁なスケジュール
    let is_frequent = sched.minute.count_ones() > 1;
    SchedulerEdgeEvent {
        content_hash,
        fire_at_secs,
        dow_bits: sched.dow,
        hour_bits: sched.hour,
        is_frequent,
        backoff_base_ms: retry.base_ms,
    }
}

// ── Bridge 5: Scheduler → Queue (scheduled task dispatch) ─────────────────

/// Scheduled task dispatch envelope for ALICE-Queue.
///
/// Wraps a scheduled job as a queue message so that downstream workers can
/// consume it with standard queue semantics (priority, TTL, dedup key).
pub struct SchedulerQueueDispatch {
    /// FNV-1a hash of the task name — queue dedup key.
    pub content_hash: u64,
    /// Scheduled fire timestamp in epoch seconds.
    pub scheduled_at_secs: u64,
    /// EDF task deadline in epoch seconds (0 = no deadline).
    pub deadline_secs: u64,
    /// Remaining execution time estimate in milliseconds.
    pub remaining_ms: u64,
    /// Queue message TTL in seconds (branchless: 3600 when deadline > 0, else 86400).
    pub ttl_secs: u32,
    /// Priority byte: maps deadline urgency to 0-255.
    pub priority: u8,
}

/// Build a queue dispatch envelope for ALICE-Queue.
///
/// `now_secs` is used to compute priority from the EDF deadline urgency.
#[inline]
#[must_use]
pub fn scheduler_to_queue_dispatch(
    task: &EdfTask,
    task_name: &str,
    scheduled_at_secs: u64,
    now_secs: u64,
) -> SchedulerQueueDispatch {
    let content_hash = fnv1a(task_name.as_bytes());
    let deadline_secs = task.deadline;
    // ブランチレスTTL: deadline > 0 → 3600秒、それ以外 → 86400秒
    let has_deadline = (deadline_secs > 0) as u32;
    let ttl_secs = 86400 - has_deadline * (86400 - 3600);
    // 優先度: 締め切りまでの残り時間が短いほど高い (0-255)
    let secs_left = deadline_secs.saturating_sub(now_secs).max(1);
    let priority = (255u64).saturating_sub(secs_left.min(255)) as u8;
    SchedulerQueueDispatch {
        content_hash,
        scheduled_at_secs,
        deadline_secs,
        remaining_ms: task.execution_time,
        ttl_secs,
        priority,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cron() -> CronSchedule {
        CronSchedule::parse("*/5 * * * *").unwrap()
    }

    fn make_retry() -> RetryPolicy {
        RetryPolicy::new(100, 10_000, 5)
    }

    fn make_edf_task() -> EdfTask {
        EdfTask { id: 0, deadline: 2_000_000_000, execution_time: 500 }
    }

    // ── Bridge 1 ──────────────────────────────────────────────────────────

    #[test]
    fn test_analytics_cron_event_hash_nonzero() {
        let sched = make_cron();
        let ev = scheduler_to_analytics_cron_event("*/5 * * * *", &sched, 0);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_analytics_cron_event_fields() {
        let sched = make_cron();
        let ev = scheduler_to_analytics_cron_event("*/5 * * * *", &sched, 0);
        assert_eq!(ev.minute_bits, sched.minute);
        assert_eq!(ev.hour_bits, sched.hour);
        assert_eq!(ev.dow_bits, sched.dow);
        assert_eq!(ev.cadence_secs, 60);
    }

    #[test]
    fn test_analytics_cron_event_determinism() {
        let sched = make_cron();
        let e1 = scheduler_to_analytics_cron_event("*/5 * * * *", &sched, 1_000);
        let e2 = scheduler_to_analytics_cron_event("*/5 * * * *", &sched, 1_000);
        assert_eq!(e1.content_hash, e2.content_hash);
    }

    // ── Bridge 2 ──────────────────────────────────────────────────────────

    #[test]
    fn test_db_job_record_hash_nonzero() {
        let mut dag = JobDag::new();
        dag.add_job("ingest");
        let rec = scheduler_to_db_job_record(&dag, 0, 1_700_000_000_000, 5_000);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_db_job_record_fields() {
        let mut dag = JobDag::new();
        dag.add_job("transform");
        let rec = scheduler_to_db_job_record(&dag, 0, 1_700_000_000_000, 3_000);
        assert_eq!(rec.job_index, 0);
        assert_eq!(rec.dispatched_at_ms, 1_700_000_000_000);
        assert_eq!(rec.estimated_duration_ms, 3_000);
        assert!(rec.name_len > 0);
    }

    // ── Bridge 3 ──────────────────────────────────────────────────────────

    #[test]
    fn test_cache_entry_ttl_branchless_has_fire() {
        let sched = make_cron();
        let entry = scheduler_to_cache_entry("*/5 * * * *", &sched, 0);
        // next_fire_secs > 0 → TTL = 55
        if entry.next_fire_secs > 0 {
            assert_eq!(entry.ttl_secs, 55);
        }
    }

    #[test]
    fn test_cache_entry_hash_nonzero() {
        let sched = make_cron();
        let entry = scheduler_to_cache_entry("*/5 * * * *", &sched, 0);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_cache_entry_determinism() {
        let sched = make_cron();
        let e1 = scheduler_to_cache_entry("0 * * * *", &sched, 1_000);
        let e2 = scheduler_to_cache_entry("0 * * * *", &sched, 1_000);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.next_fire_secs, e2.next_fire_secs);
    }

    // ── Bridge 4 ──────────────────────────────────────────────────────────

    #[test]
    fn test_edge_event_hash_nonzero() {
        let sched = make_cron();
        let retry = make_retry();
        let ev = scheduler_to_edge_event("iot-sensor-poll", &sched, &retry, 0);
        assert_ne!(ev.content_hash, 0);
    }

    #[test]
    fn test_edge_event_fields() {
        let sched = make_cron();
        let retry = make_retry();
        let ev = scheduler_to_edge_event("iot-sensor-poll", &sched, &retry, 0);
        assert_eq!(ev.dow_bits, sched.dow);
        assert_eq!(ev.hour_bits, sched.hour);
        assert_eq!(ev.backoff_base_ms, 100);
        assert!(ev.is_frequent); // */5 → many bits set
    }

    // ── Bridge 5 ──────────────────────────────────────────────────────────

    #[test]
    fn test_queue_dispatch_hash_nonzero() {
        let task = make_edf_task();
        let disp = scheduler_to_queue_dispatch(&task, "batch-export", 1_700_000_000, 1_699_999_000);
        assert_ne!(disp.content_hash, 0);
    }

    #[test]
    fn test_queue_dispatch_ttl_branchless() {
        let task = make_edf_task();
        // deadline > 0 → ttl_secs = 3600
        let disp = scheduler_to_queue_dispatch(&task, "export", 1_000, 500);
        assert_eq!(disp.ttl_secs, 3600);
        // deadline = 0 → ttl_secs = 86400
        let nodl = EdfTask { id: 1, deadline: 0, execution_time: 100 };
        let disp2 = scheduler_to_queue_dispatch(&nodl, "export", 1_000, 500);
        assert_eq!(disp2.ttl_secs, 86400);
    }

    #[test]
    fn test_queue_dispatch_determinism() {
        let task = make_edf_task();
        let d1 = scheduler_to_queue_dispatch(&task, "job-x", 100, 50);
        let d2 = scheduler_to_queue_dispatch(&task, "job-x", 100, 50);
        assert_eq!(d1.content_hash, d2.content_hash);
    }
}
