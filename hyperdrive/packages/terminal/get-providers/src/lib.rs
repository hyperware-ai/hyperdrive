use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::Value;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, _args: String) -> String {
    // Manually construct the JSON for GetProviders since it's a simple enum variant
    let request_body = r#""GetProviders""#.as_bytes().to_vec();

    let Ok(Ok(Message::Response { body, .. })) = Request::to(("our", "eth", "distro", "sys"))
        .body(request_body)
        .send_and_await_response(60)
    else {
        return "Failed to get providers from eth module".to_string();
    };

    //// Parse as generic JSON and pretty-print
    //match serde_json::from_slice::<Value>(&body) {
    //    Ok(json_value) => {
    //        serde_json::to_string_pretty(&json_value)
    //            .unwrap_or_else(|_| "Failed to format JSON".to_string())
    //    }
    //    Err(e) => {
    //        format!("Failed to parse response as JSON: {}\nRaw response: {}", e, String::from_utf8_lossy(&body))
    //    }
    //}
    match serde_json::from_slice::<Value>(&body) {
        Ok(json_value) => {
            // Check if it looks like a Providers response
            if let Some(obj) = json_value.as_object() {
                if let Some(providers_value) = obj.get("Providers") {
                    if let Some(providers_array) = providers_value.as_array() {
                        // We have a Providers response with an array
                        if providers_array.is_empty() {
                            return "No providers configured.".to_string();
                        } else {
                            let mut output = format!("Found {} provider(s):\n", providers_array.len());
                            for (i, provider) in providers_array.iter().enumerate() {
                                output.push_str(&format!("\n{}. ", i + 1));

                                // Extract basic info
                                if let Some(chain_id) = provider.get("chain_id") {
                                    output.push_str(&format!("Chain ID: {}\n", chain_id));
                                }
                                if let Some(trusted) = provider.get("trusted") {
                                    output.push_str(&format!("   Trusted: {}\n", trusted));
                                }

                                // Handle provider type
                                if let Some(provider_info) = provider.get("provider") {
                                    if let Some(rpc_url) = provider_info.get("RpcUrl") {
                                        output.push_str("   Type: RPC URL\n");
                                        if let Some(url) = rpc_url.get("url") {
                                            output.push_str(&format!("   URL: {}\n", url));
                                        }
                                        if rpc_url.get("auth").is_some() {
                                            output.push_str("   Auth: Configured\n");
                                        }
                                    } else if let Some(node_info) = provider_info.get("Node") {
                                        output.push_str("   Type: Node Provider\n");
                                        if let Some(hns_update) = node_info.get("hns_update") {
                                            if let Some(name) = hns_update.get("name") {
                                                output.push_str(&format!("   Node: {}\n", name));
                                            }
                                        }
                                        if let Some(use_as_provider) = node_info.get("use_as_provider") {
                                            output.push_str(&format!("   Use as Provider: {}\n", use_as_provider));
                                        }
                                    }
                                }
                            }
                            return output;
                        }
                    }
                }
            }

            // If it's not a Providers response, show the JSON structure
            format!("Response (not Providers): {}", serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| "Could not format JSON".to_string()))
        }
        Err(e) => {
            format!("Failed to parse as JSON: {}\nRaw response: {}", e, String::from_utf8_lossy(&body))
        }
    }
}