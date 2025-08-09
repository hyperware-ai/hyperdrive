// add-node-provider/lib.rs
use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::{json, Value};
use std::collections::HashMap;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, args: String) -> String {
    // Parse arguments: <chain-id> <node-name> <public-key> <ip-address> <ws-port> [--trusted <true|false>]
    let parts: Vec<&str> = args.trim().split_whitespace().collect();

    if parts.len() < 5 {
        return "Usage: add-node-provider <chain-id> <node-name> <public-key> <ip-address> <ws-port> [--trusted <true|false>]\n  Examples:\n    add-node-provider 8453 other-node.hypr abc123pubkey 192.168.1.1 9000 (defaults to trusted=false)\n    add-node-provider 1 other-node.hypr abc123pubkey 192.168.1.1 9000 --trusted true".to_string();
    }

    let chain_id = match parts[0].parse::<u64>() {
        Ok(id) => id,
        Err(_) => return format!("Invalid chain ID: {}. Must be a number.", parts[0]),
    };

    let node_name = parts[1];
    let public_key = parts[2];
    let ip_address = parts[3];
    let ws_port = match parts[4].parse::<u16>() {
        Ok(port) => port,
        Err(_) => return format!("Invalid WebSocket port: {}. Must be a number.", parts[4]),
    };

    // Parse trusted flag (default to false for node providers)
    let trusted = parse_flag_bool(&parts[5..], "--trusted", false);

    // Create ports map with WebSocket port
    let mut ports = HashMap::new();
    ports.insert("ws".to_string(), ws_port);

    // Create the HNS update object
    let hns_update = json!({
        "name": node_name,
        "public_key": public_key,
        "ips": [ip_address],
        "ports": ports,
        "routers": []
    });

    // Create the provider configuration
    let provider_config = json!({
        "chain_id": chain_id,
        "provider": {
            "Node": {
                "hns_update": hns_update,
                "use_as_provider": true
            }
        },
        "trusted": trusted
    });

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
    if let Ok(json_value) = serde_json::from_slice::<Value>(&body) {
        if let Some(response) = json_value.as_str() {
            match response {
                "Ok" => {
                    format!(
                        "Successfully added node provider: {} ({}:{}) on chain {} with trusted={}",
                        node_name, ip_address, ws_port, chain_id, trusted
                    )
                }
                "PermissionDenied" => "Permission denied: insufficient privileges".to_string(),
                other => format!("Error: {}", other),
            }
        } else {
            // Handle any other response types with better formatting
            format!(
                "Unexpected response: {}",
                serde_json::to_string_pretty(&json_value)
                    .unwrap_or_else(|_| "Failed to format response".to_string())
            )
        }
    } else {
        format!(
            "Failed to parse response as JSON\nRaw response: {}",
            String::from_utf8_lossy(&body)
        )
    }
}
fn parse_flag_bool(args: &[&str], flag: &str, default: bool) -> bool {
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            if i + 1 < args.len() {
                if let Ok(value) = args[i + 1].parse::<bool>() {
                    return value;
                }
            }
            break;
        }
        i += 1;
    }
    default
}
