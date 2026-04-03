//! Agent-LLM bridges — ALICE-Agent ↔ ALICE-Chat LLM layer
//!
//! 5 bridges connecting the coding agent harness to the Chat LLM pipeline.

use alice_agent::conversation::message::{AgentMessage, Role, ToolCall};
use alice_agent::conversation::session::Session;
use alice_agent::permission::PermissionLevel;
use alice_agent::tools::ToolSpec;
use alice_chat::llm::{LlmMessage, LlmRequest, LlmResponse};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: AgentMessage → LlmMessage (ロール変換) ────────────────────

/// AgentMessage を LlmMessage に変換したブリッジレコード。
pub struct AgentToLlmMessageBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// ロール文字列 ("system" / "user" / "assistant" / "tool")。
    pub role_str: &'static str,
    /// コンテンツサイズ (bytes)。
    pub content_size: u32,
    /// ツール呼び出し数。
    pub tool_call_count: u8,
    /// 変換後の LlmMessage。
    pub llm_message: LlmMessage,
}

/// `AgentMessage` を `LlmMessage` ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn agent_message_to_llm_message(msg: &AgentMessage) -> AgentToLlmMessageBridge {
    let role_str = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };
    let content_size = msg.content.len() as u32;
    let tool_call_count = msg.tool_calls.len().min(255) as u8;

    let mut buf = [0u8; 14];
    buf[..4].copy_from_slice(&content_size.to_le_bytes());
    buf[4] = tool_call_count;
    buf[5..13].copy_from_slice(&fnv1a(role_str.as_bytes()).to_le_bytes());
    buf[13] = role_str.len() as u8;
    let content_hash = fnv1a(&[&buf[..], msg.content.as_bytes()].concat());

    AgentToLlmMessageBridge {
        content_hash,
        role_str,
        content_size,
        tool_call_count,
        llm_message: LlmMessage {
            role: role_str.to_owned(),
            content: msg.content.clone(),
        },
    }
}

// ── Bridge 2: Session → LlmRequest (セッションからリクエスト生成) ────────

/// Session を LlmRequest に変換したブリッジレコード。
pub struct SessionToLlmRequestBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// セッション ID ハッシュ。
    pub session_id_hash: u64,
    /// メッセージ数。
    pub message_count: u32,
    /// 変換後の LlmRequest。
    pub llm_request: LlmRequest,
}

/// `Session` を `LlmRequest` ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn agent_session_to_llm_request(session: &Session) -> SessionToLlmRequestBridge {
    let session_id_hash = fnv1a(session.id.as_bytes());
    let model_hash = fnv1a(session.model_name.as_bytes());
    let message_count = session.messages.len() as u32;

    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&session_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&model_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&message_count.to_le_bytes());
    let content_hash = fnv1a(&buf);

    let messages: Vec<LlmMessage> = session
        .messages
        .iter()
        .map(|m| agent_message_to_llm_message(m).llm_message)
        .collect();

    SessionToLlmRequestBridge {
        content_hash,
        session_id_hash,
        message_count,
        llm_request: LlmRequest {
            model: session.model_name.clone(),
            messages,
            max_tokens: 1024,
            temperature: 0.7,
            system: None,
        },
    }
}

// ── Bridge 3: LlmResponse → AgentMessage (レスポンスをメッセージに逆変換) ─

/// LlmResponse を AgentMessage 互換のブリッジレコードに変換したもの。
pub struct LlmResponseToAgentBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// プロバイダ名ハッシュ。
    pub provider_hash: u64,
    /// 入力トークン数。
    pub input_tokens: u32,
    /// 出力トークン数。
    pub output_tokens: u32,
    /// モデル名ハッシュ。
    pub model_hash: u64,
    /// アシスタントメッセージとして再構成した AgentMessage。
    pub agent_message: AgentMessage,
}

/// `LlmResponse` を `AgentMessage` ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn llm_response_to_agent_message(resp: &LlmResponse) -> LlmResponseToAgentBridge {
    let provider_hash = fnv1a(resp.provider.as_bytes());
    let model_hash = fnv1a(resp.model.as_bytes());
    let content_hash_base = fnv1a(resp.content.as_bytes());

    let mut buf = [0u8; 24];
    buf[..8].copy_from_slice(&provider_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&model_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&resp.input_tokens.to_le_bytes());
    buf[20..24].copy_from_slice(&resp.output_tokens.to_le_bytes());
    let content_hash = fnv1a(&[&buf[..], &content_hash_base.to_le_bytes()].concat());

    LlmResponseToAgentBridge {
        content_hash,
        provider_hash,
        input_tokens: resp.input_tokens,
        output_tokens: resp.output_tokens,
        model_hash,
        agent_message: AgentMessage {
            role: Role::Assistant,
            content: resp.content.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            is_error: false,
        },
    }
}

// ── Bridge 4: ToolCall → LlmMessage (ツール結果をLLMメッセージに変換) ────

/// ToolCall 結果を LlmMessage に変換したブリッジレコード。
pub struct ToolCallToLlmBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// ツール名ハッシュ。
    pub tool_name_hash: u64,
    /// 呼び出し ID ハッシュ。
    pub call_id_hash: u64,
    /// 入力サイズ (bytes)。
    pub input_size: u32,
    /// 変換後の LlmMessage (role="tool")。
    pub llm_message: LlmMessage,
}

/// `ToolCall` を LlmMessage ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn tool_call_to_llm_message(tc: &ToolCall) -> ToolCallToLlmBridge {
    let name_hash = fnv1a(tc.name.as_bytes());
    let call_id_hash = fnv1a(tc.id.as_bytes());
    let input_str = tc.input.to_string();
    let input_size = input_str.len() as u32;

    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&call_id_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&input_size.to_le_bytes());
    let content_hash = fnv1a(&buf);

    let content = format!("[tool:{}] {}", tc.name, input_str);

    ToolCallToLlmBridge {
        content_hash,
        tool_name_hash: name_hash,
        call_id_hash,
        input_size,
        llm_message: LlmMessage {
            role: "tool".to_owned(),
            content,
        },
    }
}

// ── Bridge 5: ToolSpec → LlmMessage (ツール定義をシステムプロンプトに埋込) ─

/// ToolSpec をシステムプロンプト埋め込み用 LlmMessage に変換したブリッジレコード。
pub struct ToolSpecToLlmBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// ツール名ハッシュ。
    pub tool_name_hash: u64,
    /// 説明文サイズ (bytes)。
    pub description_size: u32,
    /// パーミッションレベル (0=ReadOnly, 1=WorkspaceWrite, 2=FullAccess)。
    pub permission_level: u8,
    /// 変換後の LlmMessage (role="system")。
    pub llm_message: LlmMessage,
}

/// `ToolSpec` を LlmMessage ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn tool_spec_to_llm_message(spec: &ToolSpec) -> ToolSpecToLlmBridge {
    let name_hash = fnv1a(spec.name.as_bytes());
    let perm = match spec.permission {
        PermissionLevel::ReadOnly => 0,
        PermissionLevel::WorkspaceWrite => 1,
        PermissionLevel::FullAccess => 2,
    };
    let description_size = spec.description.len() as u32;

    let mut buf = [0u8; 13];
    buf[..8].copy_from_slice(&name_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&description_size.to_le_bytes());
    buf[12] = perm;
    let content_hash = fnv1a(&buf);

    let content = format!(
        "Tool: {}\nDescription: {}\nPermission: {}",
        spec.name,
        spec.description,
        match perm {
            0 => "ReadOnly",
            1 => "WorkspaceWrite",
            _ => "FullAccess",
        }
    );

    ToolSpecToLlmBridge {
        content_hash,
        tool_name_hash: name_hash,
        description_size,
        permission_level: perm,
        llm_message: LlmMessage {
            role: "system".to_owned(),
            content,
        },
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent_message(role: Role, content: &str) -> AgentMessage {
        AgentMessage {
            role,
            content: content.to_owned(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            is_error: false,
        }
    }

    fn make_session() -> Session {
        let mut s = Session::new("/tmp", "test-model");
        s.messages.push(make_agent_message(Role::User, "hello"));
        s.messages.push(make_agent_message(Role::Assistant, "world"));
        s
    }

    // Bridge 1 tests

    #[test]
    fn test_agent_message_to_llm_user() {
        let msg = make_agent_message(Role::User, "テスト");
        let b = agent_message_to_llm_message(&msg);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.role_str, "user");
        assert_eq!(b.llm_message.role, "user");
        assert_eq!(b.llm_message.content, "テスト");
        assert_eq!(b.content_size, "テスト".len() as u32);
    }

    #[test]
    fn test_agent_message_to_llm_assistant() {
        let msg = make_agent_message(Role::Assistant, "回答");
        let b = agent_message_to_llm_message(&msg);
        assert_eq!(b.role_str, "assistant");
        assert_eq!(b.llm_message.role, "assistant");
    }

    #[test]
    fn test_agent_message_to_llm_system() {
        let msg = make_agent_message(Role::System, "システム");
        let b = agent_message_to_llm_message(&msg);
        assert_eq!(b.role_str, "system");
    }

    #[test]
    fn test_agent_message_to_llm_tool() {
        let msg = make_agent_message(Role::Tool, "ツール結果");
        let b = agent_message_to_llm_message(&msg);
        assert_eq!(b.role_str, "tool");
    }

    // Bridge 2 tests

    #[test]
    fn test_session_to_llm_request() {
        let s = make_session();
        let b = agent_session_to_llm_request(&s);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.message_count, 2);
        assert_eq!(b.llm_request.model, "test-model");
        assert_eq!(b.llm_request.messages.len(), 2);
    }

    #[test]
    fn test_session_to_llm_request_hash_deterministic() {
        let s = make_session();
        let b1 = agent_session_to_llm_request(&s);
        let b2 = agent_session_to_llm_request(&s);
        assert_eq!(b1.content_hash, b2.content_hash);
        assert_eq!(b1.session_id_hash, b2.session_id_hash);
    }

    // Bridge 3 tests

    #[test]
    fn test_llm_response_to_agent_message() {
        let resp = LlmResponse {
            content: "生成テキスト".to_owned(),
            model: "gpt-4".to_owned(),
            input_tokens: 10,
            output_tokens: 20,
            provider: "openai".to_owned(),
            finish_reason: None,
        };
        let b = llm_response_to_agent_message(&resp);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.input_tokens, 10);
        assert_eq!(b.output_tokens, 20);
        assert_eq!(b.agent_message.role, Role::Assistant);
        assert_eq!(b.agent_message.content, "生成テキスト");
    }

    #[test]
    fn test_llm_response_hash_deterministic() {
        let resp = LlmResponse {
            content: "abc".to_owned(),
            model: "m".to_owned(),
            input_tokens: 1,
            output_tokens: 2,
            provider: "p".to_owned(),
            finish_reason: None,
        };
        let b1 = llm_response_to_agent_message(&resp);
        let b2 = llm_response_to_agent_message(&resp);
        assert_eq!(b1.content_hash, b2.content_hash);
    }

    // Bridge 4 tests

    #[test]
    fn test_tool_call_to_llm_message() {
        let tc = ToolCall {
            id: "call-1".to_owned(),
            name: "read_file".to_owned(),
            input: serde_json::json!({"path": "/tmp/test.txt"}),
        };
        let b = tool_call_to_llm_message(&tc);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.llm_message.role, "tool");
        assert!(b.llm_message.content.contains("read_file"));
        assert_ne!(b.tool_name_hash, 0);
        assert_ne!(b.call_id_hash, 0);
    }

    // Bridge 5 tests

    #[test]
    fn test_tool_spec_to_llm_message_readonly() {
        let spec = ToolSpec {
            name: "list_files".to_owned(),
            description: "ファイル一覧を取得する".to_owned(),
            input_schema: serde_json::json!({}),
            permission: PermissionLevel::ReadOnly,
        };
        let b = tool_spec_to_llm_message(&spec);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.permission_level, 0);
        assert_eq!(b.llm_message.role, "system");
        assert!(b.llm_message.content.contains("list_files"));
        assert!(b.llm_message.content.contains("ReadOnly"));
    }

    #[test]
    fn test_tool_spec_to_llm_message_fullaccess() {
        let spec = ToolSpec {
            name: "exec".to_owned(),
            description: "コマンド実行".to_owned(),
            input_schema: serde_json::json!({}),
            permission: PermissionLevel::FullAccess,
        };
        let b = tool_spec_to_llm_message(&spec);
        assert_eq!(b.permission_level, 2);
        assert!(b.llm_message.content.contains("FullAccess"));
    }

    #[test]
    fn test_tool_spec_description_size() {
        let desc = "詳細な説明文";
        let spec = ToolSpec {
            name: "tool".to_owned(),
            description: desc.to_owned(),
            input_schema: serde_json::json!({}),
            permission: PermissionLevel::WorkspaceWrite,
        };
        let b = tool_spec_to_llm_message(&spec);
        assert_eq!(b.description_size, desc.len() as u32);
        assert_eq!(b.permission_level, 1);
    }
}
