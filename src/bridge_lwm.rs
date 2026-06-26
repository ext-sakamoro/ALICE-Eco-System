//! Language World Model (LWM) bridges — ALICE-GameEngine
//! `environment_trajectory` ↔ Agent dispatch / Cache / DB / Analytics / LLM
//! token budget.
//!
//! 5 bridges connecting the language-world-model abstraction defined in
//! `alice_game_engine::environment_trajectory` to downstream services
//! (action dispatch, prediction cache, trajectory persistence, telemetry
//! pipeline, and LLM judge token planning). Each bridge produces a
//! content-hashed POD record; cache entries use a branchless TTL.

use alice_game_engine::environment_trajectory::{Action, EnvironmentSchema, Observation, Turn};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Action → Agent action dispatch record ─────────────────────

/// Agent action dispatch record produced from an LWM `Action`.
pub struct LwmAgentActionDispatch {
    /// FNV-1a hash over kind + payload length + payload bytes.
    pub content_hash: u64,
    /// Action kind tag (lifted from `Action::Custom { kind, .. }`).
    pub kind: u32,
    /// Payload size in bytes.
    pub payload_size: u32,
    /// Priority bucket (0=low, 1=normal, 2=high, 3=critical).
    pub priority: u8,
}

/// Convert an LWM `Action` into an agent action dispatch record.
///
/// `priority` is supplied by the caller; the bridge only computes the
/// content hash and payload size.
#[inline]
#[must_use]
pub fn lwm_action_to_agent_dispatch(action: &Action, priority: u8) -> LwmAgentActionDispatch {
    let kind = action.kind();
    let payload = action.payload();
    let payload_size = u32::try_from(payload.len()).unwrap_or(u32::MAX);

    let mut header = [0u8; 9];
    header[0..4].copy_from_slice(&kind.to_le_bytes());
    header[4..8].copy_from_slice(&payload_size.to_le_bytes());
    header[8] = priority;
    let content_hash = fnv1a(&[&header[..], payload].concat());

    LwmAgentActionDispatch {
        content_hash,
        kind,
        payload_size,
        priority,
    }
}

// ── Bridge 2: Observation → Cache entry (branchless TTL) ────────────────

/// Cache entry produced from an LWM-predicted `Observation`.
pub struct LwmObservationCacheEntry {
    /// FNV-1a hash over kind + payload length + payload bytes.
    pub content_hash: u64,
    /// Observation kind tag.
    pub kind: u32,
    /// Payload size in bytes.
    pub payload_size: u32,
    /// Cache TTL in seconds. Shortened by `delta_secs` when the
    /// prediction is uncertain (`uncertain_flag != 0`).
    pub ttl_secs: u32,
}

/// Build a cache entry for an LWM-predicted observation with a
/// branchless TTL: confident predictions keep the full base TTL,
/// uncertain ones are shortened by `delta_secs`.
#[inline]
#[must_use]
pub fn lwm_observation_to_cache_entry(
    observation: &Observation,
    base_ttl_secs: u32,
    delta_secs: u32,
    uncertain_flag: u8,
) -> LwmObservationCacheEntry {
    let kind = observation.kind();
    let payload = observation.payload();
    let payload_size = u32::try_from(payload.len()).unwrap_or(u32::MAX);

    let mut header = [0u8; 9];
    header[0..4].copy_from_slice(&kind.to_le_bytes());
    header[4..8].copy_from_slice(&payload_size.to_le_bytes());
    header[8] = uncertain_flag;
    let content_hash = fnv1a(&[&header[..], payload].concat());

    // Branchless TTL: subtract delta when uncertain_flag is non-zero.
    let condition = u32::from(uncertain_flag != 0);
    let ttl_secs = base_ttl_secs.saturating_sub(condition * delta_secs);

    LwmObservationCacheEntry {
        content_hash,
        kind,
        payload_size,
        ttl_secs,
    }
}

// ── Bridge 3: Turn → DB trajectory record ───────────────────────────────

/// Database record produced from a single `Turn` for trajectory
/// persistence.
pub struct LwmTrajectoryRecord {
    /// FNV-1a hash over the entire turn (action + observation).
    pub content_hash: u64,
    /// Action kind tag.
    pub action_kind: u32,
    /// Observation kind tag.
    pub observation_kind: u32,
    /// Action payload size in bytes.
    pub action_size: u32,
    /// Observation payload size in bytes.
    pub observation_size: u32,
    /// Turn index within the trajectory (caller-supplied).
    pub turn_index: u32,
}

/// Convert an LWM `Turn` into a DB trajectory record.
#[inline]
#[must_use]
pub fn lwm_turn_to_trajectory_record(turn: &Turn, turn_index: u32) -> LwmTrajectoryRecord {
    let action_kind = turn.action.kind();
    let observation_kind = turn.observation.kind();
    let action_payload = turn.action.payload();
    let observation_payload = turn.observation.payload();
    let action_size = u32::try_from(action_payload.len()).unwrap_or(u32::MAX);
    let observation_size = u32::try_from(observation_payload.len()).unwrap_or(u32::MAX);

    let mut header = [0u8; 20];
    header[0..4].copy_from_slice(&action_kind.to_le_bytes());
    header[4..8].copy_from_slice(&observation_kind.to_le_bytes());
    header[8..12].copy_from_slice(&action_size.to_le_bytes());
    header[12..16].copy_from_slice(&observation_size.to_le_bytes());
    header[16..20].copy_from_slice(&turn_index.to_le_bytes());

    let content_hash = fnv1a(&[&header[..], action_payload, observation_payload].concat());

    LwmTrajectoryRecord {
        content_hash,
        action_kind,
        observation_kind,
        action_size,
        observation_size,
        turn_index,
    }
}

// ── Bridge 4: EnvironmentSchema → Analytics telemetry event ─────────────

/// Analytics telemetry event produced from an `EnvironmentSchema`
/// snapshot plus a 5-dimensional rubric score.
pub struct LwmAnalyticsTelemetry {
    /// FNV-1a hash over the schema + rubric components.
    pub content_hash: u64,
    /// Schema task description hash.
    pub task_description_hash: u64,
    /// Number of distinct action kinds in the schema.
    pub action_kind_count: u32,
    /// Stateful environment flag (0=stateless, 1=stateful).
    pub stateful_flag: u8,
    /// 5 dimension scores, each scaled to basis points (0–10000) so the
    /// record is integer-only and cache-friendly. Order matches
    /// Qwen-AgentWorld: Format / Factuality / Consistency / Realism /
    /// Quality.
    pub rubric_bps: [u16; 5],
}

/// Build an analytics telemetry event from an `EnvironmentSchema` and
/// the 5-dimension rubric score (each in basis points).
#[inline]
#[must_use]
pub fn lwm_schema_to_analytics_telemetry(
    schema: &EnvironmentSchema,
    rubric_bps: [u16; 5],
) -> LwmAnalyticsTelemetry {
    let action_kind_count = u32::try_from(schema.action_space_kinds.len()).unwrap_or(u32::MAX);
    let stateful_flag = u8::from(schema.stateful);

    let mut header = [0u8; 23];
    header[0..8].copy_from_slice(&schema.task_description_hash.to_le_bytes());
    header[8..12].copy_from_slice(&action_kind_count.to_le_bytes());
    header[12] = stateful_flag;
    for (i, &score) in rubric_bps.iter().enumerate() {
        let off = 13 + i * 2;
        header[off..off + 2].copy_from_slice(&score.to_le_bytes());
    }
    let content_hash = fnv1a(&header);

    LwmAnalyticsTelemetry {
        content_hash,
        task_description_hash: schema.task_description_hash,
        action_kind_count,
        stateful_flag,
        rubric_bps,
    }
}

// ── Bridge 5: Trajectory length → LLM judge token budget forecast ───────

/// Token budget forecast for an LLM judge call evaluating an LWM
/// prediction.
pub struct LwmJudgeTokenForecast {
    /// FNV-1a hash over the forecast inputs.
    pub content_hash: u64,
    /// Estimated prompt tokens (trajectory history + predicted obs +
    /// rubric prompt).
    pub prompt_tokens: u32,
    /// Recommended `max_output_tokens` for the judge response (= 5 dim
    /// rubric JSON + brief justification).
    pub max_output_tokens: u32,
    /// Recommended `thinking_budget_tokens` (= 0 unless extended
    /// reasoning is required).
    pub thinking_budget_tokens: u32,
}

/// Forecast the LLM judge token budget given trajectory length and
/// payload sizes.
///
/// Approximates ~4 bytes per token. `extended_reasoning_flag != 0`
/// allocates a thinking budget proportional to the prompt size; the
/// safe default (= 0) is OFF, matching the `llm-api-cost-guard`
/// guidance.
#[inline]
#[must_use]
pub fn lwm_judge_token_forecast(
    history_payload_bytes: u64,
    predicted_observation_bytes: u32,
    rubric_prompt_bytes: u32,
    extended_reasoning_flag: u8,
) -> LwmJudgeTokenForecast {
    // ~4 bytes per token (Qwen-AgentWorld doc + general english+japanese mix).
    let prompt_bytes = history_payload_bytes
        .saturating_add(u64::from(predicted_observation_bytes))
        .saturating_add(u64::from(rubric_prompt_bytes));
    let prompt_tokens = u32::try_from(prompt_bytes / 4).unwrap_or(u32::MAX);

    // Judge response = 5-dim rubric JSON (~200 tokens) + justification
    // (~600 tokens) capped at 1024 per llm-api-cost-guard §0 default.
    let max_output_tokens: u32 = 1024;

    // Branchless thinking budget: 0 unless flag set, then 2x output budget.
    let condition = u32::from(extended_reasoning_flag != 0);
    let thinking_budget_tokens = condition * max_output_tokens * 2;

    let mut buf = [0u8; 21];
    buf[0..8].copy_from_slice(&prompt_bytes.to_le_bytes());
    buf[8..12].copy_from_slice(&predicted_observation_bytes.to_le_bytes());
    buf[12..16].copy_from_slice(&rubric_prompt_bytes.to_le_bytes());
    buf[16..20].copy_from_slice(&prompt_tokens.to_le_bytes());
    buf[20] = extended_reasoning_flag;
    let content_hash = fnv1a(&buf);

    LwmJudgeTokenForecast {
        content_hash,
        prompt_tokens,
        max_output_tokens,
        thinking_budget_tokens,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_action() -> Action {
        Action::Custom {
            kind: 7,
            payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
        }
    }

    fn sample_observation() -> Observation {
        Observation::Custom {
            kind: 42,
            payload: vec![0x01, 0x02, 0x03],
        }
    }

    fn sample_schema() -> EnvironmentSchema {
        EnvironmentSchema {
            task_description_hash: 0x1234_5678_9ABC_DEF0,
            action_space_kinds: vec![1, 2, 3, 7],
            stateful: true,
        }
    }

    #[test]
    fn bridge1_action_dispatch_carries_kind_and_priority() {
        let action = sample_action();
        let dispatch = lwm_action_to_agent_dispatch(&action, 2);
        assert_ne!(dispatch.content_hash, 0);
        assert_eq!(dispatch.kind, 7);
        assert_eq!(dispatch.payload_size, 4);
        assert_eq!(dispatch.priority, 2);
    }

    #[test]
    fn bridge1_content_hash_is_deterministic() {
        let action = sample_action();
        let a = lwm_action_to_agent_dispatch(&action, 1);
        let b = lwm_action_to_agent_dispatch(&action, 1);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn bridge2_cache_ttl_branchless_full_when_confident() {
        let obs = sample_observation();
        let entry = lwm_observation_to_cache_entry(&obs, 3600, 1800, 0);
        assert_eq!(entry.ttl_secs, 3600);
        assert_eq!(entry.kind, 42);
        assert_eq!(entry.payload_size, 3);
    }

    #[test]
    fn bridge2_cache_ttl_branchless_shortened_when_uncertain() {
        let obs = sample_observation();
        let entry = lwm_observation_to_cache_entry(&obs, 3600, 1800, 1);
        assert_eq!(entry.ttl_secs, 1800);
    }

    #[test]
    fn bridge3_trajectory_record_captures_action_and_observation() {
        let turn = Turn {
            action: sample_action(),
            observation: sample_observation(),
        };
        let record = lwm_turn_to_trajectory_record(&turn, 5);
        assert_ne!(record.content_hash, 0);
        assert_eq!(record.action_kind, 7);
        assert_eq!(record.observation_kind, 42);
        assert_eq!(record.action_size, 4);
        assert_eq!(record.observation_size, 3);
        assert_eq!(record.turn_index, 5);
    }

    #[test]
    fn bridge4_analytics_carries_rubric_bps() {
        let schema = sample_schema();
        let rubric = [9_500, 8_200, 9_100, 8_800, 9_300];
        let event = lwm_schema_to_analytics_telemetry(&schema, rubric);
        assert_ne!(event.content_hash, 0);
        assert_eq!(event.task_description_hash, 0x1234_5678_9ABC_DEF0);
        assert_eq!(event.action_kind_count, 4);
        assert_eq!(event.stateful_flag, 1);
        assert_eq!(event.rubric_bps, rubric);
    }

    #[test]
    fn bridge4_stateful_flag_distinguishes_terminal_from_search() {
        let mut schema = sample_schema();
        schema.stateful = false;
        let event = lwm_schema_to_analytics_telemetry(&schema, [0; 5]);
        assert_eq!(event.stateful_flag, 0);
    }

    #[test]
    fn bridge5_token_forecast_with_reasoning_off() {
        let forecast = lwm_judge_token_forecast(40_000, 2_000, 1_200, 0);
        // (40_000 + 2_000 + 1_200) / 4 = 10_800
        assert_eq!(forecast.prompt_tokens, 10_800);
        assert_eq!(forecast.max_output_tokens, 1024);
        assert_eq!(forecast.thinking_budget_tokens, 0);
        assert_ne!(forecast.content_hash, 0);
    }

    #[test]
    fn bridge5_token_forecast_with_reasoning_on() {
        let forecast = lwm_judge_token_forecast(40_000, 2_000, 1_200, 1);
        assert_eq!(forecast.thinking_budget_tokens, 2048);
    }

    #[test]
    fn bridge1_5_all_content_hashes_unique_for_distinct_inputs() {
        let a1 = lwm_action_to_agent_dispatch(&sample_action(), 1).content_hash;
        let a2 = lwm_action_to_agent_dispatch(&sample_action(), 2).content_hash;
        assert_ne!(a1, a2);
    }

    #[test]
    fn bridge1_5_cache_ttl_saturates_when_delta_exceeds_base() {
        let obs = sample_observation();
        let entry = lwm_observation_to_cache_entry(&obs, 100, 1000, 1);
        assert_eq!(entry.ttl_secs, 0);
    }
}
