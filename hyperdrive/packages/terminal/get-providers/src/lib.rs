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

    // Parse as generic JSON and pretty-print
    match serde_json::from_slice::<Value>(&body) {
        Ok(json_value) => {
            serde_json::to_string_pretty(&json_value)
                .unwrap_or_else(|_| "Failed to format JSON".to_string())
        }
        Err(e) => {
            format!("Failed to parse response as JSON: {}\nRaw response: {}", e, String::from_utf8_lossy(&body))
        }
    }
}
