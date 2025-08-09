use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::{json, Value};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, args: String) -> String {
    // Parse arguments: <rpc-url> [--chain-id <id>] [--trusted <true|false>] [--auth-type <type> --auth-value <value>]
    let parts: Vec<&str> = args.trim().split_whitespace().collect();

    if parts.is_empty() {
        return "Usage: add-rpcurl-provider <rpc-url> [--chain-id <id>] [--trusted <true|false>] [--auth-type <basic|bearer|raw> --auth-value <value>]\n  Examples:\n    add-rpcurl-provider wss://base-mainnet.infura.io/v3/your-key (defaults to chain-id=8453, trusted=true)\n    add-rpcurl-provider wss://mainnet.infura.io/v3/your-key --chain-id 1\n    add-rpcurl-provider wss://base-mainnet.infura.io/ws/v3/your-key --trusted false\n    add-rpcurl-provider wss://base-mainnet.infura.io/ws/v3/your-key --auth-type bearer --auth-value your-token\n    add-rpcurl-provider wss://rpc.example.com --chain-id 1 --trusted true --auth-type basic --auth-value username:password".to_string();
    }

    let provider_str = parts[0];

    // Validate URL format
    if !provider_str.starts_with("http://")
        && !provider_str.starts_with("https://")
        && !provider_str.starts_with("wss://")
        && !provider_str.starts_with("ws://")
    {
        return "Error: URL must start with http://, https://, ws://, or wss://".to_string();
    }

    // Parse optional flags
    let chain_id = parse_flag_value(&parts[1..], "--chain-id", 8453);
    let trusted = parse_flag_bool(&parts[1..], "--trusted", true);
    let auth = parse_auth_options(&parts[1..]);

    match auth {
        Ok(auth_option) => {
            // Check if auth is configured before consuming the value
            let has_auth = auth_option.is_some();

            // This is an RPC URL
            let mut rpc_config = json!({
                "url": provider_str
            });

            // Only include auth field if authentication is provided
            if let Some(auth_value) = auth_option {
                rpc_config
                    .as_object_mut()
                    .unwrap()
                    .insert("auth".to_string(), auth_value);
            }

            let provider_config = json!({
                "chain_id": chain_id,
                "provider": {
                    "RpcUrl": rpc_config
                },
                "trusted": trusted
            });

            // Create AddProvider request
            let request_body = json!({
                "AddProvider": provider_config
            });

            let Ok(Ok(Message::Response { body, .. })) =
                Request::to(("our", "eth", "distro", "sys"))
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
                            let auth_info = if has_auth { " with authentication" } else { "" };
                            format!(
                                "Successfully added RPC URL provider: {} on chain {}{}",
                                provider_str, chain_id, auth_info
                            )
                        }
                        "PermissionDenied" => {
                            "Permission denied: insufficient privileges".to_string()
                        }
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
        Err(err_msg) => err_msg,
    }
}

fn parse_flag_value<T: std::str::FromStr>(args: &[&str], flag: &str, default: T) -> T {
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            if i + 1 < args.len() {
                if let Ok(value) = args[i + 1].parse::<T>() {
                    return value;
                }
            }
            break;
        }
        i += 1;
    }
    default
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

fn parse_auth_options(args: &[&str]) -> Result<Option<Value>, String> {
    let mut i = 0;
    let mut auth_type: Option<&str> = None;
    let mut auth_value: Option<&str> = None;

    while i < args.len() {
        match args[i] {
            "--auth-type" => {
                if i + 1 >= args.len() {
                    return Err("Missing value for --auth-type".to_string());
                }
                auth_type = Some(args[i + 1]);
                i += 2;
            }
            "--auth-value" => {
                if i + 1 >= args.len() {
                    return Err("Missing value for --auth-value".to_string());
                }
                auth_value = Some(args[i + 1]);
                i += 2;
            }
            "--chain-id" | "--trusted" => {
                // Skip these flags and their values as they're handled separately
                i += 2;
            }
            _ => {
                // Skip unknown arguments (the URL is handled separately)
                i += 1;
            }
        }
    }

    match (auth_type, auth_value) {
        (Some(auth_type), Some(auth_value)) => {
            let auth_json = match auth_type.to_lowercase().as_str() {
                "basic" => json!({"Basic": auth_value}),
                "bearer" => json!({"Bearer": auth_value}),
                "raw" => json!({"Raw": auth_value}),
                _ => {
                    return Err("Invalid auth type. Must be 'basic', 'bearer', or 'raw'".to_string())
                }
            };
            Ok(Some(auth_json))
        }
        (Some(_), None) => Err("--auth-type specified but --auth-value is missing".to_string()),
        (None, Some(_)) => Err("--auth-value specified but --auth-type is missing".to_string()),
        (None, None) => Ok(None),
    }
}
