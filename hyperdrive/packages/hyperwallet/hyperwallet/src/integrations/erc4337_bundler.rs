//! Bundler client for submitting UserOperations
//! Supports multiple bundler backends (Pimlico, Candide)

use serde_json::{json, Value, Map};
use hyperware_process_lib::logging::{info, error};
use hyperware_process_lib::http::client::send_request_await_response;
use hyperware_process_lib::http::Method;
use hyperware_process_lib::hyperwallet_client::types::OperationError;
use std::collections::HashMap;
use url::Url;

// Bundler configuration
const PIMLICO_API_KEY: &str = "pim_JV4vJ4B1zmf1vBvbdgsLXi";
const PIMLICO_BASE_URL: &str = "https://api.pimlico.io/v2/8453/rpc";
const CANDIDE_BASE_URL: &str = "https://api.candide.dev/public/v3/8453";
const CIRCLE_PAYMASTER_ADDRESS: &str = "0x0578cFB241215b77442a541325d6A4E6dFE700Ec";

/// Bundler backend type
#[derive(Debug, Clone, Copy)]
enum BundlerBackend {
    Pimlico,
    Candide,
}

impl BundlerBackend {
    fn base_url(&self) -> &'static str {
        match self {
            Self::Pimlico => PIMLICO_BASE_URL,
            Self::Candide => CANDIDE_BASE_URL,
        }
    }
    
    fn name(&self) -> &'static str {
        match self {
            Self::Pimlico => "Pimlico",
            Self::Candide => "Candide",
        }
    }
}

/// Determine which bundler to use based on the UserOperation
fn select_bundler(_user_op: &Value) -> BundlerBackend {
    // Always use Candide as the default bundler
    BundlerBackend::Candide
}

/// Format a UserOperation for the target bundler
fn format_user_operation(user_op: &Value, backend: BundlerBackend) -> Value {
    match backend {
        BundlerBackend::Candide => {
            // Candide expects the format as-is for v0.8 with separate paymaster fields
            user_op.clone()
        }
        BundlerBackend::Pimlico => {
            // Pimlico might need some format adjustments
            // Note: This is kept for potential future use but won't be called with Candide as default
            format_for_pimlico(user_op)
        }
    }
}

/// Submit a UserOperation to the appropriate bundler
pub fn submit_user_operation(
    user_op: Value,
    entry_point: String,
) -> Result<String, OperationError> {
    let backend = select_bundler(&user_op);
    info!("Submitting UserOperation to {} bundler (default)", backend.name());
    
    // Format the UserOperation for the selected backend
    let formatted_user_op = format_user_operation(&user_op, backend);
    
    // Build the URL
    let url = match backend {
        BundlerBackend::Pimlico => {
            let url_str = format!("{}?apikey={}", backend.base_url(), PIMLICO_API_KEY);
            Url::parse(&url_str)
        }
        BundlerBackend::Candide => {
            Url::parse(backend.base_url())
        }
    }.map_err(|e| OperationError::internal_error(&format!("Invalid URL: {}", e)))?;
    
    // Create JSON-RPC request
    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "eth_sendUserOperation",
        "params": [formatted_user_op, entry_point],
        "id": 1
    });
    
    info!("{} request: {}", backend.name(), serde_json::to_string_pretty(&request_body).unwrap_or_default());
    
    // Send the request
    let response_data = send_json_rpc_request(url, request_body)?;
    
    // Extract user operation hash
    match response_data.get("result").and_then(|r| r.as_str()) {
        Some(hash) => {
            info!("UserOperation submitted successfully: {}", hash);
            Ok(hash.to_string())
        }
        None => {
            error!("{} response missing user operation hash", backend.name());
            Err(OperationError::internal_error("Invalid response from bundler"))
        }
    }
}

/// Helper function to send JSON-RPC requests
fn send_json_rpc_request(url: Url, request_body: Value) -> Result<Value, OperationError> {
    let body_bytes = serde_json::to_vec(&request_body)
        .map_err(|e| OperationError::internal_error(&format!("Failed to serialize request: {}", e)))?;
    
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    
    // Send request
    let response = send_request_await_response(
        Method::POST,
        url.clone(),
        Some(headers),
        30000, // 30 second timeout
        body_bytes,
    ).map_err(|e| {
        error!("Failed to send request to {}: {}", url, e);
        OperationError::internal_error(&format!("Bundler request failed: {}", e))
    })?;
    
    // Parse response
    let response_data: Value = serde_json::from_slice(&response.body())
        .map_err(|e| {
            error!("Failed to parse response: {}", e);
            error!("Raw response: {:?}", String::from_utf8_lossy(&response.body()));
            OperationError::internal_error(&format!("Invalid bundler response: {}", e))
        })?;
    
    // Check for JSON-RPC error
    if let Some(error) = response_data.get("error") {
        let error_msg = error.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        let error_code = error.get("code")
            .and_then(|c| c.as_i64())
            .unwrap_or(0);
        error!("JSON-RPC error {}: {}", error_code, error_msg);
        
        return Err(OperationError::invalid_params(error_msg));
    }
    
    Ok(response_data)
}



/// Format UserOperation specifically for Pimlico
fn format_for_pimlico(user_op: &Value) -> Value {
    if let Some(op) = user_op.as_object() {
        let mut formatted = Map::new();
        
        // Copy basic fields
        copy_field(&mut formatted, op, "sender");
        copy_field(&mut formatted, op, "nonce");
        
        // Handle optional initCode
        if let Some(init_code) = op.get("initCode").or_else(|| op.get("init_code")) {
            if init_code.as_str() != Some("0x") && init_code.as_str() != Some("") {
                formatted.insert("initCode".to_string(), init_code.clone());
            }
        }
        
        copy_field(&mut formatted, op, "callData");
        copy_field(&mut formatted, op, "callGasLimit");
        copy_field(&mut formatted, op, "verificationGasLimit");
        copy_field(&mut formatted, op, "preVerificationGas");
        copy_field(&mut formatted, op, "maxFeePerGas");
        copy_field(&mut formatted, op, "maxPriorityFeePerGas");
        copy_field(&mut formatted, op, "signature");
        
        // Handle paymaster fields if present
        if let Some(paymaster) = op.get("paymaster") {
            formatted.insert("paymaster".to_string(), paymaster.clone());
            copy_field(&mut formatted, op, "paymasterVerificationGasLimit");
            copy_field(&mut formatted, op, "paymasterPostOpGasLimit");
            copy_field(&mut formatted, op, "paymasterData");
        }
        
        json!(formatted)
    } else {
        user_op.clone()
    }
}

/// Helper to copy a field from source to destination
fn copy_field(dest: &mut Map<String, Value>, source: &Map<String, Value>, field: &str) {
    if let Some(value) = source.get(field) {
        dest.insert(field.to_string(), value.clone());
    }
}


/// Estimate gas for a UserOperation
pub fn estimate_user_operation_gas(
    user_op: Value,
    entry_point: String,
) -> Result<Value, OperationError> {
    let backend = select_bundler(&user_op);
    info!("Estimating gas via {} bundler", backend.name());
    
    // Format the UserOperation for the selected backend
    let formatted_user_op = format_user_operation(&user_op, backend);
    
    // Build the URL
    let url = match backend {
        BundlerBackend::Pimlico => {
            let url_str = format!("{}?apikey={}", backend.base_url(), PIMLICO_API_KEY);
            Url::parse(&url_str)
        }
        BundlerBackend::Candide => {
            Url::parse(backend.base_url())
        }
    }.map_err(|e| OperationError::internal_error(&format!("Invalid URL: {}", e)))?;
    
    // Create JSON-RPC request
    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "eth_estimateUserOperationGas",
        "params": [formatted_user_op, entry_point],
        "id": 1
    });
    
    // Send the request
    let response_data = send_json_rpc_request(url, request_body)?;
    
    // Extract gas estimates
    match response_data.get("result") {
        Some(estimates) => {
            info!("Gas estimation successful");
            Ok(estimates.clone())
        }
        None => {
            error!("Gas estimation response missing result");
            Err(OperationError::internal_error("Invalid gas estimation response"))
        }
    }
}

/// Get receipt for a UserOperation with retry logic
pub fn get_user_operation_receipt(
    user_op_hash: String,
) -> Result<Value, OperationError> {
    // TODO: Track which bundler was used for submission
    let backend = BundlerBackend::Candide;
    info!("Getting UserOperation receipt from {}", backend.name());
    
    // Retry with exponential backoff: 1s, 2s, 4s, 8s
    let retry_delays = [2, 3, 3];
    
    for (attempt, &delay) in retry_delays.iter().enumerate() {
        let url = match backend {
            BundlerBackend::Pimlico => {
                let url_str = format!("{}?apikey={}", backend.base_url(), PIMLICO_API_KEY);
                Url::parse(&url_str)
            }
            BundlerBackend::Candide => {
                Url::parse(backend.base_url())
            }
        }.map_err(|e| OperationError::internal_error(&format!("Invalid URL: {}", e)))?;
        
        // Create JSON-RPC request
        let request_body = json!({
            "jsonrpc": "2.0",
            "method": "eth_getUserOperationReceipt",
            "params": [user_op_hash.clone()],
            "id": 1
        });
        
        // Send the request
        let response_data = match send_json_rpc_request(url, request_body) {
            Ok(data) => data,
            Err(e) => {
                if attempt < retry_delays.len() - 1 {
                    info!("Network error getting receipt (attempt {}): {:?}, retrying in {}s...", attempt + 1, e, delay);
                    std::thread::sleep(std::time::Duration::from_secs(delay as u64));
                    continue;
                } else {
                    return Err(e);
                }
            }
        };
        
        info!("Raw bundler response: {}", serde_json::to_string_pretty(&response_data).unwrap_or_else(|_| format!("{:?}", response_data)));
        
        // Extract receipt
        match response_data.get("result") {
            Some(receipt) => {
                if receipt.is_null() {
                    // Receipt not ready yet
                    if attempt < retry_delays.len() - 1 {
                        info!("UserOp receipt not ready yet (attempt {}), retrying in {}s...", attempt + 1, delay);
                        std::thread::sleep(std::time::Duration::from_secs(delay as u64));
                        continue;
                    } else {
                        info!("UserOperation receipt still not available after all retries");
                        return Ok(json!(null));
                    }
                } else {
                    // Receipt is available
                    info!("UserOperation receipt retrieved");
                    return Ok(receipt.clone());
                }
            }
            None => {
                if attempt < retry_delays.len() - 1 {
                    info!("No result in bundler response (attempt {}), retrying in {}s...", attempt + 1, delay);
                    std::thread::sleep(std::time::Duration::from_secs(delay as u64));
                    continue;
                } else {
                    info!("UserOperation not yet mined after all retries");
                    return Ok(json!(null));
                }
            }
        }
    }
    
    // This should never be reached due to the returns above, but just in case
    info!("UserOperation receipt still not available after all retries");
    Ok(json!(null))
}
