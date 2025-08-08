//! Bundler client for submitting UserOperations to Candide
//!
//! This module handles all communication with the Candide bundler service
//! for ERC-4337 UserOperation submission and monitoring.

use hyperware_process_lib::http::client::send_request_await_response;
use hyperware_process_lib::http::Method;
use hyperware_process_lib::hyperwallet_client::types::OperationError;
use hyperware_process_lib::logging::{error, info};
use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;

/// Candide bundler configuration
const CANDIDE_BASE_URL: &str = "https://api.candide.dev/public/v3/8453";
const REQUEST_TIMEOUT_MS: u64 = 30000;

/// Retry delays for receipt polling (in seconds)
/// Optimized for typical confirmation time of 6-8 seconds
/// Strategy: Initial wait, then frequent polling around 6-8s mark, then progressive backoff
/// Timeline: 3s, 4s, 5s, 6s, 7s, 8s, 9s, 11s, 15s, 21s, 29s, 41s (total ~41s with 12 attempts)
const RECEIPT_RETRY_DELAYS_SECS: &[u64] = &[3, 1, 1, 1, 1, 1, 1, 2, 4, 6, 8, 12];

/// Result from getting a UserOperation receipt
#[derive(Debug)]
pub struct UserOperationReceipt {
    pub user_op_hash: String,
    pub transaction_hash: String,
    pub success: bool,
    pub actual_gas_used: Option<String>,
    pub actual_gas_cost: Option<String>,
    pub raw_receipt: Value,
}

/// Submit a UserOperation to Candide bundler
pub fn submit_user_operation(
    user_op: Value,
    entry_point: String,
) -> Result<String, OperationError> {
    info!("Submitting UserOperation to Candide bundler");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "eth_sendUserOperation",
        "params": [user_op, entry_point],
        "id": 1
    });

    log_request(&request);

    let response = send_bundler_request(request)?;
    extract_result(response, "user operation hash")
}

/// Estimate gas for a UserOperation
pub fn estimate_user_operation_gas(
    user_op: Value,
    entry_point: String,
) -> Result<Value, OperationError> {
    info!("Estimating gas via Candide bundler");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "eth_estimateUserOperationGas",
        "params": [user_op, entry_point],
        "id": 1
    });

    let response = send_bundler_request(request)?;

    match response.get("result") {
        Some(estimates) => {
            info!("Gas estimation successful");
            Ok(estimates.clone())
        }
        None => {
            error!("Gas estimation response missing result");
            Err(OperationError::internal_error(
                "Invalid gas estimation response",
            ))
        }
    }
}

/// Get receipt for a UserOperation with improved retry logic
/// Returns the transaction hash for proof of payment
pub fn get_user_operation_receipt(
    user_op_hash: String,
) -> Result<UserOperationReceipt, OperationError> {
    info!(
        "Getting UserOperation receipt from Candide for {}",
        user_op_hash
    );

    let request = json!({
        "jsonrpc": "2.0",
        "method": "eth_getUserOperationReceipt",
        "params": [user_op_hash.clone()],
        "id": 1
    });

    // Retry with progressive backoff
    for (attempt, &delay) in RECEIPT_RETRY_DELAYS_SECS.iter().enumerate() {
        match send_bundler_request(request.clone()) {
            Ok(response) => {
                if let Some(result) = response.get("result") {
                    if !result.is_null() {
                        // Extract transaction hash and other details
                        return parse_user_op_receipt(result.clone(), user_op_hash.clone());
                    }

                    // Receipt not ready yet, retry if attempts remain
                    if attempt < RECEIPT_RETRY_DELAYS_SECS.len() - 1 {
                        info!(
                            "Receipt not ready (attempt {}/{}), retrying in {}s...",
                            attempt + 1,
                            RECEIPT_RETRY_DELAYS_SECS.len(),
                            delay
                        );
                        std::thread::sleep(std::time::Duration::from_secs(delay));
                        continue;
                    }
                }
            }
            Err(e) => {
                // Network error, retry if attempts remain
                if attempt < RECEIPT_RETRY_DELAYS_SECS.len() - 1 {
                    info!(
                        "Network error (attempt {}/{}): {:?}, retrying in {}s...",
                        attempt + 1,
                        RECEIPT_RETRY_DELAYS_SECS.len(),
                        e,
                        delay
                    );
                    std::thread::sleep(std::time::Duration::from_secs(delay));
                    continue;
                }
                return Err(e);
            }
        }
    }

    // If we've exhausted all retries (total ~41 seconds), return error
    error!(
        "UserOperation receipt not available after {} attempts",
        RECEIPT_RETRY_DELAYS_SECS.len()
    );
    Err(OperationError::internal_error(
        "Transaction not confirmed after 41 seconds. It may still be pending.",
    ))
}

/// Get receipt with a custom wait strategy
/// Useful when you need the receipt immediately but can tolerate waiting longer
pub fn get_user_operation_receipt_wait(
    user_op_hash: String,
    max_wait_seconds: u64,
) -> Result<UserOperationReceipt, OperationError> {
    info!(
        "Waiting up to {}s for UserOperation receipt: {}",
        max_wait_seconds, user_op_hash
    );

    let request = json!({
        "jsonrpc": "2.0",
        "method": "eth_getUserOperationReceipt",
        "params": [user_op_hash.clone()],
        "id": 1
    });

    let mut elapsed = 0u64;
    let poll_interval = 2u64; // Check every 2 seconds

    while elapsed < max_wait_seconds {
        match send_bundler_request(request.clone()) {
            Ok(response) => {
                if let Some(result) = response.get("result") {
                    if !result.is_null() {
                        return parse_user_op_receipt(result.clone(), user_op_hash.clone());
                    }
                }
            }
            Err(e) => {
                // Log but continue on network errors
                info!("Network error while polling: {:?}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(poll_interval));
        elapsed += poll_interval;

        if elapsed % 10 == 0 {
            info!("Still waiting for receipt... ({}s elapsed)", elapsed);
        }
    }

    error!(
        "UserOperation receipt not available after {}s",
        max_wait_seconds
    );
    Err(OperationError::internal_error(&format!(
        "Transaction not confirmed after {} seconds",
        max_wait_seconds
    )))
}

// ============================================================================
// Private Helper Functions
// ============================================================================

/// Parse the UserOperation receipt to extract transaction hash and other details
fn parse_user_op_receipt(
    receipt_data: Value,
    user_op_hash: String,
) -> Result<UserOperationReceipt, OperationError> {
    // The receipt structure from Candide includes:
    // - userOpHash: the hash of the userOperation
    // - receipt: the actual transaction receipt containing transactionHash
    // - success: whether the userOp succeeded
    // - actualGasUsed: actual gas used
    // - actualGasCost: actual gas cost in wei

    // Extract the transaction hash from the nested receipt
    let transaction_hash = receipt_data
        .get("receipt")
        .and_then(|r| r.get("transactionHash"))
        .and_then(|h| h.as_str())
        .or_else(|| {
            // Fallback: some bundlers put it at the top level
            receipt_data.get("transactionHash").and_then(|h| h.as_str())
        })
        .ok_or_else(|| {
            error!("Receipt missing transaction hash: {:?}", receipt_data);
            OperationError::internal_error("Receipt missing transaction hash")
        })?
        .to_string();

    let success = receipt_data
        .get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(true); // Default to true if not specified

    let actual_gas_used = receipt_data
        .get("actualGasUsed")
        .and_then(|g| g.as_str())
        .map(|s| s.to_string());

    let actual_gas_cost = receipt_data
        .get("actualGasCost")
        .and_then(|g| g.as_str())
        .map(|s| s.to_string());

    info!(
        "UserOperation {} confirmed in transaction {}",
        user_op_hash, transaction_hash
    );

    if !success {
        error!("UserOperation failed: {}", user_op_hash);
    }

    Ok(UserOperationReceipt {
        user_op_hash,
        transaction_hash,
        success,
        actual_gas_used,
        actual_gas_cost,
        raw_receipt: receipt_data,
    })
}

/// Send a JSON-RPC request to the bundler
fn send_bundler_request(request_body: Value) -> Result<Value, OperationError> {
    let url = Url::parse(CANDIDE_BASE_URL)
        .map_err(|e| OperationError::internal_error(&format!("Invalid URL: {}", e)))?;

    let body_bytes = serde_json::to_vec(&request_body).map_err(|e| {
        OperationError::internal_error(&format!("Failed to serialize request: {}", e))
    })?;

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let response = send_request_await_response(
        Method::POST,
        url.clone(),
        Some(headers),
        REQUEST_TIMEOUT_MS,
        body_bytes,
    )
    .map_err(|e| {
        error!("Failed to send request to Candide: {}", e);
        OperationError::internal_error(&format!("Bundler request failed: {}", e))
    })?;

    let response_data: Value = serde_json::from_slice(&response.body()).map_err(|e| {
        error!("Failed to parse response: {}", e);
        OperationError::internal_error(&format!("Invalid bundler response: {}", e))
    })?;

    check_json_rpc_error(&response_data)?;
    Ok(response_data)
}

/// Check for JSON-RPC errors in the response
fn check_json_rpc_error(response: &Value) -> Result<(), OperationError> {
    if let Some(error) = response.get("error") {
        let error_msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        let error_code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);

        error!("JSON-RPC error {}: {}", error_code, error_msg);
        return Err(OperationError::invalid_params(error_msg));
    }
    Ok(())
}

/// Extract a result field from the response
fn extract_result(response: Value, field_name: &str) -> Result<String, OperationError> {
    match response.get("result").and_then(|r| r.as_str()) {
        Some(value) => {
            info!("Successfully retrieved {}: {}", field_name, value);
            Ok(value.to_string())
        }
        None => {
            error!("Response missing {}", field_name);
            Err(OperationError::internal_error(&format!(
                "Invalid response: missing {}",
                field_name
            )))
        }
    }
}

/// Log the bundler request for debugging
fn log_request(request: &Value) {
    if let Ok(pretty) = serde_json::to_string_pretty(request) {
        info!("Candide request: {}", pretty);
    }
}
