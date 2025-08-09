use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::Value;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, args: String) -> String {
    if args.is_empty() {
        return "Usage: remove-provider <chain_id> <provider_name_or_url>".to_string();
    }

    let parts: Vec<&str> = args.trim().split_whitespace().collect();
    if parts.len() != 2 {
        return "Usage: remove-provider <chain_id> <provider_name_or_url>".to_string();
    }

    let Ok(chain_id) = parts[0].parse::<u64>() else {
        return format!("Invalid chain_id: '{}'. Must be a number.", parts[0]);
    };

    let provider_identifier = parts[1].to_string();

    // Manually construct the JSON for RemoveProvider
    let request_json = format!(
        r#"{{"RemoveProvider": [{}, "{}"]}}"#,
        chain_id, provider_identifier
    );

    let Ok(Ok(Message::Response { body, .. })) = Request::to(("our", "eth", "distro", "sys"))
        .body(request_json.as_bytes().to_vec())
        .send_and_await_response(60)
    else {
        return "Failed to remove provider from eth module".to_string();
    };

    // Parse the response and handle different variants
    if let Ok(json_value) = serde_json::from_slice::<Value>(&body) {
        match json_value.as_str() {
            Some("Ok") => {
                format!(
                    "Successfully removed provider '{}' from chain {}",
                    provider_identifier, chain_id
                )
            }
            Some("ProviderNotFound") => {
                format!(
                    "Provider '{}' not found on chain {} (may have already been removed)",
                    provider_identifier, chain_id
                )
            }
            Some("PermissionDenied") => {
                "Permission denied: you don't have root capability for eth module".to_string()
            }
            _ => {
                // Handle any other response types
                format!(
                    "Unexpected response: {}",
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_else(|_| "Failed to format response".to_string())
                )
            }
        }
    } else {
        format!(
            "Failed to parse response as JSON\nRaw response: {}",
            String::from_utf8_lossy(&body)
        )
    }
}
