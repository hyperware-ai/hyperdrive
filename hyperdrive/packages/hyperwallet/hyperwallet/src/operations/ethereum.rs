/// Ethereum blockchain operations using process_lib's high-level wallet functions
/// 
/// // TODO: Use pertinent functions from hyperware_process_lib::wallet and hyperware_process_lib::signer

use crate::config::DEFAULT_CHAIN_ID;
use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet::{self, EthAmount};
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::logging::{info, error};
use serde_json::{json, Value};
use std::str::FromStr;


/// Sign a hash (e.g., UserOperation hash) using the wallet's signer
pub fn sign_hash(wallet: &Wallet, password: Option<&Value>, user_op_hash: &[u8]) -> Result<Vec<u8>, OperationResponse> {
    // Get the signer from wallet (password will be used if wallet is encrypted)
    let signer = match get_signer_from_wallet(wallet, password) {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    
    // Sign the hash
    let signature = match signer.sign_hash(user_op_hash) {
        Ok(sig) => {
            info!("✅ UserOperation signed successfully");
            sig
        }
        Err(e) => {
            error!("❌ Failed to sign UserOperation: {}", e);
            return Err(OperationResponse::error(
                OperationError::blockchain_error(&format!("Failed to sign hash: {}", e))
            ));
        }
    };
    
    Ok(signature)
}

/// Send ETH to another address
pub fn send_eth(request: OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
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
    let to = match params.get("to").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: to",
            ));
        }
    };

    let amount = match params.get("amount").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: amount",
            ));
        }
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
    let signer = match get_signer_from_wallet(wallet, params.get("password")) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Use the high-level send_eth function from wallet module
    let eth_amount = match EthAmount::from_string(amount) {
        Ok(amt) => amt,
        Err(e) => {
            return OperationResponse::error(OperationError::invalid_params(&format!(
                "Invalid amount format: {}",
                e
            )));
        }
    };
    
    match wallet::send_eth(to, eth_amount, provider, &signer) {
        Ok(receipt) => {
            // Update wallet last used
            if let Some(wallet_mut) = state.get_wallet_mut(process_address, wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            info!("Process {} sent {} ETH from {} to {}", 
                process_address, amount, wallet_address, to);

            OperationResponse::success(json!({
                "transaction_hash": receipt.hash,
                "from": wallet_address,
                "to": to,
                "amount": amount,
                "chain_id": chain_id,
                "status": "pending",
                "details": receipt.details
            }))
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to send ETH: {}",
            e
        ))),
    }
}

/// Helper function to get signer from wallet, handling encryption
pub fn get_signer_from_wallet<'a>(
    wallet: &'a Wallet,
    password: Option<&'a Value>,
) -> Result<LocalSigner, OperationResponse> {
    match &wallet.key_storage {
        KeyStorage::Decrypted(signer) => Ok(signer.clone()),
        KeyStorage::Encrypted(encrypted_data) => {
            // Need password to decrypt
            let pwd = match password.and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    return Err(OperationResponse::error(OperationError::password_required()));
                }
            };

            // Decrypt the signer
            match LocalSigner::decrypt(&encrypted_data, pwd) {
                Ok(signer) => Ok(signer),
                Err(_) => Err(OperationResponse::error(OperationError::decryption_failed())),
            }
        }
    }
}

/// Parse ETH amount string to wei
fn parse_eth_amount(amount_str: &str) -> Result<u128, OperationResponse> {
    let amount_parts: Vec<&str> = amount_str.split_whitespace().collect();
    let amount_num = match amount_parts.get(0).and_then(|s| s.parse::<f64>().ok()) {
        Some(num) => num,
        None => {
            return Err(OperationResponse::error(OperationError::invalid_params(
                &format!("Invalid amount format: {}", amount_str),
            )));
        }
    };

    // Convert ETH to wei
    Ok((amount_num * 1e18) as u128)
}

/// Parse address string, 
fn parse_address(address_str: &str) -> Result<hyperware_process_lib::eth::Address, OperationResponse> {
    match hyperware_process_lib::eth::Address::from_str(address_str) {
        Ok(addr) => Ok(addr),
        Err(_) => {
            Err(OperationResponse::error(OperationError::invalid_params(
                &format!("Invalid address: {}", address_str),
            )))
        }
    }
} 