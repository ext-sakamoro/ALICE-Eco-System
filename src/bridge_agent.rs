//! Agent bridges — ALICE-Agent ↔ DB, Cache, Analytics, Edge, ML
//!
//! 5 bridges connecting the coding agent harness to the ALICE ecosystem.

use alice_agent::conversation::message::{AgentMessage, Role, ToolCall};
use alice_agent::conversation::session::Session;
use alice_agent::permission::PermissionLevel;
use alice_agent::tools::ToolSpec;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Agent Session → DB (セッション永続化) ────

/// エージェントセッションの DB レコード。
pub struct AgentSessionDbRecord {
    /// Content hash of the session (FNV-1a).
    pub content_hash: u64,
    /// セッション ID。
    pub session_id_hash: u64,
    /// メッセージ数。
    pub message_count: u32,
    /// モデル名ハッシュ。
    pub model_hash: u64,
}

/// `Session` を DB レコードに変換。
#[inline]
#[must_use]
pub fn agent_session_to_db(session: &Session) -> AgentSessionDbRecord {
    let id_hash = fnv1a(session.id.as_bytes());
    let model_hash = fnv1a(session.model_name.as_bytes());
    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&(session.messages.len() as u32).to_le_bytes());
    buf[12..20].copy_from_slice(&model_hash.to_le_bytes());
    AgentSessionDbRecord {
        content_hash: fnv1a(&buf),
        session_id_hash: id_hash,
        message_count: session.messages.len() as u32,
        model_hash,
    }
}

// ── Bridge 2: Agent ToolCall → Analytics (ツール使用メトリクス) ────

/// ツール呼び出しの Analytics エントリ。
pub struct AgentToolAnalyticsEntry {
    /// Content hash of the tool call (FNV-1a).
    pub content_hash: u64,
    /// ツール名ハッシュ。
    pub tool_name_hash: u64,
    /// 呼び出し ID ハッシュ。
    pub call_id_hash: u64,
    /// 入力サイズ (bytes)。
    pub input_size: u32,
}

/// `ToolCall` を Analytics エントリに変換。
#[inline]
#[must_use]
pub fn agent_tool_call_to_analytics(tc: &ToolCall) -> AgentToolAnalyticsEntry {
    let name_hash = fnv1a(tc.name.as_bytes());
    let id_hash = fnv1a(tc.id.as_bytes());
    let input_str = tc.input.to_string();
    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&id_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&(input_str.len() as u32).to_le_bytes());
    AgentToolAnalyticsEntry {
        content_hash: fnv1a(&buf),
        tool_name_hash: name_hash,
        call_id_hash: id_hash,
        input_size: input_str.len() as u32,
    }
}

// ── Bridge 3: Agent Message → Cache (メッセージキャッシュ) ────

/// メッセージのキャッシュエントリ。
pub struct AgentMessageCacheEntry {
    /// Content hash of the message (FNV-1a).
    pub content_hash: u64,
    /// ロール (0=System, 1=User, 2=Assistant, 3=Tool)。
    pub role_id: u8,
    /// コンテンツサイズ (bytes)。
    pub content_size: u32,
    /// ツール呼び出し数。
    pub tool_call_count: u8,
    /// キャッシュ TTL (秒)。
    pub ttl_secs: u32,
}

/// `AgentMessage` をキャッシュエントリに変換。
#[inline]
#[must_use]
pub fn agent_message_to_cache(msg: &AgentMessage) -> AgentMessageCacheEntry {
    let role_id = match msg.role {
        Role::System => 0,
        Role::User => 1,
        Role::Assistant => 2,
        Role::Tool => 3,
    };
    let content_size = msg.content.len() as u32;
    let tool_call_count = msg.tool_calls.len().min(255) as u8;

    let mut buf = [0u8; 9];
    buf[0] = role_id;
    buf[1..5].copy_from_slice(&content_size.to_le_bytes());
    buf[5] = tool_call_count;
    // assistant メッセージは長時間キャッシュ、tool 結果は短命
    let is_tool = (role_id == 3) as u32;
    let ttl_secs = 3600 - is_tool * 3300;
    buf[6..9].copy_from_slice(&ttl_secs.to_le_bytes()[..3]);

    AgentMessageCacheEntry {
        content_hash: fnv1a(&buf),
        role_id,
        content_size,
        tool_call_count,
        ttl_secs,
    }
}

// ── Bridge 4: Agent ToolSpec → Edge (ツール定義のエッジ配信) ────

/// ツール定義のエッジ配信レコード。
pub struct AgentToolEdgeRecord {
    /// Content hash of the tool spec (FNV-1a).
    pub content_hash: u64,
    /// ツール名ハッシュ。
    pub tool_name_hash: u64,
    /// 説明文サイズ (bytes)。
    pub description_size: u32,
    /// パーミッションレベル (0=ReadOnly, 1=WorkspaceWrite, 2=FullAccess)。
    pub permission_level: u8,
}

/// `ToolSpec` をエッジ配信レコードに変換。
#[inline]
#[must_use]
pub fn agent_tool_spec_to_edge(spec: &ToolSpec) -> AgentToolEdgeRecord {
    let name_hash = fnv1a(spec.name.as_bytes());
    let perm = match spec.permission {
        PermissionLevel::ReadOnly => 0,
        PermissionLevel::WorkspaceWrite => 1,
        PermissionLevel::FullAccess => 2,
    };
    let mut buf = [0u8; 13];
    buf[..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&(spec.description.len() as u32).to_le_bytes());
    buf[12] = perm;
    AgentToolEdgeRecord {
        content_hash: fnv1a(&buf),
        tool_name_hash: name_hash,
        description_size: spec.description.len() as u32,
        permission_level: perm,
    }
}

// ── Bridge 5: Agent Session → ML (セッション特徴量) ────

/// セッションの ML 特徴量ベクトル。
pub struct AgentSessionMlFeatures {
    /// Content hash of the features (FNV-1a).
    pub content_hash: u64,
    /// 総メッセージ数。
    pub total_messages: u32,
    /// ユーザーメッセージ数。
    pub user_messages: u32,
    /// アシスタントメッセージ数。
    pub assistant_messages: u32,
    /// ツール呼び出し総数。
    pub total_tool_calls: u32,
}

/// `Session` から ML 特徴量を抽出。
#[inline]
#[must_use]
pub fn agent_session_to_ml(session: &Session) -> AgentSessionMlFeatures {
    let mut user_count: u32 = 0;
    let mut assistant_count: u32 = 0;
    let mut tool_count: u32 = 0;

    for msg in &session.messages {
        match msg.role {
            Role::User => user_count += 1,
            Role::Assistant => {
                assistant_count += 1;
                tool_count += msg.tool_calls.len() as u32;
            }
            _ => {}
        }
    }

    let total = session.messages.len() as u32;
    let mut buf = [0u8; 16];
    buf[..4].copy_from_slice(&total.to_le_bytes());
    buf[4..8].copy_from_slice(&user_count.to_le_bytes());
    buf[8..12].copy_from_slice(&assistant_count.to_le_bytes());
    buf[12..16].copy_from_slice(&tool_count.to_le_bytes());

    AgentSessionMlFeatures {
        content_hash: fnv1a(&buf),
        total_messages: total,
        user_messages: user_count,
        assistant_messages: assistant_count,
        total_tool_calls: tool_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_val(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap()
    }

    fn sample_session() -> Session {
        let mut s = Session::new("/tmp/test", "alice-9b");
        s.messages.push(AgentMessage::user("hello"));
        s.messages.push(AgentMessage::assistant(
            "checking",
            vec![ToolCall {
                id: "c1".to_string(),
                name: "bash".to_string(),
                input: json_val(r#"{"command":"ls"}"#),
            }],
        ));
        s.messages
            .push(AgentMessage::tool_result("c1", "file1.rs", false));
        s
    }

    #[test]
    fn test_session_to_db_hash_nonzero() {
        let s = sample_session();
        let rec = agent_session_to_db(&s);
        assert_ne!(rec.content_hash, 0);
    }

    #[test]
    fn test_session_to_db_message_count() {
        let s = sample_session();
        let rec = agent_session_to_db(&s);
        assert_eq!(rec.message_count, 3);
    }

    #[test]
    fn test_session_to_db_deterministic() {
        let s = sample_session();
        let r1 = agent_session_to_db(&s);
        let r2 = agent_session_to_db(&s);
        assert_eq!(r1.content_hash, r2.content_hash);
    }

    #[test]
    fn test_tool_call_to_analytics_hash_nonzero() {
        let tc = ToolCall {
            id: "call_001".to_string(),
            name: "bash".to_string(),
            input: json_val(r#"{"command":"ls"}"#),
        };
        let entry = agent_tool_call_to_analytics(&tc);
        assert_ne!(entry.content_hash, 0);
        assert!(entry.input_size > 0);
    }

    #[test]
    fn test_message_to_cache_user() {
        let msg = AgentMessage::user("hello world");
        let entry = agent_message_to_cache(&msg);
        assert_eq!(entry.role_id, 1);
        assert_eq!(entry.content_size, 11);
        assert_eq!(entry.tool_call_count, 0);
        assert_eq!(entry.ttl_secs, 3600);
    }

    #[test]
    fn test_message_to_cache_tool_short_ttl() {
        let msg = AgentMessage::tool_result("c1", "output", false);
        let entry = agent_message_to_cache(&msg);
        assert_eq!(entry.role_id, 3);
        assert_eq!(entry.ttl_secs, 300);
    }

    #[test]
    fn test_tool_spec_to_edge() {
        let spec = ToolSpec {
            name: "bash".to_string(),
            description: "Execute a command".to_string(),
            input_schema: json_val("{}"),
            permission: PermissionLevel::FullAccess,
        };
        let rec = agent_tool_spec_to_edge(&spec);
        assert_ne!(rec.content_hash, 0);
        assert_eq!(rec.permission_level, 2);
        assert_eq!(rec.description_size, 17);
    }

    #[test]
    fn test_session_to_ml_features() {
        let s = sample_session();
        let f = agent_session_to_ml(&s);
        assert_eq!(f.total_messages, 3);
        assert_eq!(f.user_messages, 1);
        assert_eq!(f.assistant_messages, 1);
        assert_eq!(f.total_tool_calls, 1);
        assert_ne!(f.content_hash, 0);
    }
}
