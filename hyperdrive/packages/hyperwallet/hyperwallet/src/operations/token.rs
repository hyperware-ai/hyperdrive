/// Token operations using process_lib's high-level ERC20 functions

use crate::config::DEFAULT_CHAIN_ID;
use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::state::HyperwalletState;
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet::{self, erc20_transfer, erc20_approve};
use hyperware_process_lib::logging::info;
use alloy_primitives::U256;
use serde_json::json;

/// Send ERC20 tokens to another address
pub fn send_token(request: OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let wallet_id = match request.wallet_id.as_deref() {
        Some(id) => id,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: wallet_id",
            ));
        }
    };

    let params = request.params;
    
    // Extract parameters
    let token_address = match params.get("token").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: token (can be symbol like 'USDC' or address)",
            ));
        }
    };

    let to = match params.get("to").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: to",
            ));
        }
    };

    // Support both decimal amount and raw token units
    let amount: U256 = if let Some(raw_amount) = params.get("amount_raw").and_then(|v| v.as_str()) {
        // Raw token units provided
        match U256::from_str_radix(raw_amount, 10) {
            Ok(amt) => amt,
            Err(_) => {
                return OperationResponse::error(OperationError::invalid_params(
                    "Invalid amount_raw format",
                ));
            }
        }
    } else if let Some(decimal_amount) = params.get("amount").and_then(|v| v.as_f64()) {
        // Decimal amount provided - need to get token decimals first
        let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);
        let provider = Provider::new(chain_id, 60000);
        
        match wallet::erc20_decimals(token_address, &provider) {
            Ok(decimals) => {
                let multiplier = 10_u128.pow(decimals as u32);
                U256::from((decimal_amount * multiplier as f64) as u128)
            }
            Err(e) => {
                return OperationResponse::error(OperationError::blockchain_error(&format!(
                    "Failed to get token decimals: {}",
                    e
                )));
            }
        }
    } else {
        return OperationResponse::error(OperationError::invalid_params(
            "Missing required parameter: amount or amount_raw",
        ));
    };

    let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
        }
    };

    // Verify chain ID matches
    if wallet.chain_id != chain_id {
        return OperationResponse::error(OperationError::invalid_params(&format!(
            "Wallet is configured for chain {}, but request is for chain {}",
            wallet.chain_id, chain_id
        )));
    }

    // Clone needed values
    let wallet_address = wallet.address.clone();

    // Get the signer (handle encryption)
    let signer = match super::ethereum::get_signer_from_wallet(wallet, params.get("password")) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Use the high-level erc20_transfer function from wallet module
    match erc20_transfer(token_address, to, amount, &provider, &signer) {
        Ok(receipt) => {
            // Update wallet last used
            if let Some(wallet_mut) = state.get_wallet_mut(process_address, wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            info!("Process {} sent {} tokens from {} to {}", 
                process_address, amount, wallet_address, to);

            OperationResponse::success(json!({
                "transaction_hash": receipt.hash,
                "from": wallet_address,
                "to": to,
                "token": token_address,
                "amount": amount.to_string(),
                "chain_id": chain_id,
                "status": "pending",
                "details": receipt.details
            }))
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to send token: {}",
            e
        ))),
    }
}

/// Approve token spending
pub fn approve_token(request: OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let wallet_id = match request.wallet_id.as_deref() {
        Some(id) => id,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: wallet_id",
            ));
        }
    };

    let params = request.params;
    
    // Extract parameters
    let token_address = match params.get("token").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: token",
            ));
        }
    };

    let spender = match params.get("spender").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: spender",
            ));
        }
    };

    // Support both decimal amount and raw token units
    let amount: U256 = if let Some(raw_amount) = params.get("amount_raw").and_then(|v| v.as_str()) {
        match U256::from_str_radix(raw_amount, 10) {
            Ok(amt) => amt,
            Err(_) => {
                return OperationResponse::error(OperationError::invalid_params(
                    "Invalid amount_raw format",
                ));
            }
        }
    } else if let Some(max_str) = params.get("amount").and_then(|v| v.as_str()) {
        if max_str == "max" || max_str == "unlimited" {
            U256::MAX
        } else if let Ok(decimal_amount) = max_str.parse::<f64>() {
            // Get token decimals to convert
            let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);
            let provider = Provider::new(chain_id, 60000);
            
            match wallet::erc20_decimals(token_address, &provider) {
                Ok(decimals) => {
                    let multiplier = 10_u128.pow(decimals as u32);
                    U256::from((decimal_amount * multiplier as f64) as u128)
                }
                Err(e) => {
                    return OperationResponse::error(OperationError::blockchain_error(&format!(
                        "Failed to get token decimals: {}",
                        e
                    )));
                }
            }
        } else {
            return OperationResponse::error(OperationError::invalid_params(
                "Invalid amount format",
            ));
        }
    } else {
        return OperationResponse::error(OperationError::invalid_params(
            "Missing required parameter: amount or amount_raw",
        ));
    };

    let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
        }
    };

    // Verify chain ID matches
    if wallet.chain_id != chain_id {
        return OperationResponse::error(OperationError::invalid_params(&format!(
            "Wallet is configured for chain {}, but request is for chain {}",
            wallet.chain_id, chain_id
        )));
    }

    // Clone needed values
    let wallet_address = wallet.address.clone();

    // Get the signer (handle encryption)
    let signer = match super::ethereum::get_signer_from_wallet(wallet, params.get("password")) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Use the high-level erc20_approve function
    match erc20_approve(token_address, spender, amount, &provider, &signer) {
        Ok(receipt) => {
            // Update wallet last used
            if let Some(wallet_mut) = state.get_wallet_mut(process_address, wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            info!("Process {} approved {} to spend {} tokens", 
                process_address, spender, amount);

            OperationResponse::success(json!({
                "transaction_hash": receipt.hash,
                "from": wallet_address,
                "token": token_address,
                "spender": spender,
                "amount": amount.to_string(),
                "chain_id": chain_id,
                "status": "pending",
                "details": receipt.details
            }))
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to approve token: {}",
            e
        ))),
    }
}

/// Get token balance
pub fn get_token_balance(
    wallet_id: Option<&str>,
    process_address: &str,
    token_address: Option<&str>,
    chain_id: Option<u64>,
    state: &HyperwalletState,
) -> OperationResponse {
    let wallet_id = match wallet_id {
        Some(id) => id,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: wallet_id",
            ));
        }
    };

    let token_address = match token_address {
        Some(addr) => addr,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: token_address",
            ));
        }
    };

    let chain_id = chain_id.unwrap_or(DEFAULT_CHAIN_ID);

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
        }
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Get token details
    match wallet::get_token_details(token_address, &wallet.address, &provider) {
        Ok(details) => OperationResponse::success(json!({
            "wallet_address": wallet.address,
            "token_address": details.address,
            "token_symbol": details.symbol,
            "token_name": details.name,
            "decimals": details.decimals,
            "balance": details.balance,
            "formatted_balance": details.formatted_balance,
            "chain_id": chain_id
        })),
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to get token balance: {}",
            e
        ))),
    }
} 