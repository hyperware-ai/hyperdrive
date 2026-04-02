use hyperware_process_lib::{println, script, Address, Request};
use serde_json::Value;
use std::collections::HashMap;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
});

const USAGE: &str = r#"\x1b[1mUsage:\x1b[0m send-bulk-chat '{"node1": "message1", "node2": "message2", ...}'

Send messages to multiple nodes at once. Creates chats if they don't exist.

Example:
  send-bulk-chat '{"alice:hyper": "Hello Alice!", "bob:hyper": "Hey Bob!"}'
"#;

const CHAT_PROCESS_ID: (&str, &str, &str) = ("chat", "homepage", "sys");

script!(init);
fn init(our: Address, args: String) -> String {
    if args.is_empty() {
        return USAGE.to_string();
    }

    // Parse the JSON argument
    println!("{args}");
    let args_slice = if args.starts_with('\'') && args.ends_with('\'') && args.len() >= 2 {
        &args[1..args.len() - 1]
    } else {
        &args
    };
    let messages: HashMap<String, String> = match serde_json::from_str(args_slice) {
        Ok(j) => j,
        Err(e) => return format!("Error parsing JSON: {}\n\n{}", e, USAGE),
    };

    if messages.is_empty() {
        return "Error: No messages to send".to_string();
    }

    let mut results = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    // Process each node-message pair
    for (node, message) in messages {
        // Create typed request for creating/getting chat
        let create_chat_request = serde_json::json!({
            "CreateChat": {
                "counterparty": node.clone()
            }
        });

        // Send create chat request to our own node's chat:chat:hyper
        let chat_address = Address::new(our.node(), CHAT_PROCESS_ID);

        match Request::to(&chat_address)
            .body(serde_json::to_vec(&create_chat_request).unwrap_or_default())
            .send_and_await_response(5)
        {
            Ok(Ok(response_msg)) => {
                // Parse the response to get the actual chat object with normalized ID
                let response: Value = match serde_json::from_slice(response_msg.body()) {
                    Ok(v) => v,
                    Err(e) => {
                        results.push(format!("✗ {}: Failed to parse chat response - {}", node, e));
                        error_count += 1;
                        continue;
                    }
                };

                // Extract the chat ID from the Ok response
                let chat_id = match response
                    .get("Ok")
                    .and_then(|ok| ok.get("id"))
                    .and_then(|id| id.as_str())
                {
                    Some(id) => id.to_string(),
                    None => {
                        results.push(format!("✗ {}: Invalid chat response format", node));
                        error_count += 1;
                        continue;
                    }
                };

                println!("Created/got chat with ID: {} for node: {}", chat_id, node);

                // Now send the message with typed request using the actual chat ID
                let send_msg_request = serde_json::json!({
                    "SendMessage": {
                        "chat_id": chat_id,
                        "content": message.clone(),
                        "reply_to": null,
                        "file_info": null
                    }
                });

                match Request::to(&chat_address)
                    .body(serde_json::to_vec(&send_msg_request).unwrap_or_default())
                    .send_and_await_response(5)
                {
                    Ok(Ok(_)) => {
                        results.push(format!("✓ {}: Message sent", node));
                        success_count += 1;
                    }
                    Ok(Err(e)) => {
                        results.push(format!("✗ {}: Failed to send message - {:?}", node, e));
                        error_count += 1;
                    }
                    Err(e) => {
                        results.push(format!("✗ {}: Failed to send message - {:?}", node, e));
                        error_count += 1;
                    }
                }
            }
            Ok(Err(e)) => {
                results.push(format!("✗ {}: Failed to create/get chat - {:?}", node, e));
                error_count += 1;
            }
            Err(e) => {
                results.push(format!("✗ {}: Failed to create/get chat - {:?}", node, e));
                error_count += 1;
            }
        }
    }

    // Format output
    let mut output = results.join("\n");
    output.push_str(&format!(
        "\n\n\x1b[1mSummary:\x1b[0m {} sent, {} failed",
        success_count, error_count
    ));

    output
}
