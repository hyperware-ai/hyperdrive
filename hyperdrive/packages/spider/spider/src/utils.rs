use hyperware_process_lib::{
    our,
    vfs::{create_drive, open_dir, open_file},
};

use crate::types::{Conversation, Tool, TransportConfig};

/// Check if an API key is an OAuth token by examining the third field
/// OAuth tokens have "oat" followed by 2 digits in the third field (e.g., sk-ant-oat01-...)
/// API keys have "api" followed by 2 digits in the third field (e.g., sk-ant-api03-...)
pub(crate) fn is_oauth_token(key: &str) -> bool {
    // Split the key by '-' and check the third field
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() >= 3 {
        let third_field = parts[2];
        // Check if it starts with "oat" followed by 2 digits
        if third_field.len() >= 5 && third_field.starts_with("oat") {
            let suffix = &third_field[3..5];
            return suffix.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

pub(crate) fn encrypt_key(key: &str) -> String {
    // For actual encryption, use base64 encoding with a marker
    // In production, this should use proper encryption with a key derivation function
    use base64::{engine::general_purpose, Engine as _};
    format!(
        "encrypted:{}",
        general_purpose::STANDARD.encode(key.as_bytes())
    )
}

pub(crate) fn decrypt_key(encrypted_key: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    if encrypted_key.starts_with("encrypted:") {
        let encoded = &encrypted_key[10..];
        String::from_utf8(
            general_purpose::STANDARD
                .decode(encoded)
                .unwrap_or_default(),
        )
        .unwrap_or_default()
    } else {
        encrypted_key.to_string()
    }
}

pub(crate) fn preview_key(encrypted_key: &str) -> String {
    if encrypted_key.len() > 20 {
        format!("{}...", &encrypted_key[..20])
    } else {
        "***".to_string()
    }
}

pub(crate) async fn save_conversation_to_vfs(conversation: &Conversation) -> Result<(), String> {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("{}-{}.json", timestamp, conversation.id);

    // Try to create the conversations directory if it doesn't exist
    let dir_path = match create_drive(our().package_id(), "conversations", None) {
        Ok(drive_path) => drive_path,
        Err(e) => {
            println!("Warning: Failed to create conversations drive: {:?}", e);
            // Continue anyway - we'll keep conversations in memory
            return Ok(());
        }
    };
    let file_path = format!("{dir_path}/{filename}");

    // Serialize the conversation
    let json_content = serde_json::to_string_pretty(conversation)
        .map_err(|e| format!("Failed to serialize conversation: {}", e))?;

    // Write to file
    match open_file(&file_path, true, None) {
        Ok(file) => {
            file.write(json_content.as_bytes())
                .map_err(|e| format!("Failed to write conversation: {:?}", e))?;
            println!(
                "Conversation {} saved to VFS at {}",
                conversation.id, file_path
            );
        }
        Err(e) => {
            println!("Warning: Failed to save conversation to VFS: {:?}", e);
            // Continue - conversation is still in memory
        }
    }

    Ok(())
}

pub(crate) async fn load_conversation_from_vfs(
    conversation_id: &str,
) -> Result<Conversation, String> {
    let dir_path = format!("{}/conversations", our().node);

    // Open the conversations directory
    let dir = open_dir(&dir_path, false, None)
        .map_err(|e| format!("Failed to open conversations directory: {:?}", e))?;

    // List all files in the directory
    let entries = dir
        .read()
        .map_err(|e| format!("Failed to read directory: {:?}", e))?;

    // Look for a file containing the conversation ID
    for entry in entries {
        if entry.path.contains(conversation_id) {
            let file_path = format!("{}/{}", dir_path, entry.path);
            let file = open_file(&file_path, false, None)
                .map_err(|e| format!("Failed to open conversation file: {:?}", e))?;

            let content = file
                .read()
                .map_err(|e| format!("Failed to read conversation file: {:?}", e))?;

            let conversation: Conversation = serde_json::from_slice(&content)
                .map_err(|e| format!("Failed to parse conversation: {}", e))?;

            return Ok(conversation);
        }
    }

    Err(format!("Conversation {} not found in VFS", conversation_id))
}

pub(crate) async fn discover_mcp_tools(transport: &TransportConfig) -> Result<Vec<Tool>, String> {
    // MCP tool discovery implementation
    match transport.transport_type.as_str() {
        "stdio" => {
            // In WASM environment, we can't spawn processes
            // Return example tools for demonstration
            // In production, this would use a proxy service or HTTP transport
            println!("Note: stdio transport not fully supported in WASM environment");
            println!("Returning example tools for MCP server");

            Ok(vec![
                Tool {
                    name: "search".to_string(),
                    description: "Search for information".to_string(),
                    parameters: r#"{"type":"object","properties":{"query":{"type":"string","description":"The search query"}},"required":["query"]}"#.to_string(),
                    input_schema_json: None,
                },
                Tool {
                    name: "calculate".to_string(),
                    description: "Perform mathematical calculations".to_string(),
                    parameters: r#"{"type":"object","properties":{"expression":{"type":"string","description":"The mathematical expression to evaluate"}},"required":["expression"]}"#.to_string(),
                    input_schema_json: None,
                },
            ])
        }
        "http" => {
            // For HTTP transport, we would make HTTP requests to discover tools
            // This is a placeholder implementation
            Ok(vec![Tool {
                name: "http_tool".to_string(),
                description: "An HTTP-based MCP tool".to_string(),
                parameters: r#"{"type":"object","properties":{"query":{"type":"string"}}}"#
                    .to_string(),
                input_schema_json: None,
            }])
        }
        _ => Err(format!(
            "Unsupported transport type: {}",
            transport.transport_type
        )),
    }
}
