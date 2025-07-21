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
    // The JSON structure for RemoveProvider((chain_id, provider_str)) would be:
    // {"RemoveProvider": [chain_id, "provider_str"]}
    let request_json = format!(
        r#"{{"RemoveProvider": [{}, "{}"]}}"#,
        chain_id,
        provider_identifier
    );

    let Ok(Ok(Message::Response { body, .. })) = Request::to(("our", "eth", "distro", "sys"))
        .body(request_json.as_bytes().to_vec())
        .send_and_await_response(60)
    else {
        return "Failed to remove provider from eth module".to_string();
    };

    // Parse the response and show result
    match serde_json::from_slice::<Value>(&body) {
        Ok(json_value) => {
            if json_value == "Ok" {
                format!("Successfully removed provider '{}' from chain {}", provider_identifier, chain_id)
            } else {
                format!("Response: {}", serde_json::to_string_pretty(&json_value)
                    .unwrap_or_else(|_| "Failed to format response".to_string()))
            }
        }
        Err(e) => {
            format!("Failed to parse response as JSON: {}\nRaw response: {}", e, String::from_utf8_lossy(&body))
        }
    }
}