/// State-changing on-chain transaction operations
///
/// This module handles all transactions that modify blockchain state:
/// - ETH transfers
/// - ERC20 token transfers and approvals
/// - Token Bound Account (TBA) executions
/// - Transaction signing utilities
use crate::config::DEFAULT_CHAIN_ID;
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use alloy_primitives::{hex, U256};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::hyperware::process::hyperwallet::{
    CallContractRequest, CallContractResponse,
};
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletResponse, HyperwalletResponseData, OperationError, SendEthRequest, SendEthResponse,
    SendTokenRequest, SendTokenResponse,
};
use hyperware_process_lib::logging::info;
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::wallet::{self, erc20_transfer, EthAmount};
use hyperware_process_lib::Address;
use serde_json::Value;

pub fn get_signer_from_wallet(
    wallet: &Wallet,
    password: Option<&Value>,
) -> Result<LocalSigner, HyperwalletResponse> {
    match &wallet.key_storage {
        KeyStorage::Decrypted(signer) => Ok(signer.clone()),
        KeyStorage::Encrypted(encrypted_data) => {
            let pwd = match password.and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return Err(HyperwalletResponse::error(OperationError::invalid_params(
                        "Password required for encrypted wallet",
                    )));
                }
            };

            match LocalSigner::decrypt(encrypted_data, pwd) {
                Ok(signer) => Ok(signer),
                Err(_) => Err(HyperwalletResponse::error(OperationError::internal_error(
                    "Failed to decrypt wallet",
                ))),
            }
        }
    }
}

pub fn sign_hash(
    wallet: &Wallet,
    password: Option<&Value>,
    user_op_hash: &[u8],
) -> Result<Vec<u8>, HyperwalletResponse> {
    let signer = match get_signer_from_wallet(wallet, password) {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    match signer.sign_hash(user_op_hash) {
        Ok(signature) => {
            info!("UserOperation signed successfully");
            Ok(signature)
        }
        Err(e) => Err(HyperwalletResponse::error(OperationError::internal_error(
            &format!("Failed to sign hash: {}", e),
        ))),
    }
}

pub fn call_contract(
    request: CallContractRequest,
    session_id: &str,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    let data = &request;
    let chain_id = DEFAULT_CHAIN_ID;

    // Get the first unlocked wallet from the session (operator model)
    // This ensures we use the same wallet as other operations like payments
    let wallet_id = match state.validate_session(&session_id.to_string()) {
        Some(session_data) => {
            match session_data.unlocked_wallets.iter().next() {
                Some(addr) => addr.clone(),
                None => {
                    // Fallback: use first available wallet for the process
                    match state.list_wallets(address).into_iter().next() {
                        Some(w) => w.address.clone(),
                        None => {
                            return HyperwalletResponse::error(OperationError::invalid_params(
                                "No wallet available for this process",
                            ));
                        }
                    }
                }
            }
        }
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Invalid or expired session",
            ));
        }
    };

    let wallet = match state.get_wallet(address, &wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Wallet {} not found",
                wallet_id
            )));
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

    let value_str = data.value.as_deref().unwrap_or("0");
    let value = match EthAmount::from_string(value_str) {
        Ok(amt) => amt,
        Err(e) => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Invalid amount format: {}",
                e
            )));
        }
    };

    let call_data = match hex::decode(data.data.trim_start_matches("0x")) {
        Ok(bytes) => bytes,
        Err(_) => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Invalid call data hex",
            ));
        }
    };

    let to_address = match data.to.parse() {
        Ok(addr) => addr,
        Err(_) => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Invalid contract address",
            ));
        }
    };

    // Build and send transaction (inlined logic from wallet::prepare_and_send_tx)
    use hyperware_process_lib::signer::TransactionData;
    
    let nonce = match provider.get_transaction_count(signer.address(), None) {
        Ok(n) => n.to::<u64>(),
        Err(e) => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Failed to get nonce: {:?}",
                e
            )));
        }
    };
    
    let (gas_price, priority_fee) = match calculate_gas_params(&provider, chain_id) {
        Ok(params) => params,
        Err(e) => {
            return HyperwalletResponse::error(OperationError::internal_error(&format!(
                "Failed to calculate gas: {:?}",
                e
            )));
        }
    };
    
    let tx_data = TransactionData {
        to: to_address,
        value: value.as_wei(),
        data: Some(call_data),
        nonce,
        gas_limit: 200_000,
        gas_price,
        max_priority_fee: Some(priority_fee),
        chain_id,
    };
    
    let signed_tx = match signer.sign_transaction(&tx_data) {
        Ok(bytes) => bytes,
        Err(e) => {
            return HyperwalletResponse::error(OperationError::internal_error(&format!(
                "Failed to sign transaction: {}",
                e
            )));
        }
    };
    
    let tx_hash = match provider.send_raw_transaction(signed_tx.into()) {
        Ok(hash) => hash,
        Err(e) => {
            return HyperwalletResponse::error(OperationError::internal_error(&format!(
                "Failed to send transaction: {:?}",
                e
            )));
        }
    };
    
    info!("Contract call transaction: 0x{:x}", tx_hash);
    HyperwalletResponse::success(HyperwalletResponseData::CallContract(CallContractResponse {
        tx_hash: format!("0x{:x}", tx_hash),
        from_address: wallet_address,
        to_address: data.to.clone(),
        amount: data.value.clone(),
        chain_id,
    }))
}

// Helper from wallet.rs for gas calculation
fn calculate_gas_params(provider: &Provider, chain_id: u64) -> Result<(u128, u128), String> {
    use hyperware_process_lib::eth::BlockNumberOrTag;
    
    match chain_id {
        8453 => {
            // Base
            let latest_block = provider
                .get_block_by_number(BlockNumberOrTag::Latest, false)
                .map_err(|e| format!("Failed to get block: {:?}", e))?
                .ok_or_else(|| "No latest block".to_string())?;

            let base_fee = latest_block
                .header
                .inner
                .base_fee_per_gas
                .ok_or_else(|| "No base fee in block".to_string())?
                as u128;

            let max_fee = base_fee + (base_fee / 3);
            let priority_fee = std::cmp::max(100_000u128, base_fee / 10);

            Ok((max_fee, priority_fee))
        }
        _ => {
            let base_fee = provider.get_gas_price()
                .map_err(|e| format!("Failed to get gas price: {:?}", e))?
                .to::<u128>();
            let adjusted_fee = (base_fee * 130) / 100;
            Ok((adjusted_fee, adjusted_fee / 10))
        }
    }
}

pub fn send_eth(
    request: SendEthRequest,
    _session_id: &str,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    let data = &request;
    let chain_id = DEFAULT_CHAIN_ID;

    let wallet = match state.get_wallet(address, &data.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Wallet not found: {}",
                &data.wallet_id
            )));
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

            info!(
                "Process {} sent {} ETH from {} to {}",
                address, data.amount, wallet_address, data.to
            );

            HyperwalletResponse::success(HyperwalletResponseData::SendEth(SendEthResponse {
                tx_hash: format!("0x{:x}", receipt.hash),
                from_address: wallet_address,
                to_address: data.to.clone(),
                amount: data.amount.clone(),
                chain_id,
            }))
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to send ETH: {}",
            e
        ))),
    }
}

pub fn send_token(
    request: SendTokenRequest,
    _session_id: &str,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    let data = &request;
    let chain_id = DEFAULT_CHAIN_ID;

    let wallet = match state.get_wallet(address, &data.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Wallet not found: {}",
                &data.wallet_id
            )));
        }
    };

    info!(
        "Token send request from {} for wallet {}: {} {} to {}",
        address, data.wallet_id, data.amount, data.token_address, data.to
    );

    let amount: U256 = if data.amount.starts_with("0x") {
        match U256::from_str_radix(&data.amount[2..], 16) {
            Ok(amt) => amt,
            Err(_) => {
                return HyperwalletResponse::error(OperationError::invalid_params(
                    "Invalid hex amount format",
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
                    "Failed to get token decimals: {}",
                    e
                )));
            }
        }
    } else {
        return HyperwalletResponse::error(OperationError::invalid_params(
            "Invalid amount format - must be decimal number, integer, or hex",
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

            info!(
                "Process {} sent {} tokens from {} to {}",
                address, amount, wallet_address, data.to
            );

            HyperwalletResponse::success(HyperwalletResponseData::SendToken(SendTokenResponse {
                tx_hash: format!("0x{:x}", receipt.hash),
                from_address: wallet_address,
                to_address: data.to.clone(),
                token_address: data.token_address.clone(),
                amount: data.amount.clone(),
                chain_id,
            }))
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to send token: {}",
            e
        ))),
    }
}

///// Execute a transaction through a TBA
//pub fn execute_via_tba(
//    request: HyperwalletRequest<ExecuteViaTbaRequest>,
//    address: &Address,
//    state: &mut HyperwalletState,
//) -> HyperwalletResponse {
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
