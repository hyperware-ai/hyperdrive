/// Wallet management operations

use crate::config::DEFAULT_CHAIN_ID;
use crate::operations::{OperationError, OperationResponse, OperationRequest};
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::logging::info;
use serde_json::{json, Value};

/// Create a new wallet
pub fn create_wallet(params: Value, request: &OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let chain_id = params
        .get("chain_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_CHAIN_ID);
    let password = params.get("password").and_then(|v| v.as_str());

    // Create a new random wallet using LocalSigner
    match LocalSigner::new_random(chain_id) {
        Ok(signer) => {
            let address = signer.address().to_string();

            // Create wallet storage based on whether password is provided
            let key_storage = if let Some(pwd) = password {
                // Encrypt the signer
                match signer.encrypt(pwd) {
                    Ok(encrypted) => KeyStorage::Encrypted(encrypted),
                    Err(e) => {
                        return OperationResponse::error(OperationError::internal_error(
                            &format!("Failed to encrypt wallet: {}", e),
                        ));
                    }
                }
            } else {
                // Store unencrypted for immediate use
                KeyStorage::Decrypted(signer)
            };

            // Create the wallet
            let wallet = Wallet {
                address: address.clone(),
                name,
                chain_id,
                key_storage,
                created_at: chrono::Utc::now(),
                last_used: None,
                spending_limits: None, // No limits by default
            };

            // Store wallet under the process
            state.add_wallet(process_address, wallet);
            
            info!("Created wallet {} for process {}", address, process_address);

            OperationResponse::success(json!({
                "wallet_id": address.clone(),
                "address": address,
                "name": state.get_wallet(process_address, &address).and_then(|w| w.name.clone()),
                "chain_id": chain_id,
                "encrypted": password.is_some(),
                "created_at": chrono::Utc::now()
            }))
        }
        Err(e) => OperationResponse::error(OperationError::internal_error(&format!(
            "Failed to create wallet: {}",
            e
        ))),
    }
}

/// Import an existing wallet from private key
pub fn import_wallet(params: Value, request: &OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let private_key = match params.get("private_key").and_then(|v| v.as_str()) {
        Some(pk) => pk,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: private_key",
            ));
        }
    };
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let chain_id = params
        .get("chain_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_CHAIN_ID);
    let password = params.get("password").and_then(|v| v.as_str());

    // Create signer from private key
    match LocalSigner::from_private_key(private_key, chain_id) {
        Ok(signer) => {
            let address = signer.address().to_string();
            
            // Check if wallet already exists for this process
            if state.get_wallet(process_address, &address).is_some() {
                return OperationResponse::error(OperationError::invalid_params(
                    "Wallet with this address already exists for this process"
                ));
            }

            // Create wallet storage based on whether password is provided
            let key_storage = if let Some(pwd) = password {
                // Encrypt the signer
                match signer.encrypt(pwd) {
                    Ok(encrypted) => KeyStorage::Encrypted(encrypted),
                    Err(e) => {
                        return OperationResponse::error(OperationError::internal_error(
                            &format!("Failed to encrypt wallet: {}", e),
                        ));
                    }
                }
            } else {
                // Store unencrypted for immediate use
                KeyStorage::Decrypted(signer)
            };

            // Create the wallet
            let wallet = Wallet {
                address: address.clone(),
                name,
                chain_id,
                key_storage,
                created_at: chrono::Utc::now(),
                last_used: None,
                spending_limits: None, // No limits by default
            };

            // Store wallet under the process
            state.add_wallet(process_address, wallet);
            
            info!("Imported wallet {} for process {}", address, process_address);

            OperationResponse::success(json!({
                "address": address,
                "name": state.get_wallet(process_address, &address).and_then(|w| w.name.clone()),
                "chain_id": chain_id,
                "encrypted": password.is_some(),
                "imported": true
            }))
        }
        Err(e) => OperationResponse::error(OperationError::internal_error(&format!(
            "Failed to import wallet: {}",
            e
        ))),
    }
}

/// Delete a wallet
pub fn delete_wallet(wallet_id: &str, request: &OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    
    // Get wallet to confirm it exists
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w.clone(),
        None => return OperationResponse::error(OperationError::wallet_not_found(wallet_id)),
    };

    // Remove the wallet
    if state.remove_wallet(process_address, &wallet.address).is_some() {
        // Also remove from active signers cache if present
        state.active_signers.remove(&(process_address.clone(), wallet.address.clone()));
        
        info!("Deleted wallet {} for process {}", wallet.address, process_address);
        
        OperationResponse::success(json!({
            "deleted": wallet.address,
            "message": "Wallet deleted successfully"
        }))
    } else {
        OperationResponse::error(OperationError::internal_error("Failed to delete wallet"))
    }
}

/// Rename a wallet
pub fn rename_wallet(wallet_id: &str, params: Value, request: &OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let new_name = match params.get("new_name").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: new_name",
            ));
        }
    };

    // Get the wallet address for response
    let wallet_address = match state.get_wallet(process_address, wallet_id) {
        Some(wallet) => wallet.address.clone(),
        None => return OperationResponse::error(OperationError::wallet_not_found(wallet_id)),
    };

    // Update the wallet name
    match state.get_wallet_mut(process_address, wallet_id) {
        Some(wallet) => {
            wallet.name = Some(new_name.to_string());
        }
        None => return OperationResponse::error(OperationError::wallet_not_found(wallet_id)),
    }
    
    // Save state
    state.save();
    
    info!("Renamed wallet {} to '{}' for process {}", wallet_address, new_name, process_address);
    
    OperationResponse::success(json!({
        "address": wallet_address,
        "new_name": new_name,
        "message": "Wallet renamed successfully"
    }))
}

/// Export a wallet's private key
pub fn export_wallet(wallet_id: &str, params: Value, request: &OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
    let process_address = &request.auth.process_address;
    let password = params.get("password").and_then(|v| v.as_str());

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => return OperationResponse::error(OperationError::wallet_not_found(wallet_id)),
    };

    // Get the private key based on storage type
    let private_key = match &wallet.key_storage {
        KeyStorage::Decrypted(signer) => signer.export_private_key(),
        KeyStorage::Encrypted(encrypted_data) => {
            // Need password to decrypt
            let pwd = match password {
                Some(p) => p,
                None => {
                    return OperationResponse::error(OperationError::password_required());
                }
            };

            // Decrypt the signer
            match LocalSigner::decrypt(encrypted_data, pwd) {
                Ok(signer) => signer.export_private_key(),
                Err(_) => {
                    return OperationResponse::error(OperationError::decryption_failed());
                }
            }
        }
    };

    info!("Exported wallet {} for process {}", wallet.address, process_address);

    OperationResponse::success(json!({
        "address": wallet.address,
        "private_key": private_key,
        "chain_id": wallet.chain_id
    }))
} 

/// Set spending limits for a specific wallet
pub fn set_wallet_limits(
    wallet_id: &str, 
    params: Value, 
    request: &OperationRequest, 
    state: &mut HyperwalletState
) -> OperationResponse {
    let process_address = &request.auth.process_address;
    
    // Parse the limits from params
    let max_per_call = params
        .get("max_per_call")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let max_total = params
        .get("max_total")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let currency = params
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("USDC")
        .to_string();
    
    // Create the spending limits
    let limits = crate::state::WalletSpendingLimits {
        max_per_call: max_per_call.clone(),
        max_total: max_total.clone(),
        currency: currency.clone(),
        total_spent: "0".to_string(),
        set_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    // Set the limits in the state
    match state.set_wallet_spending_limits(process_address, wallet_id, limits) {
        Ok(()) => {
            info!("Set spending limits for wallet {} (process {}): max_per_call={:?}, max_total={:?}, currency={}", 
                  wallet_id, process_address, max_per_call, max_total, currency);
            
            OperationResponse::success(json!({
                "wallet_id": wallet_id,
                "limits": {
                    "max_per_call": max_per_call,
                    "max_total": max_total,
                    "currency": currency
                }
            }))
        }
        Err(e) => {
            OperationResponse::error(OperationError::invalid_params(&e))
        }
    }
}