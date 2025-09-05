#[cfg(test)]
mod tests {
    use chrono::Utc;
    use spider::{ApiKey, Conversation, ConversationMetadata, Message, SpiderApiKey, SpiderState};

    #[test]
    fn test_api_key_encryption() {
        let state = SpiderState::default();
        let key = "test_key_123";
        let encrypted = state.encrypt_key(key);
        assert_ne!(encrypted, key);
        assert!(encrypted.starts_with("encrypted:"));
        let decrypted = state.decrypt_key(&encrypted);
        assert_eq!(decrypted, key);
    }

    #[test]
    fn test_api_key_preview() {
        let state = SpiderState::default();
        let encrypted = "encrypted:VGVzdEtleVZlcnlMb25nU3RyaW5nVGhhdE5lZWRzVG9CZVRydW5jYXRlZA==";
        let preview = state.preview_key(encrypted);
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 23); // 20 chars + "..."

        let short_key = "short";
        let preview_short = state.preview_key(short_key);
        assert_eq!(preview_short, "***");
    }

    #[test]
    fn test_spider_key_validation() {
        let mut state = SpiderState::default();
        let test_key = SpiderApiKey {
            key: "sp_test".to_string(),
            name: "Test Key".to_string(),
            permissions: vec!["chat".to_string()],
            created_at: Utc::now().timestamp() as u64,
        };

        state.spider_api_keys.push(test_key);

        assert!(state.validate_spider_key("sp_test"));
        assert!(!state.validate_spider_key("invalid"));
        assert!(!state.validate_spider_key("sp_other"));
    }

    #[test]
    fn test_spider_key_permissions() {
        let mut state = SpiderState::default();

        // Add a key with limited permissions
        let limited_key = SpiderApiKey {
            key: "sp_limited".to_string(),
            name: "Limited Key".to_string(),
            permissions: vec!["list".to_string()],
            created_at: Utc::now().timestamp() as u64,
        };

        // Add a key with full permissions
        let full_key = SpiderApiKey {
            key: "sp_full".to_string(),
            name: "Full Key".to_string(),
            permissions: vec!["chat".to_string(), "list".to_string(), "admin".to_string()],
            created_at: Utc::now().timestamp() as u64,
        };

        state.spider_api_keys.push(limited_key.clone());
        state.spider_api_keys.push(full_key.clone());

        // Test limited key doesn't have chat permission
        let limited = state
            .spider_api_keys
            .iter()
            .find(|k| k.key == "sp_limited")
            .unwrap();
        assert!(!limited.permissions.contains(&"chat".to_string()));
        assert!(limited.permissions.contains(&"list".to_string()));

        // Test full key has chat permission
        let full = state
            .spider_api_keys
            .iter()
            .find(|k| k.key == "sp_full")
            .unwrap();
        assert!(full.permissions.contains(&"chat".to_string()));
        assert!(full.permissions.contains(&"list".to_string()));
        assert!(full.permissions.contains(&"admin".to_string()));
    }

    #[test]
    fn test_conversation_metadata() {
        let metadata = ConversationMetadata {
            start_time: Utc::now().to_rfc3339(),
            client: "test-client".to_string(),
            from_stt: false,
        };

        assert_eq!(metadata.client, "test-client");
        assert!(!metadata.from_stt);
        assert!(!metadata.start_time.is_empty());
    }

    #[test]
    fn test_message_creation() {
        let user_message = Message {
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
            tool_calls_json: None,
            tool_results_json: None,
            timestamp: Utc::now().timestamp() as u64,
        };

        assert_eq!(user_message.role, "user");
        assert_eq!(user_message.content, "Hello, world!");
        assert!(user_message.tool_calls_json.is_none());
        assert!(user_message.tool_results_json.is_none());
        assert!(user_message.timestamp > 0);

        // Test message with tool calls
        let tool_call_json = r#"[{"id":"123","tool_name":"test_tool","parameters":"{}"}]"#;
        let assistant_message = Message {
            role: "assistant".to_string(),
            content: "Let me help you with that.".to_string(),
            tool_calls_json: Some(tool_call_json.to_string()),
            tool_results_json: None,
            timestamp: Utc::now().timestamp() as u64,
        };

        assert_eq!(assistant_message.role, "assistant");
        assert!(assistant_message.tool_calls_json.is_some());
        assert_eq!(assistant_message.tool_calls_json.unwrap(), tool_call_json);
    }

    #[test]
    fn test_conversation_creation() {
        let conversation = Conversation {
            id: "test-conv-123".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test message".to_string(),
                tool_calls_json: None,
                tool_results_json: None,
                timestamp: Utc::now().timestamp() as u64,
            }],
            metadata: ConversationMetadata {
                start_time: Utc::now().to_rfc3339(),
                client: "test".to_string(),
                from_stt: false,
            },
            llm_provider: "anthropic".to_string(),
            mcp_servers: vec![],
        };

        assert_eq!(conversation.id, "test-conv-123");
        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.llm_provider, "anthropic");
        assert!(conversation.mcp_servers.is_empty());
    }

    #[async_std::test]
    async fn test_provider_factory() {
        use spider::create_llm_provider;

        let anthropic_provider = create_llm_provider("anthropic", "test_key");
        assert_eq!(anthropic_provider.name(), "anthropic");

        let openai_provider = create_llm_provider("openai", "test_key");
        assert_eq!(openai_provider.name(), "openai");

        // Test default fallback
        let unknown_provider = create_llm_provider("unknown", "test_key");
        assert_eq!(unknown_provider.name(), "anthropic"); // Falls back to Anthropic
    }
}
