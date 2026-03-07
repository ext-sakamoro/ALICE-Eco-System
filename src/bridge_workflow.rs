//! Workflow bridges — ALICE-Workflow ↔ DB, Analytics, Cache, Queue, Edge
//!
//! 5 bridges connecting the workflow engine to the ALICE ecosystem.
//! Covers workflow record persistence, metric telemetry, state caching,
//! task dispatch via Queue, and edge event forwarding.

use alice_workflow::{StateMachine, Task, TaskStatus, WorkflowDag};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Workflow → DB (workflow records) ───────────────────────────

/// Workflow state record for ALICE-DB.
///
/// Written on every state-machine transition so that workflow progress is
/// durable and auditable.  The `content_hash` is derived from the workflow
/// ID and current state name to detect duplicate writes.
pub struct WorkflowDbRecord {
    /// FNV-1a hash of workflow ID + current state name.
    pub content_hash: u64,
    /// Current state identifier of the state machine.
    pub current_state_hash: u64,
    /// Number of transitions recorded in the history.
    pub history_len: u32,
    /// Number of available events from the current state.
    pub available_events: u32,
    /// True when the state machine is in a terminal state (no available events).
    pub is_terminal: bool,
}

/// Build a workflow DB record from a `StateMachine`.
#[inline]
#[must_use]
pub fn workflow_to_db_record(
    workflow_id: &str,
    sm: &StateMachine,
) -> WorkflowDbRecord {
    let mut key = alloc::vec::Vec::with_capacity(workflow_id.len() + sm.current.len());
    key.extend_from_slice(workflow_id.as_bytes());
    key.extend_from_slice(sm.current.as_bytes());
    let content_hash = fnv1a(&key);
    let current_state_hash = fnv1a(sm.current.as_bytes());
    let available = sm.available_events().len() as u32;
    WorkflowDbRecord {
        content_hash,
        current_state_hash,
        history_len: sm.history().len() as u32,
        available_events: available,
        is_terminal: available == 0,
    }
}

// ── Bridge 2: Workflow → Analytics (workflow metrics) ────────────────────

/// Workflow execution metric event for ALICE-Analytics.
///
/// Emitted after each DAG execution step so the analytics layer can track
/// task throughput, failure rates, and retry budgets.
pub struct WorkflowAnalyticsMetrics {
    /// FNV-1a hash of the workflow DAG identifier.
    pub content_hash: u64,
    /// Total task count in the DAG.
    pub task_count: u32,
    /// Number of completed tasks.
    pub completed_count: u32,
    /// Number of failed tasks.
    pub failed_count: u32,
    /// Completion ratio in permille (completed / total × 1000).
    pub completion_permille: u32,
    /// True when the DAG has any failed tasks.
    pub has_failures: bool,
}

/// Build a workflow analytics metric event from a `WorkflowDag`.
#[inline]
#[must_use]
pub fn workflow_to_analytics_metrics(
    dag_id: &str,
    dag: &WorkflowDag,
    completed_count: u32,
    failed_count: u32,
) -> WorkflowAnalyticsMetrics {
    let content_hash = fnv1a(dag_id.as_bytes());
    let task_count = dag.task_count() as u32;
    let total_safe = task_count.max(1);
    let completion_permille =
        completed_count.min(total_safe).wrapping_mul(1_000) / total_safe;
    WorkflowAnalyticsMetrics {
        content_hash,
        task_count,
        completed_count,
        failed_count,
        completion_permille,
        has_failures: dag.has_failed(),
    }
}

// ── Bridge 3: Workflow → Cache (state cache) ──────────────────────────────

/// Workflow state cache entry for ALICE-Cache.
///
/// Caches the current state-machine state so that hot-path orchestrators
/// can skip DB reads.  TTL is set branchlessly: 30 s for non-terminal states
/// (active workflows), 300 s for terminal states (completed/failed).
pub struct WorkflowCacheState {
    /// FNV-1a hash of the workflow identifier.
    pub content_hash: u64,
    /// FNV-1a hash of the current state name.
    pub state_hash: u64,
    /// Number of transitions in the history (version vector).
    pub history_len: u32,
    /// Cache TTL in seconds (branchless: 300 terminal, 30 active).
    pub ttl_secs: u32,
    /// True when no further transitions are possible.
    pub is_terminal: bool,
}

/// Build a workflow state cache entry for ALICE-Cache from a `StateMachine`.
///
/// `ttl_secs` is branchless: 300 when terminal (no events), else 30.
#[inline]
#[must_use]
pub fn workflow_to_cache_state(
    workflow_id: &str,
    sm: &StateMachine,
) -> WorkflowCacheState {
    let content_hash = fnv1a(workflow_id.as_bytes());
    let state_hash = fnv1a(sm.current.as_bytes());
    let is_terminal = sm.available_events().is_empty();
    let terminal_flag = is_terminal as u32;
    // ブランチレス TTL: 終端=300s, アクティブ=30s
    let ttl_secs = 30 + terminal_flag * 270;
    WorkflowCacheState {
        content_hash,
        state_hash,
        history_len: sm.history().len() as u32,
        ttl_secs,
        is_terminal,
    }
}

// ── Bridge 4: Workflow → Queue (task dispatch) ───────────────────────────

/// Task dispatch message for ALICE-Queue.
///
/// When a DAG task becomes ready for execution, this descriptor is enqueued
/// so that worker processes can pick it up without polling the workflow engine.
pub struct WorkflowQueueTask {
    /// FNV-1a hash of the task identifier.
    pub content_hash: u64,
    /// Task identifier hash (same as content_hash for single-task dispatch).
    pub task_id_hash: u64,
    /// Task status encoded as u8: 0=Pending, 1=Running, 2=Completed, 3=Failed, 4=Skipped.
    pub status: u8,
    /// Retry count for this task.
    pub retries: u32,
    /// Maximum allowed retries.
    pub max_retries: u32,
    /// Number of task dependencies.
    pub dep_count: u32,
}

/// Build a task dispatch message for ALICE-Queue from a `Task`.
#[inline]
#[must_use]
pub fn workflow_to_queue_task(task: &Task) -> WorkflowQueueTask {
    let task_id_hash = fnv1a(task.id.as_bytes());
    let status: u8 = match task.status {
        TaskStatus::Pending => 0,
        TaskStatus::Running => 1,
        TaskStatus::Completed => 2,
        TaskStatus::Failed => 3,
        TaskStatus::Skipped => 4,
    };
    WorkflowQueueTask {
        content_hash: task_id_hash,
        task_id_hash,
        status,
        retries: task.retries,
        max_retries: task.max_retries,
        dep_count: task.dependencies.len() as u32,
    }
}

// ── Bridge 5: Workflow → Edge (workflow events) ───────────────────────────

/// Workflow progress event for ALICE-Edge.
///
/// Pushed to Edge nodes so that downstream IoT devices or lightweight clients
/// can track workflow progress without querying the core engine directly.
pub struct WorkflowEdgeEvent {
    /// FNV-1a hash of the workflow ID + DAG completion permille.
    pub content_hash: u64,
    /// Workflow DAG completion ratio in permille.
    pub completion_permille: u32,
    /// True when the DAG is fully complete.
    pub is_complete: bool,
    /// True when the DAG has failed tasks.
    pub has_failures: bool,
    /// Total task count.
    pub task_count: u32,
}

/// Build a workflow Edge event from a `WorkflowDag`.
#[inline]
#[must_use]
pub fn workflow_to_edge_event(
    workflow_id: &str,
    dag: &WorkflowDag,
    completed_count: u32,
) -> WorkflowEdgeEvent {
    let task_count = dag.task_count() as u32;
    let total_safe = task_count.max(1);
    let completion_permille =
        completed_count.min(total_safe).wrapping_mul(1_000) / total_safe;
    let mut key_data = [0u8; 4];
    key_data.copy_from_slice(&completion_permille.to_le_bytes());
    let mut hash_input = alloc::vec::Vec::with_capacity(workflow_id.len() + 4);
    hash_input.extend_from_slice(workflow_id.as_bytes());
    hash_input.extend_from_slice(&key_data);
    WorkflowEdgeEvent {
        content_hash: fnv1a(&hash_input),
        completion_permille,
        is_complete: dag.is_complete(),
        has_failures: dag.has_failed(),
        task_count,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_workflow::{StateMachine, Transition, WorkflowDag};

    fn order_sm() -> StateMachine {
        StateMachine::new("created", alloc::vec![
            Transition {
                from: alloc::string::String::from("created"),
                to: alloc::string::String::from("paid"),
                event: alloc::string::String::from("pay"),
                guard: None,
            },
            Transition {
                from: alloc::string::String::from("paid"),
                to: alloc::string::String::from("shipped"),
                event: alloc::string::String::from("ship"),
                guard: None,
            },
        ])
    }

    fn build_dag() -> WorkflowDag {
        let mut dag = WorkflowDag::new();
        dag.add_task("build", alloc::vec![], 3);
        dag.add_task("test", alloc::vec![alloc::string::String::from("build")], 3);
        dag
    }

    // Bridge 1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_workflow_to_db_record_initial() {
        let sm = order_sm();
        let rec = workflow_to_db_record("order-wf", &sm);
        assert_ne!(rec.content_hash, 0);
        assert_ne!(rec.current_state_hash, 0);
        assert_eq!(rec.history_len, 1); // 初期状態のみ
        assert_eq!(rec.available_events, 1); // "pay"
        assert!(!rec.is_terminal);
    }

    #[test]
    fn test_workflow_to_db_record_terminal() {
        let mut sm = order_sm();
        sm.fire("pay").unwrap();
        sm.fire("ship").unwrap();
        let rec = workflow_to_db_record("order-wf", &sm);
        assert_eq!(rec.history_len, 3);
        assert_eq!(rec.available_events, 0);
        assert!(rec.is_terminal);
    }

    // Bridge 2 ───────────────────────────────────────────────────────────

    #[test]
    fn test_workflow_to_analytics_metrics_in_progress() {
        let dag = build_dag();
        let m = workflow_to_analytics_metrics("pipe-1", &dag, 0, 0);
        assert_ne!(m.content_hash, 0);
        assert_eq!(m.task_count, 2);
        assert_eq!(m.completion_permille, 0);
        assert!(!m.has_failures);
    }

    #[test]
    fn test_workflow_to_analytics_metrics_half_done() {
        let dag = build_dag();
        // 1/2 タスク完了 → 500 permille
        let m = workflow_to_analytics_metrics("pipe-2", &dag, 1, 0);
        assert_eq!(m.completion_permille, 500);
    }

    // Bridge 3 ───────────────────────────────────────────────────────────

    #[test]
    fn test_workflow_to_cache_state_active_ttl() {
        let sm = order_sm();
        let entry = workflow_to_cache_state("wf-active", &sm);
        assert_ne!(entry.content_hash, 0);
        assert_eq!(entry.ttl_secs, 30); // アクティブ → 30s
        assert!(!entry.is_terminal);
    }

    #[test]
    fn test_workflow_to_cache_state_terminal_ttl() {
        let mut sm = order_sm();
        sm.fire("pay").unwrap();
        sm.fire("ship").unwrap();
        let entry = workflow_to_cache_state("wf-done", &sm);
        assert_eq!(entry.ttl_secs, 300); // 終端 → 300s
        assert!(entry.is_terminal);
    }

    #[test]
    fn test_workflow_to_cache_state_determinism() {
        let sm = order_sm();
        let e1 = workflow_to_cache_state("wf-x", &sm);
        let e2 = workflow_to_cache_state("wf-x", &sm);
        assert_eq!(e1.content_hash, e2.content_hash);
        assert_eq!(e1.ttl_secs, e2.ttl_secs);
    }

    // Bridge 4 ───────────────────────────────────────────────────────────

    #[test]
    fn test_workflow_to_queue_task_pending() {
        let task = Task {
            id: alloc::string::String::from("build"),
            dependencies: alloc::vec![],
            status: TaskStatus::Pending,
            retries: 0,
            max_retries: 3,
        };
        let q = workflow_to_queue_task(&task);
        assert_ne!(q.content_hash, 0);
        assert_eq!(q.status, 0); // Pending
        assert_eq!(q.retries, 0);
        assert_eq!(q.max_retries, 3);
        assert_eq!(q.dep_count, 0);
    }

    #[test]
    fn test_workflow_to_queue_task_status_mapping() {
        let statuses = [
            (TaskStatus::Pending, 0u8),
            (TaskStatus::Running, 1),
            (TaskStatus::Completed, 2),
            (TaskStatus::Failed, 3),
            (TaskStatus::Skipped, 4),
        ];
        for (status, expected_code) in statuses {
            let task = Task {
                id: alloc::string::String::from("t"),
                dependencies: alloc::vec![],
                status,
                retries: 0,
                max_retries: 1,
            };
            assert_eq!(workflow_to_queue_task(&task).status, expected_code);
        }
    }

    // Bridge 5 ───────────────────────────────────────────────────────────

    #[test]
    fn test_workflow_to_edge_event_incomplete() {
        let dag = build_dag();
        let ev = workflow_to_edge_event("pipe-edge", &dag, 0);
        assert_ne!(ev.content_hash, 0);
        assert_eq!(ev.task_count, 2);
        assert_eq!(ev.completion_permille, 0);
        assert!(!ev.is_complete);
    }

    #[test]
    fn test_workflow_to_edge_event_complete() {
        let mut dag = build_dag();
        dag.complete_task("build").unwrap();
        dag.complete_task("test").unwrap();
        let ev = workflow_to_edge_event("pipe-done", &dag, 2);
        assert_eq!(ev.completion_permille, 1_000);
        assert!(ev.is_complete);
        assert!(!ev.has_failures);
    }
}

extern crate alloc;
