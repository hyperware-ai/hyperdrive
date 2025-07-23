use hyperware_process_lib::{script, Address, Message, Request};
use serde_json::{json, Value};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

script!(init);
fn init(_our: Address, args: String) -> String {
    // Parse arguments: <chain-id> <node-id-or-rpc-url> [trusted] [--auth-type <type> --auth-value <value>]
    let parts: Vec<&str> = args.trim().split_whitespace().collect();

    if parts.len() < 2 {
        return "Usage: add-provider <chain-id> <node-id-or-rpc-url> [trusted] [--auth-type <basic|bearer|raw> --auth-value <value>]\n  Examples:\n    add-provider 1 my-node.hypr\n    add-provider 1 wss://mainnet.infura.io/v3/your-key\n    add-provider 1 my-node.hypr false\n    add-provider 1 wss://mainnet.infura.io/v3/your-key true --auth-type bearer --auth-value your-token\n    add-provider 1 wss://rpc.example.com true --auth-type basic --auth-value username:password".to_string();
    }

    let chain_id: u64 = match parts[0].parse() {
        Ok(id) => id,
        Err(_) => return format!("Invalid chain ID: {}", parts[0]),
    };

    let provider_str = parts[1];

    // Parse trusted flag (default to true)
    let mut trusted = true;
    let mut auth_start_idx = 2;

    if parts.len() > 2 {
        if let Ok(trusted_val) = parts[2].parse::<bool>() {
            trusted = trusted_val;
            auth_start_idx = 3;
        } else if parts[2].starts_with("--") {
            // If third argument starts with --, it's an auth flag, keep trusted as true
            auth_start_idx = 2;
        } else {
            return "Invalid trusted value. Must be 'true' or 'false'".to_string();
        }
    }

    // Parse authentication options
    let auth = parse_auth_options(&parts[auth_start_idx..]);

    match auth {
        Ok(auth_option) => {
            // Check if auth is configured before consuming the value
            let has_auth = auth_option.is_some();

            // Determine if this is a node or RPC URL
            let provider_config = if provider_str.starts_with("http://") || provider_str.starts_with("https://") || provider_str.starts_with("wss://") || provider_str.starts_with("ws://") {
                // This is an RPC URL
                let mut rpc_config = json!({
                    "url": provider_str
                });

                // Only include auth field if authentication is provided
                if let Some(auth_value) = auth_option {
                    rpc_config.as_object_mut().unwrap().insert("auth".to_string(), auth_value);
                }

                json!({
                    "chain_id": chain_id,
                    "provider": {
                        "RpcUrl": rpc_config
                    },
                    "trusted": trusted
                })
            } else {
                // This is a node ID - authentication not supported for nodes
                if has_auth {
                    return "Authentication is not supported for node providers, only for RPC URLs".to_string();
                }

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
                            "Ok" => {
                                let auth_info = if has_auth { " with authentication" } else { "" };
                                format!("Successfully added provider: {} on chain {}{}", provider_str, chain_id, auth_info)
                            },
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
        Err(err_msg) => err_msg,
    }
}

fn parse_auth_options(args: &[&str]) -> Result<Option<Value>, String> {
    if args.is_empty() {
        return Ok(None);
    }

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
            _ => {
                return Err(format!("Unknown argument: {}", args[i]));
            }
        }
    }

    match (auth_type, auth_value) {
        (Some(auth_type), Some(auth_value)) => {
            let auth_json = match auth_type.to_lowercase().as_str() {
                "basic" => json!({"Basic": auth_value}),
                "bearer" => json!({"Bearer": auth_value}),
                "raw" => json!({"Raw": auth_value}),
                _ => return Err("Invalid auth type. Must be 'basic', 'bearer', or 'raw'".to_string()),
            };
            Ok(Some(auth_json))
        }
        (Some(_), None) => Err("--auth-type specified but --auth-value is missing".to_string()),
        (None, Some(_)) => Err("--auth-value specified but --auth-type is missing".to_string()),
        (None, None) => Ok(None),
    }
}