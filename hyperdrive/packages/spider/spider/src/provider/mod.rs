use std::future::Future;
use std::pin::Pin;

use crate::types::{Message, Tool};

mod anthropic;
use anthropic::AnthropicProvider;

pub(crate) trait LlmProvider {
    fn complete<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [Tool],
        model: Option<&'a str>,
        max_tokens: u32,
        temperature: f32,
    ) -> Pin<Box<dyn Future<Output = Result<Message, String>> + 'a>>;
    fn name(&self) -> &str;
}

// Placeholder for future providers
struct OpenAIProvider {
    api_key: String,
}

impl OpenAIProvider {
    fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl LlmProvider for OpenAIProvider {
    fn complete<'a>(
        &'a self,
        _messages: &'a [Message],
        _tools: &'a [Tool],
        _model: Option<&'a str>,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Pin<Box<dyn Future<Output = Result<Message, String>> + 'a>> {
        Box::pin(async move { Err("OpenAI provider not yet implemented".to_string()) })
    }

    fn name(&self) -> &str {
        "openai"
    }
}

pub(crate) fn create_llm_provider(provider_type: &str, api_key: &str) -> Box<dyn LlmProvider> {
    use crate::utils::is_oauth_token;

    match provider_type {
        "anthropic" => {
            // Check if this is an OAuth token by examining the third field
            let is_oauth = is_oauth_token(api_key);
            Box::new(AnthropicProvider::new(api_key.to_string(), is_oauth))
        }
        "openai" => Box::new(OpenAIProvider::new(api_key.to_string())),
        _ => {
            // Default to Anthropic
            let is_oauth = is_oauth_token(api_key);
            Box::new(AnthropicProvider::new(api_key.to_string(), is_oauth))
        }
    }
}
