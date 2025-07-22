use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::{json, Value};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, args: String) -> String {
    // Parse arguments: <chain-id> <node-id-or-rpc-url> [trusted]
    let parts: Vec<&str> = args.trim().split_whitespace().collect();

    if parts.len() < 2 {
        return "Usage: add-provider <chain-id> <node-id-or-rpc-url> [trusted]\n  Examples:\n    add-provider 1 my-node.hypr\n    add-provider 1 wss://mainnet.infura.io/v3/your-key\n    add-provider 1 my-node.hypr false".to_string();
    }

    let chain_id: u64 = match parts[0].parse() {
        Ok(id) => id,
        Err(_) => return format!("Invalid chain ID: {}", parts[0]),
    };

    let provider_str = parts[1];
    let trusted = parts.get(2).map(|s| s.parse::<bool>().unwrap_or(true)).unwrap_or(true);

    // Determine if this is a node or RPC URL
    let provider_config = if provider_str.starts_with("ws://") || provider_str.starts_with("wss://") {
        // This is an RPC URL
        json!({
            "chain_id": chain_id,
            "provider": {
                "RpcUrl": {
                    "url": provider_str,
                    "auth": null
                }
            },
            "trusted": trusted
        })
    } else {
        // This is a node ID
        json!({
            "chain_id": chain_id,
            "provider": {
                "Node": {
                    "hns_update": {
                        "name": provider_str,
                        "public_key": "",
                        "ips": [],
                        "ports": {},
                        "routers": []
                    },
                    "use_as_provider": true
                }
            },
            "trusted": trusted
        })
    };

    // Create AddProvider request
    let request_body = json!({
        "AddProvider": provider_config
    });

    let Ok(Ok(Message::Response { body, .. })) = Request::to(("our", "eth", "distro", "sys"))
        .body(serde_json::to_vec(&request_body).unwrap())
        .send_and_await_response(60)
    else {
        return "Failed to communicate with eth module".to_string();
    };

    // Parse the response
    match serde_json::from_slice::<Value>(&body) {
        Ok(json_value) => {
            if let Some(response) = json_value.as_str() {
                match response {
                    "Ok" => format!("Successfully added provider: {} on chain {}", provider_str, chain_id),
                    "PermissionDenied" => "Permission denied: insufficient privileges".to_string(),
                    other => format!("Error: {}", other),
                }
            } else {
                format!("Unexpected response format: {}", json_value)
            }
        }
        Err(e) => {
            format!("Failed to parse response: {}\nRaw: {}", e, String::from_utf8_lossy(&body))
        }
    }
}