/// State-changing on-chain transaction operations
/// 
/// This module handles all transactions that modify blockchain state:
/// - ETH transfers
/// - ERC20 token transfers and approvals
/// - Token Bound Account (TBA) executions
/// - Transaction signing utilities

use crate::config::DEFAULT_CHAIN_ID;
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletRequest, HyperwalletResponse, HyperwalletResponseData, OperationError,
    SendEthRequest, SendEthResponse, SendTokenRequest, SendTokenResponse, ExecuteViaTbaRequest, ExecuteViaTbaResponse,
};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet::{self, EthAmount, erc20_transfer};
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::logging::info;
use hyperware_process_lib::Address;
use serde_json::{json, Value};
use alloy_primitives::U256;

pub fn get_signer_from_wallet(
    wallet: &Wallet,
    password: Option<&Value>,
) -> Result<LocalSigner, HyperwalletResponse<serde_json::Value>> {
    match &wallet.key_storage {
        KeyStorage::Decrypted(signer) => Ok(signer.clone()),
        KeyStorage::Encrypted(encrypted_data) => {
            let pwd = match password.and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return Err(HyperwalletResponse::error(OperationError::invalid_params("Password required for encrypted wallet")));
                }
            };

            match LocalSigner::decrypt(encrypted_data, pwd) {
                Ok(signer) => Ok(signer),
                Err(_) => Err(HyperwalletResponse::error(OperationError::internal_error("Failed to decrypt wallet"))),
            }
        }
    }
}

pub fn sign_hash(
    wallet: &Wallet, 
    password: Option<&Value>, 
    user_op_hash: &[u8]
) -> Result<Vec<u8>, HyperwalletResponse<serde_json::Value>> {
    let signer = match get_signer_from_wallet(wallet, password) {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    
    match signer.sign_hash(user_op_hash) {
        Ok(signature) => {
            info!("UserOperation signed successfully");
            Ok(signature)
        }
        Err(e) => {
            Err(HyperwalletResponse::error(OperationError::internal_error(&format!("Failed to sign hash: {}", e))))
        }
    }
}

pub fn send_eth(
    request: HyperwalletRequest<SendEthRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;
    let chain_id = DEFAULT_CHAIN_ID;

    let wallet = match state.get_wallet(address, &data.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &data.wallet_id)));
        }
    };

    if wallet.chain_id != chain_id {
        return HyperwalletResponse::error(OperationError::invalid_params(&format!(
            "Wallet is configured for chain {}, but request is for chain {}",
            wallet.chain_id, chain_id
        )));
    }

    let wallet_address = wallet.address.clone();

    let signer = match &wallet.key_storage {
        crate::state::KeyStorage::Decrypted(signer) => signer.clone(),
        crate::state::KeyStorage::Encrypted(_encrypted_data) => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Encrypted wallet operations not yet supported - unlock wallet first",
            ));
        }
    };

    let provider = Provider::new(chain_id, 60000);

    let eth_amount = match EthAmount::from_string(&data.amount) {
        Ok(amt) => amt,
        Err(e) => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Invalid amount format: {}",
                e
            )));
        }
    };
    
    match wallet::send_eth(&data.to, eth_amount, provider, &signer) {
        Ok(receipt) => {
            if let Some(wallet_mut) = state.get_wallet_mut(address, &data.wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            info!("Process {} sent {} ETH from {} to {}", 
                address, data.amount, wallet_address, data.to);

            let result = HyperwalletResponse::success(SendEthResponse {
                tx_hash: format!("0x{:x}", receipt.hash),
                from_address: wallet_address,
                to_address: data.to.clone(),
                amount: data.amount.clone(),
                chain_id,
            });
            result.map(HyperwalletResponseData::SendEth)
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to send ETH: {}",
            e
        ))),
    }
}

pub fn send_token(
    request: HyperwalletRequest<SendTokenRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;
    let chain_id = DEFAULT_CHAIN_ID;

    let wallet = match state.get_wallet(address, &data.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &data.wallet_id)));
        }
    };

    info!("Token send request from {} for wallet {}: {} {} to {}", 
          address, data.wallet_id, data.amount, data.token_address, data.to);

    let amount: U256 = if data.amount.starts_with("0x") {
        match U256::from_str_radix(&data.amount[2..], 16) {
            Ok(amt) => amt,
            Err(_) => {
                return HyperwalletResponse::error(OperationError::invalid_params(
                    "Invalid hex amount format"
                ));
            }
        }
    } else if let Ok(raw_amount) = U256::from_str_radix(&data.amount, 10) {
        raw_amount
    } else if let Ok(decimal_amount) = data.amount.parse::<f64>() {
        let provider = Provider::new(chain_id, 60000);
        
        match wallet::erc20_decimals(&data.token_address, &provider) {
            Ok(decimals) => {
                let multiplier = 10_u128.pow(decimals as u32);
                U256::from((decimal_amount * multiplier as f64) as u128)
            }
            Err(e) => {
                return HyperwalletResponse::error(OperationError::internal_error(&format!(
                    "Failed to get token decimals: {}", e
                )));
            }
        }
    } else {
        return HyperwalletResponse::error(OperationError::invalid_params(
            "Invalid amount format - must be decimal number, integer, or hex"
        ));
    };

    let wallet_address = wallet.address.clone();

    let signer = match &wallet.key_storage {
        crate::state::KeyStorage::Decrypted(signer) => signer.clone(),
        crate::state::KeyStorage::Encrypted(_encrypted_data) => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Encrypted wallet operations not yet supported - unlock wallet first",
            ));
        }
    };

    let provider = Provider::new(chain_id, 60000);

    match erc20_transfer(&data.token_address, &data.to, amount, &provider, &signer) {
        Ok(receipt) => {
            if let Some(wallet_mut) = state.get_wallet_mut(address, &data.wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            info!("Process {} sent {} tokens from {} to {}", 
                address, amount, wallet_address, data.to);

            let result = HyperwalletResponse::success(SendTokenResponse {
                tx_hash: format!("0x{:x}", receipt.hash),
                from_address: wallet_address,
                to_address: data.to.clone(),
                token_address: data.token_address.clone(),
                amount: data.amount.clone(),
                chain_id,
            });
            result.map(HyperwalletResponseData::SendToken)
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to send token: {}", e
        ))),
    }
}

///// Execute a transaction through a TBA
//pub fn execute_via_tba(
//    request: HyperwalletRequest<ExecuteViaTbaRequest>,
//    address: &Address,
//    state: &mut HyperwalletState,
//) -> HyperwalletResponse<HyperwalletResponseData> {
//    // eoa should be in request 
//    //let eoa_signer= match request.data.wallet_id.as_deref() {
//    //    Some(id) => id,
//    //    None => {
//    //        return HyperwalletResponse::error(OperationError::invalid_params(
//    //            "Missing required parameter: wallet_id",
//    //        ));
//    //    }
//    //};
//
//    // Extract parameters
//    let tba_address = match request.data.tba_address.as_deref() {
//        Some(addr) => addr,
//        None => {
//            return HyperwalletResponse::error(
//                OperationError::invalid_params("Missing required parameter: tba_address"),
//            );
//        }
//    };
//
//    let target = match request.data.target.as_deref() {
//        Some(t) => t,
//        None => {
//            return HyperwalletResponse::error(OperationError::invalid_params(
//                "Missing required parameter: target",
//            ));
//        }
//    };
//
//    let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("0");
//    
//    let call_data = match params.get("call_data").and_then(|v| v.as_str()) {
//        Some(data) => {
//            // Parse hex string to bytes
//            match hex::decode(data.trim_start_matches("0x")) {
//                Ok(bytes) => bytes,
//                Err(e) => {
//                    return OperationResponse::error(OperationError::invalid_params(&format!(
//                        "Invalid call_data hex: {}",
//                        e
//                    )));
//                }
//            }
//        }
//        None => Vec::new(),
//    };
//
//    let operation = params.get("operation").and_then(|v| v.as_u64()).unwrap_or(0);
//    let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);
//
//    // Get the wallet
//    let wallet = match state.get_wallet(address, wallet_id) {
//        Some(w) => w,
//        None => {
//            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
//        }
//    };
//
//    // Get the signer
//    let signer = match crate::core::transactions::get_signer_from_wallet(wallet, params.get("password")) {
//        Ok(s) => s,
//        Err(e) => return OperationResponse::from_hyperwallet_response(e),
//    };
//
//    // Get provider for the chain
//    let provider = Provider::new(chain_id, 60000);
//
//    // Use the high-level execute_via_tba_with_signer function
//    match wallet::execute_via_tba_with_signer(
//        tba_address,
//        &signer,
//        target,
//        call_data,
//        value.parse().unwrap_or(U256::ZERO),
//        &provider,
//        Some(operation as u8),
//    ) {
//        Ok(receipt) => {
//            // Update wallet last used
//            if let Some(wallet_mut) = state.get_wallet_mut(address, wallet_id) {
//                wallet_mut.last_used = Some(chrono::Utc::now());
//            }
//            state.save();
//
//            OperationResponse::success(json!({
//                "transaction_hash": receipt.hash,
//                "tba_address": tba_address,
//                "target": target,
//                "value": value,
//                "operation": operation,
//                "chain_id": chain_id,
//                "details": receipt.details
//            }))
//        }
//        Err(e) => OperationResponse::error(OperationError::internal_error(&format!(
//            "Failed to execute via TBA: {}",
//            e
//        ))),
//    }
//}