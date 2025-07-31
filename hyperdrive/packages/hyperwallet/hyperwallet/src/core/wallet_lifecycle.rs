/// Wallet lifecycle management operations
/// 
/// This module manages wallet creation, import, deletion, renaming, export and limits

use crate::config::DEFAULT_CHAIN_ID;
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletRequest, HyperwalletResponse, HyperwalletResponseData, OperationError,
    CreateWalletRequest, CreateWalletResponse, ImportWalletRequest, ImportWalletResponse,
    DeleteWalletRequest, DeleteWalletResponse, ExportWalletRequest, ExportWalletResponse,
    RenameWalletRequest
};
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::logging::info;
use hyperware_process_lib::Address;

pub fn create_wallet(
    request: HyperwalletRequest<CreateWalletRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;
    let chain_id = DEFAULT_CHAIN_ID;

    match LocalSigner::new_random(chain_id) {
        Ok(signer) => {
            let wallet_address = signer.address().to_string();

            let key_storage = if let Some(ref password) = data.password {
                match signer.encrypt(password) {
                    Ok(encrypted) => KeyStorage::Encrypted(encrypted),
                    Err(e) => {
                        return HyperwalletResponse::error(OperationError::internal_error(
                            &format!("Failed to encrypt wallet: {}", e),
                        ));
                    }
                }
            } else {
                KeyStorage::Decrypted(signer)
            };

            let wallet = Wallet {
                address: wallet_address.clone(),
                name: Some(data.name.clone()),
                chain_id,
                key_storage,
                created_at: chrono::Utc::now(),
                last_used: None,
                spending_limits: None,
            };

            state.add_wallet(address.clone(), wallet);
            
            info!("Created wallet {} for process {}", wallet_address, address);

            let result = HyperwalletResponse::success(CreateWalletResponse {
                wallet_id: wallet_address.clone(),
                address: wallet_address,
                name: data.name.clone(),
            });
            result.map(HyperwalletResponseData::CreateWallet)
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to create wallet: {}",
            e
        ))),
    }
}

pub fn import_wallet(
    request: HyperwalletRequest<ImportWalletRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;
    let chain_id = DEFAULT_CHAIN_ID;

    match LocalSigner::from_private_key(&data.private_key, chain_id) {
        Ok(signer) => {
            let wallet_address = signer.address().to_string();
            
            if state.get_wallet(address, &wallet_address).is_some() {
                return HyperwalletResponse::error(OperationError::invalid_params(
                    "Wallet with this address already exists for this process"
                ));
            }

            let key_storage = if let Some(ref password) = data.password {
                match signer.encrypt(password) {
                    Ok(encrypted) => KeyStorage::Encrypted(encrypted),
                    Err(e) => {
                        return HyperwalletResponse::error(OperationError::internal_error(
                            &format!("Failed to encrypt wallet: {}", e),
                        ));
                    }
                }
            } else {
                KeyStorage::Decrypted(signer)
            };

            // Create the wallet
            let wallet = Wallet {
                address: wallet_address.clone(),
                name: Some(data.name.clone()),
                chain_id,
                key_storage,
                created_at: chrono::Utc::now(),
                last_used: None,
                spending_limits: None,
            };

            state.add_wallet(address.clone(), wallet);
            
            info!("Imported wallet {} for process {}", wallet_address, address);

            let result = HyperwalletResponse::success(ImportWalletResponse {
                wallet_id: wallet_address.clone(),
                address: wallet_address,
                name: data.name.clone(),
            });
            result.map(HyperwalletResponseData::ImportWallet)
        }
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to import wallet: {}",
            e
        ))),
    }
}

pub fn delete_wallet(
    request: HyperwalletRequest<DeleteWalletRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let wallet_id = &request.data.wallet_id;

    let wallet = match state.get_wallet(address, wallet_id) {
        Some(w) => w.clone(),
        None => return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", wallet_id))),
    };

    if state.remove_wallet(address, &wallet.address).is_some() {
        state.active_signers.remove(&(address.clone(), wallet.address.clone()));
        
        info!("Deleted wallet {} for process {}", wallet.address, address);
        
        let result = HyperwalletResponse::success(DeleteWalletResponse {
            wallet_id: wallet.address.clone(),
            success: true,
            message: "Wallet deleted successfully".to_string(),
        });
        result.map(HyperwalletResponseData::DeleteWallet)
    } else {
        HyperwalletResponse::error(OperationError::internal_error("Failed to delete wallet"))
    }
}

pub fn rename_wallet(
    request: HyperwalletRequest<RenameWalletRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;
    let new_name = &data.new_name;
    let wallet_id = &data.wallet_id;
    
    match state.get_wallet_mut(address, wallet_id) {
        Some(wallet) => {
            wallet.name = Some(new_name.clone());
            let wallet_address = wallet.address.clone();
            
            let _ = wallet;
            state.save();
            
            info!("Renamed wallet {} to '{}' for process {}", wallet_id, new_name, address);
            
            let result = HyperwalletResponse::success(serde_json::json!({
                "wallet_id": wallet_address.clone(),
                "address": wallet_address,
                "new_name": new_name.clone(),
                "message": "Wallet renamed successfully"
            }));
            
            // TODO: Add RenameWallet variant to HyperwalletResponseData enum and use it here
            result.map(|data| HyperwalletResponseData::CreateWallet(
                serde_json::from_value(data).unwrap_or_else(|_| {
                    // Fallback to a basic CreateWalletResponse structure
                    hyperware_process_lib::hyperwallet_client::types::CreateWalletResponse {
                        wallet_id: wallet_address.clone(),
                        address: wallet_address,
                        name: new_name.clone(),
                    }
                })
            ))
        }
        None => HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", wallet_id))),
    }
}

pub fn export_wallet(
    request: HyperwalletRequest<ExportWalletRequest>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    let data = &request.data;

    let wallet = match state.get_wallet(address, &data.wallet_id) {
        Some(w) => w,
        None => return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &data.wallet_id))),
    };

    let private_key = match &wallet.key_storage {
        KeyStorage::Decrypted(signer) => signer.export_private_key(),
        KeyStorage::Encrypted(encrypted_data) => {
            let pwd = match data.password.as_deref() {
                Some(p) => p,
                None => {
                    return HyperwalletResponse::error(OperationError::invalid_params("Password required for encrypted wallet"));
                }
            };

            match LocalSigner::decrypt(encrypted_data, pwd) {
                Ok(signer) => signer.export_private_key(),
                Err(_) => {
                    return HyperwalletResponse::error(OperationError::internal_error("Failed to decrypt wallet"));
                }
            }
        }
    };

    info!("Exported wallet {} for {}", wallet.address, address);

    let result = HyperwalletResponse::success(ExportWalletResponse {
        address: wallet.address.clone(),
        private_key,
    });
    result.map(HyperwalletResponseData::ExportWallet)
}

pub fn set_wallet_limits(
    wallet_id: &str,
    params: serde_json::Value,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<serde_json::Value> {
    let max_per_call = params.get("max_per_call").and_then(|v| v.as_str()).map(|s| s.to_string());
    let max_total = params.get("max_total").and_then(|v| v.as_str()).map(|s| s.to_string());
    let currency = params.get("currency").and_then(|v| v.as_str()).unwrap_or("USDC").to_string();
    
    let limits = crate::state::WalletSpendingLimits {
        max_per_call: max_per_call.clone(),
        max_total: max_total.clone(),
        currency: currency.clone(),
        total_spent: "0".to_string(),
        set_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    match state.set_wallet_spending_limits(address, wallet_id, limits) {
        Ok(()) => {
            info!("Set spending limits for wallet {} (process {}): max_per_call={:?}, max_total={:?}, currency={}", 
                  wallet_id, address, max_per_call, max_total, currency);
            
            // TODO: Replace json! macro with typed SetWalletLimitsResponse struct
            HyperwalletResponse::success(serde_json::json!({
                "wallet_id": wallet_id,
                "limits": {
                    "max_per_call": max_per_call,
                    "max_total": max_total,
                    "currency": currency
                }
            }))
        }
        Err(e) => {
            HyperwalletResponse::error(OperationError::invalid_params(&e))
        }
    }
} 