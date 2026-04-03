//! Chat-LLM bridges — ALICE-Chat ↔ ALICE-Chat LLM layer
//!
//! 5 bridges connecting Chat domain models (Message, User, Channel) to the
//! LLM pipeline (LlmRequest, LlmMessage, LlmResponse).

use alice_chat::llm::{LlmMessage, LlmRequest, LlmResponse};
use alice_chat::models::channel::{Channel, ChannelType};
use alice_chat::models::message::{Message, MessageType};
use alice_chat::models::user::{User, UserRole};

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ── Bridge 1: Message → LlmMessage (チャットメッセージをLLM入力に変換) ────

/// Message を LlmMessage に変換したブリッジレコード。
pub struct MessageToLlmBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// メッセージ ID ハッシュ。
    pub message_id_hash: u64,
    /// 送信者 ID ハッシュ。
    pub sender_id_hash: u64,
    /// コンテンツサイズ (bytes)。
    pub content_size: u32,
    /// 変換後の LlmMessage。
    pub llm_message: LlmMessage,
}

/// `Message` を `LlmMessage` ブリッジレコードに変換。
///
/// MessageType::Text はそのままコンテンツとして使用する。
/// その他の型は "[image]" / "[file:xxx]" / "[system]" として表現する。
#[inline]
#[must_use]
pub fn chat_message_to_llm_message(msg: &Message) -> MessageToLlmBridge {
    let message_id_hash = fnv1a(&msg.id.to_le_bytes());
    let sender_id_hash = fnv1a(&msg.sender_id.to_le_bytes());

    let content = match &msg.content {
        MessageType::Text(text) => text.clone(),
        MessageType::Image { url, .. } => format!("[image: {url}]"),
        MessageType::File { filename, .. } => format!("[file: {filename}]"),
        MessageType::System(text) => format!("[system: {text}]"),
    };
    let content_size = content.len() as u32;

    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&message_id_hash.to_le_bytes());
    buf[8..16].copy_from_slice(&sender_id_hash.to_le_bytes());
    buf[16..20].copy_from_slice(&content_size.to_le_bytes());
    let content_hash = fnv1a(&[&buf[..], content.as_bytes()].concat());

    MessageToLlmBridge {
        content_hash,
        message_id_hash,
        sender_id_hash,
        content_size,
        llm_message: LlmMessage {
            role: "user".to_owned(),
            content,
        },
    }
}

// ── Bridge 2: User → LlmMessage (ユーザー情報をシステムプロンプトに埋込) ──

/// User 情報をシステムプロンプト LlmMessage に変換したブリッジレコード。
pub struct UserToLlmBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// ユーザー ID ハッシュ。
    pub user_id_hash: u64,
    /// ロール ID (0=Admin, 1=Member, 2=Guest, 3=Bot)。
    pub role_id: u8,
    /// アクティブフラグ (1=active, 0=inactive)。
    pub is_active: u8,
    /// 変換後の LlmMessage (role="system")。
    pub llm_message: LlmMessage,
}

/// `User` を LlmMessage (system) ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn chat_user_to_llm_message(user: &User) -> UserToLlmBridge {
    let user_id_hash = fnv1a(&user.id.to_le_bytes());
    let role_id = match user.role {
        UserRole::Admin => 0,
        UserRole::Member => 1,
        UserRole::Guest => 2,
        UserRole::Bot => 3,
    };
    let is_active = user.active as u8;

    let mut buf = [0u8; 10];
    buf[..8].copy_from_slice(&user_id_hash.to_le_bytes());
    buf[8] = role_id;
    buf[9] = is_active;
    let content_hash = fnv1a(&[&buf[..], user.username.as_bytes()].concat());

    let role_str = match user.role {
        UserRole::Admin => "Admin",
        UserRole::Member => "Member",
        UserRole::Guest => "Guest",
        UserRole::Bot => "Bot",
    };
    let content = format!(
        "User: {} ({})\nRole: {}\nActive: {}",
        user.display_name, user.username, role_str, user.active
    );

    UserToLlmBridge {
        content_hash,
        user_id_hash,
        role_id,
        is_active,
        llm_message: LlmMessage {
            role: "system".to_owned(),
            content,
        },
    }
}

// ── Bridge 3: Channel → LlmRequest (チャンネルコンテキストでリクエスト生成) ─

/// Channel コンテキストから LlmRequest を生成したブリッジレコード。
pub struct ChannelToLlmRequestBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// チャンネル ID ハッシュ。
    pub channel_id_hash: u64,
    /// チャンネル種別 ID (0=Direct, 1=Group, 2=Public)。
    pub channel_type_id: u8,
    /// メンバー数。
    pub member_count: u32,
    /// 変換後の LlmRequest。
    pub llm_request: LlmRequest,
}

/// `Channel` を `LlmRequest` ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn chat_channel_to_llm_request(channel: &Channel, model: &str) -> ChannelToLlmRequestBridge {
    let channel_id_hash = fnv1a(&channel.id.to_le_bytes());
    let channel_type_id = match channel.channel_type {
        ChannelType::Direct => 0,
        ChannelType::Group => 1,
        ChannelType::Public => 2,
    };
    let member_count = channel.members.len() as u32;

    let mut buf = [0u8; 13];
    buf[..8].copy_from_slice(&channel_id_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&member_count.to_le_bytes());
    buf[12] = channel_type_id;
    let content_hash = fnv1a(&[&buf[..], channel.name.as_bytes()].concat());

    let system_content = format!(
        "Channel: {} | Type: {} | Members: {}",
        channel.name,
        match channel.channel_type {
            ChannelType::Direct => "Direct",
            ChannelType::Group => "Group",
            ChannelType::Public => "Public",
        },
        member_count
    );

    ChannelToLlmRequestBridge {
        content_hash,
        channel_id_hash,
        channel_type_id,
        member_count,
        llm_request: LlmRequest {
            model: model.to_owned(),
            messages: Vec::new(),
            max_tokens: 1024,
            temperature: 0.7,
            system: Some(system_content),
        },
    }
}

// ── Bridge 4: LlmResponse → Message (LLMレスポンスをチャットメッセージに変換) ─

/// LlmResponse をチャット Message に変換したブリッジレコード。
pub struct LlmResponseToMessageBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// プロバイダハッシュ。
    pub provider_hash: u64,
    /// 出力トークン数。
    pub output_tokens: u32,
    /// キャッシュ TTL (秒)。高トークン数のレスポンスは長めにキャッシュ。
    pub ttl_secs: u32,
    /// 変換後の Message コンテンツ (MessageType::Text)。
    pub message_content: MessageType,
}

/// `LlmResponse` を `Message` コンテンツブリッジレコードに変換。
///
/// TTL はブランチレスで計算: 出力が512トークン以上なら1時間、未満なら30分。
#[inline]
#[must_use]
pub fn llm_response_to_message(resp: &LlmResponse) -> LlmResponseToMessageBridge {
    let provider_hash = fnv1a(resp.provider.as_bytes());
    let content_hash_base = fnv1a(resp.content.as_bytes());

    let mut buf = [0u8; 16];
    buf[..8].copy_from_slice(&provider_hash.to_le_bytes());
    buf[8..12].copy_from_slice(&resp.output_tokens.to_le_bytes());
    buf[12..].copy_from_slice(&content_hash_base.to_le_bytes()[..4]);
    let content_hash = fnv1a(&buf);

    // 出力トークンが512以上なら1時間(3600)、未満なら30分(1800)
    let long_response = (resp.output_tokens >= 512) as u32;
    let ttl_secs = 1800 + long_response * 1800;

    LlmResponseToMessageBridge {
        content_hash,
        provider_hash,
        output_tokens: resp.output_tokens,
        ttl_secs,
        message_content: MessageType::Text(resp.content.clone()),
    }
}

// ── Bridge 5: Vec<Message> → LlmRequest (会話履歴からリクエスト生成) ───────

/// 会話履歴から LlmRequest を生成したブリッジレコード。
pub struct HistoryToLlmRequestBridge {
    /// content_hash (FNV-1a)。
    pub content_hash: u64,
    /// メッセージ数。
    pub message_count: u32,
    /// 総コンテンツサイズ (bytes)。
    pub total_content_size: u32,
    /// 変換後の LlmRequest。
    pub llm_request: LlmRequest,
}

/// `Vec<Message>` 会話履歴を `LlmRequest` ブリッジレコードに変換。
#[inline]
#[must_use]
pub fn chat_history_to_llm_request(
    messages: &[Message],
    model: &str,
    system: Option<&str>,
) -> HistoryToLlmRequestBridge {
    let message_count = messages.len() as u32;
    let mut total_content_size: u32 = 0;
    let mut hash_acc: u64 = 0xcbf2_9ce4_8422_2325;

    let llm_messages: Vec<LlmMessage> = messages
        .iter()
        .map(|m| {
            let b = chat_message_to_llm_message(m);
            total_content_size = total_content_size.saturating_add(b.content_size);
            hash_acc ^= b.content_hash;
            hash_acc = hash_acc.wrapping_mul(0x0100_0000_01b3);
            b.llm_message
        })
        .collect();

    let mut buf = [0u8; 12];
    buf[..8].copy_from_slice(&hash_acc.to_le_bytes());
    buf[8..12].copy_from_slice(&total_content_size.to_le_bytes());
    let content_hash = fnv1a(&buf);

    HistoryToLlmRequestBridge {
        content_hash,
        message_count,
        total_content_size,
        llm_request: LlmRequest {
            model: model.to_owned(),
            messages: llm_messages,
            max_tokens: 1024,
            temperature: 0.7,
            system: system.map(str::to_owned),
        },
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_text_message(id: u64, sender_id: u64, text: &str) -> Message {
        Message {
            id,
            sender_id,
            room_id: 1,
            content: MessageType::Text(text.to_owned()),
            timestamp_ms: 1_000_000,
            reply_to: None,
            read_by: HashSet::new(),
            edited: false,
            deleted: false,
            metadata: None,
        }
    }

    fn make_user(id: u64, username: &str, role: UserRole) -> User {
        User {
            id,
            username: username.to_owned(),
            display_name: username.to_owned(),
            email: None,
            role,
            avatar_url: None,
            created_at_ms: 0,
            active: true,
        }
    }

    fn make_channel(id: u64, name: &str, ct: ChannelType) -> Channel {
        let mut members = HashSet::new();
        members.insert(1u64);
        Channel {
            id,
            space_id: 1,
            name: name.to_owned(),
            channel_type: ct,
            members,
            created_by: 1,
            created_at_ms: 0,
            invitations: HashSet::new(),
            description: String::new(),
            max_members: 0,
        }
    }

    // Bridge 1 tests

    #[test]
    fn test_message_to_llm_text() {
        let msg = make_text_message(1, 42, "こんにちは");
        let b = chat_message_to_llm_message(&msg);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.llm_message.role, "user");
        assert_eq!(b.llm_message.content, "こんにちは");
        assert_eq!(b.content_size, "こんにちは".len() as u32);
    }

    #[test]
    fn test_message_to_llm_image() {
        let msg = Message {
            id: 2,
            sender_id: 1,
            room_id: 1,
            content: MessageType::Image {
                url: "https://example.com/img.png".to_owned(),
                width: 800,
                height: 600,
            },
            timestamp_ms: 0,
            reply_to: None,
            read_by: HashSet::new(),
            edited: false,
            deleted: false,
            metadata: None,
        };
        let b = chat_message_to_llm_message(&msg);
        assert!(b.llm_message.content.contains("[image:"));
    }

    #[test]
    fn test_message_to_llm_hash_deterministic() {
        let msg = make_text_message(10, 20, "テスト");
        let b1 = chat_message_to_llm_message(&msg);
        let b2 = chat_message_to_llm_message(&msg);
        assert_eq!(b1.content_hash, b2.content_hash);
    }

    // Bridge 2 tests

    #[test]
    fn test_user_to_llm_admin() {
        let user = make_user(1, "admin_user", UserRole::Admin);
        let b = chat_user_to_llm_message(&user);
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.role_id, 0);
        assert_eq!(b.is_active, 1);
        assert_eq!(b.llm_message.role, "system");
        assert!(b.llm_message.content.contains("Admin"));
    }

    #[test]
    fn test_user_to_llm_bot() {
        let user = make_user(2, "bot_alice", UserRole::Bot);
        let b = chat_user_to_llm_message(&user);
        assert_eq!(b.role_id, 3);
        assert!(b.llm_message.content.contains("Bot"));
    }

    // Bridge 3 tests

    #[test]
    fn test_channel_to_llm_request_direct() {
        let ch = make_channel(1, "dm-channel", ChannelType::Direct);
        let b = chat_channel_to_llm_request(&ch, "gpt-4");
        assert_ne!(b.content_hash, 0);
        assert_eq!(b.channel_type_id, 0);
        assert_eq!(b.llm_request.model, "gpt-4");
        assert!(b.llm_request.system.is_some());
        assert!(b.llm_request.system.as_deref().unwrap().contains("Direct"));
    }

    #[test]
    fn test_channel_to_llm_request_public() {
        let ch = make_channel(2, "general", ChannelType::Public);
        let b = chat_channel_to_llm_request(&ch, "claude-3");
        assert_eq!(b.channel_type_id, 2);
        assert!(b.llm_request.system.as_deref().unwrap().contains("Public"));
    }

    // Bridge 4 tests

    #[test]
    fn test_llm_response_to_message_ttl_short() {
        let resp = LlmResponse {
            content: "短い返答".to_owned(),
            model: "gpt-4".to_owned(),
            input_tokens: 5,
            output_tokens: 100,
            provider: "openai".to_owned(),
            finish_reason: None,
        };
        let b = llm_response_to_message(&resp);
        assert_eq!(b.ttl_secs, 1800);
        assert!(matches!(b.message_content, MessageType::Text(_)));
    }

    #[test]
    fn test_llm_response_to_message_ttl_long() {
        let resp = LlmResponse {
            content: "長い返答".to_owned(),
            model: "gpt-4".to_owned(),
            input_tokens: 100,
            output_tokens: 600,
            provider: "openai".to_owned(),
            finish_reason: None,
        };
        let b = llm_response_to_message(&resp);
        assert_eq!(b.ttl_secs, 3600);
    }

    // Bridge 5 tests

    #[test]
    fn test_history_to_llm_request_empty() {
        let b = chat_history_to_llm_request(&[], "test-model", None);
        assert_eq!(b.message_count, 0);
        assert_eq!(b.total_content_size, 0);
        assert_eq!(b.llm_request.messages.len(), 0);
        assert!(b.llm_request.system.is_none());
    }

    #[test]
    fn test_history_to_llm_request_with_messages() {
        let msgs = vec![
            make_text_message(1, 1, "質問"),
            make_text_message(2, 2, "回答"),
        ];
        let b = chat_history_to_llm_request(&msgs, "claude-3", Some("システム指示"));
        assert_eq!(b.message_count, 2);
        assert_eq!(b.llm_request.messages.len(), 2);
        assert_eq!(b.llm_request.system.as_deref(), Some("システム指示"));
        assert_ne!(b.content_hash, 0);
    }
}
