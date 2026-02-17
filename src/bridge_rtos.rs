//! RTOS bridges — ALICE-RTOS ↔ Edge, Queue, Container, Analytics, DB
//!
//! 5 bridges connecting the math-first RTOS to the ALICE ecosystem.

use alice_rtos::{Kernel, Task, TaskPriority};
use alice_rtos::kernel::KernelStats;

// ── Bridge 1: RTOS → Edge (real-time sensor scheduling) ─────────────────

/// Sensor acquisition task configuration for ALICE-Edge integration.
pub struct EdgeSensorTask {
    /// Task name.
    pub name: [u8; 8],
    /// Sampling period in microseconds.
    pub period_us: u32,
    /// Worst-case execution time in microseconds.
    pub wcet_us: u32,
    /// Priority level.
    pub priority: u8,
    /// ALICE-Edge compression mode (0=linear, 1=polynomial, 2=fourier).
    pub compression_mode: u8,
}

/// Create an ALICE-RTOS task for ALICE-Edge sensor acquisition.
pub fn rtos_edge_sensor_task(
    name: &[u8],
    period_us: u32,
    wcet_us: u32,
    priority: TaskPriority,
    func: fn(&mut [u8]),
) -> Task {
    Task::new(name, func, priority, period_us, wcet_us)
}

/// Configure a sensor acquisition schedule and verify schedulability.
pub fn rtos_edge_schedule(tasks: &[EdgeSensorTask]) -> RtosScheduleResult {
    let mut kernel = Kernel::testing();
    let dummy_fn: fn(&mut [u8]) = |_| {};
    for t in tasks {
        kernel.add_task(&t.name, dummy_fn, TaskPriority(t.priority), t.period_us, t.wcet_us);
    }
    let schedulable = kernel.is_schedulable();
    let stats = kernel.run_for(1_000_000, 100); // 1 second simulation

    RtosScheduleResult {
        schedulable,
        total_utilization: stats.utilization,
        tasks_executed: stats.tasks_executed as usize,
        context_switches: stats.context_switches as usize,
    }
}

/// Schedule analysis result.
pub struct RtosScheduleResult {
    pub schedulable: bool,
    pub total_utilization: f32,
    pub tasks_executed: usize,
    pub context_switches: usize,
}

// ── Bridge 2: RTOS → Queue (SPSC → message pipeline) ───────────────────

/// Queue message descriptor for inter-task → inter-service bridging.
pub struct RtosQueueMessage {
    /// Task ID that produced the message.
    pub source_task: u8,
    /// Message payload (u32 from SpscRing).
    pub payload: u32,
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Convert RTOS SpscRing u32 values to queue messages for ALICE-Queue.
pub fn rtos_to_queue_messages(values: &[u32], source_task: u8, base_time_us: u64, period_us: u32) -> Vec<RtosQueueMessage> {
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| RtosQueueMessage {
            source_task,
            payload: v,
            timestamp_us: base_time_us + i as u64 * period_us as u64,
        })
        .collect()
}

// ── Bridge 3: RTOS → Container (lightweight runtime) ────────────────────

/// Container resource limits derived from RTOS kernel analysis.
pub struct RtosContainerLimits {
    /// Memory footprint in bytes.
    pub memory_bytes: usize,
    /// CPU utilization (0.0..1.0).
    pub cpu_utilization: f32,
    /// Whether the task set is schedulable.
    pub schedulable: bool,
    /// Number of tasks.
    pub task_count: usize,
    /// Minimum tick period (microseconds).
    pub min_period_us: u32,
}

/// Analyze RTOS kernel for ALICE-Container resource allocation.
pub fn rtos_to_container_limits(stats: &KernelStats, task_count: usize, min_period_us: u32) -> RtosContainerLimits {
    RtosContainerLimits {
        memory_bytes: 2048 + task_count * 32, // Kernel + task table
        cpu_utilization: stats.utilization,
        schedulable: stats.schedulable,
        task_count,
        min_period_us,
    }
}

// ── Bridge 4: RTOS → Analytics (kernel telemetry) ──────────────────────

/// Telemetry record from RTOS kernel for ALICE-Analytics.
pub struct RtosTelemetryRecord {
    /// Simulation duration in microseconds.
    pub duration_us: u64,
    /// Total ticks processed.
    pub total_ticks: u64,
    /// Tasks executed.
    pub tasks_executed: usize,
    /// Context switches.
    pub context_switches: usize,
    /// CPU utilization.
    pub utilization: f32,
    /// Schedulability status.
    pub schedulable: bool,
}

/// Convert KernelStats to telemetry record for ALICE-Analytics DDSketch/HLL.
pub fn rtos_to_analytics_telemetry(stats: &KernelStats) -> RtosTelemetryRecord {
    RtosTelemetryRecord {
        duration_us: stats.total_us,
        total_ticks: stats.total_ticks,
        tasks_executed: stats.tasks_executed as usize,
        context_switches: stats.context_switches as usize,
        utilization: stats.utilization,
        schedulable: stats.schedulable,
    }
}

// ── Bridge 5: RTOS → DB (task execution log) ────────────────────────────

/// Task execution log entry for ALICE-DB persistence.
pub struct RtosDbLogEntry {
    /// Timestamp (microseconds since kernel start).
    pub timestamp_us: i64,
    /// Compact log value: (tasks_executed << 16) | context_switches.
    pub compact_value: f32,
    /// Utilization.
    pub utilization: f32,
}

/// Convert KernelStats to DB log entries for ALICE-DB.
pub fn rtos_to_db_log(stats: &KernelStats, timestamp_us: i64) -> RtosDbLogEntry {
    let compact = ((stats.tasks_executed as u32) << 16 | (stats.context_switches as u32 & 0xFFFF)) as f32;
    RtosDbLogEntry {
        timestamp_us,
        compact_value: compact,
        utilization: stats.utilization,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alice_rtos::SpscRing;

    fn dummy_task(_: &mut [u8]) {}

    #[test]
    fn test_rtos_edge_sensor_task() {
        let task = rtos_edge_sensor_task(b"temp", 10_000, 500, TaskPriority::NORMAL, dummy_task);
        assert!(task.is_active());
        assert!((task.utilization() - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_rtos_edge_schedule() {
        let tasks = vec![
            EdgeSensorTask { name: *b"temperat", period_us: 10_000, wcet_us: 500, priority: 2, compression_mode: 0 },
            EdgeSensorTask { name: *b"pressure", period_us: 50_000, wcet_us: 1000, priority: 3, compression_mode: 0 },
        ];
        let result = rtos_edge_schedule(&tasks);
        assert!(result.schedulable);
        assert!(result.total_utilization < 1.0);
        assert!(result.tasks_executed > 0);
    }

    #[test]
    fn test_rtos_to_queue_messages() {
        let values = vec![100, 200, 300];
        let msgs = rtos_to_queue_messages(&values, 1, 0, 10_000);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].payload, 100);
        assert_eq!(msgs[1].timestamp_us, 10_000);
        assert_eq!(msgs[2].timestamp_us, 20_000);
    }

    #[test]
    fn test_rtos_to_container_limits() {
        let stats = KernelStats {
            total_us: 1_000_000,
            total_ticks: 10_000,
            tasks_executed: 500,
            context_switches: 200,
            utilization: 0.35,
            schedulable: true,
        };
        let limits = rtos_to_container_limits(&stats, 4, 1000);
        assert_eq!(limits.task_count, 4);
        assert_eq!(limits.memory_bytes, 2048 + 4 * 32);
        assert!(limits.schedulable);
    }

    #[test]
    fn test_rtos_to_analytics_telemetry() {
        let stats = KernelStats {
            total_us: 500_000,
            total_ticks: 5000,
            tasks_executed: 250,
            context_switches: 100,
            utilization: 0.42,
            schedulable: true,
        };
        let record = rtos_to_analytics_telemetry(&stats);
        assert_eq!(record.duration_us, 500_000);
        assert_eq!(record.tasks_executed, 250);
        assert!((record.utilization - 0.42).abs() < 0.001);
    }

    #[test]
    fn test_rtos_to_db_log() {
        let stats = KernelStats {
            total_us: 1_000_000,
            total_ticks: 10_000,
            tasks_executed: 100,
            context_switches: 50,
            utilization: 0.25,
            schedulable: true,
        };
        let entry = rtos_to_db_log(&stats, 1_000_000);
        assert_eq!(entry.timestamp_us, 1_000_000);
        assert!((entry.utilization - 0.25).abs() < 0.001);
    }
}
