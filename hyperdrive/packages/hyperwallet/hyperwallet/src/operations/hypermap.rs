/// Hypermap and TBA operations using process_lib's high-level functions

use crate::config::DEFAULT_CHAIN_ID;
use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::state::HyperwalletState;
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet::{self, resolve_name};
use hyperware_process_lib::hypermap;
use serde_json::json;
use alloy_primitives::hex;
use alloy_primitives::U256;


/// Resolve a Hypermap identity (name -> address)
pub fn resolve_identity(
    entry_name: Option<&str>,
    chain_id: Option<u64>,
    _state: &HyperwalletState,
) -> OperationResponse {
    let entry_name = match entry_name {
        Some(name) => name,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: entry_name",
            ));
        }
    };
    
    let chain_id = chain_id.unwrap_or(DEFAULT_CHAIN_ID);
    
    // Use wallet::resolve_name which handles .hypr names
    match resolve_name(entry_name, chain_id) {
        Ok(address) => {
            // Also try to get TBA information if it's a hypermap entry
            let provider = Provider::new(chain_id, 60000);
            let hypermap_info = provider.hypermap();
            
            // Try to get the full hypermap info
            let namehash = hypermap::namehash(entry_name);
            match hypermap_info.get_hash(&namehash) {
                Ok((tba, owner, _data)) => {
                    OperationResponse::success(json!({
                        "entry_name": entry_name,
                        "resolved_address": address.to_string(),
                        "tba_address": tba.to_string(),
                        "owner_address": owner.to_string(),
                        "chain_id": chain_id,
                        "type": "hypermap_entry"
                    }))
                }
                Err(_) => {
                    // Not a hypermap entry, just return the resolved address
                    OperationResponse::success(json!({
                        "entry_name": entry_name,
                        "resolved_address": address.to_string(),
                        "chain_id": chain_id,
                        "type": "address"
                    }))
                }
            }
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to resolve identity: {}",
            e
        ))),
    }
}

/// Create a note on a Hypermap entry
pub fn create_note(request: OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
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
    let entry_name = match params.get("entry_name").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: entry_name",
            ));
        }
    };

    let note_data = match params.get("note_data") {
        Some(data) => data,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: note_data",
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

    // Get the signer
    let signer = match super::ethereum::get_signer_from_wallet(wallet, params.get("password")) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Use the high-level create_note function from wallet module
    // Note: The wallet module's create_note expects different parameters
    // For now, we'll convert the note_data to bytes
    let note_bytes = serde_json::to_vec(note_data).unwrap_or_default();
    
    match wallet::create_note(entry_name, "~note", note_bytes, provider, &signer) {
        Ok(receipt) => {
            // Update wallet last used
            if let Some(wallet_mut) = state.get_wallet_mut(process_address, wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            OperationResponse::success(json!({
                "transaction_hash": receipt.hash,
                "entry_name": entry_name,
                "note_created": true,
                "chain_id": chain_id,
                "description": receipt.description
            }))
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to create note: {}",
            e
        ))),
    }
}

/// Execute a transaction through a TBA
pub fn execute_via_tba(request: OperationRequest, state: &mut HyperwalletState) -> OperationResponse {
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
    let tba_address = match params.get("tba_address").and_then(|v| v.as_str()) {
        Some(addr) => addr,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: tba_address",
            ));
        }
    };

    let target = match params.get("target").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: target",
            ));
        }
    };

    let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("0");
    
    let call_data = match params.get("call_data").and_then(|v| v.as_str()) {
        Some(data) => {
            // Parse hex string to bytes
            match hex::decode(data.trim_start_matches("0x")) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return OperationResponse::error(OperationError::invalid_params(&format!(
                        "Invalid call_data hex: {}",
                        e
                    )));
                }
            }
        }
        None => Vec::new(),
    };

    let operation = params.get("operation").and_then(|v| v.as_u64()).unwrap_or(0);
    let chain_id = request.chain_id.unwrap_or(DEFAULT_CHAIN_ID);

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
        }
    };

    // Get the signer
    let signer = match super::ethereum::get_signer_from_wallet(wallet, params.get("password")) {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Use the high-level execute_via_tba_with_signer function
    match wallet::execute_via_tba_with_signer(
        tba_address,
        &signer,
        target,
        call_data,
        value.parse().unwrap_or(U256::ZERO),
        &provider,
        Some(operation as u8),
    ) {
        Ok(receipt) => {
            // Update wallet last used
            if let Some(wallet_mut) = state.get_wallet_mut(process_address, wallet_id) {
                wallet_mut.last_used = Some(chrono::Utc::now());
            }
            state.save();

            OperationResponse::success(json!({
                "transaction_hash": receipt.hash,
                "tba_address": tba_address,
                "target": target,
                "value": value,
                "operation": operation,
                "chain_id": chain_id,
                "details": receipt.details
            }))
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to execute via TBA: {}",
            e
        ))),
    }
}

/// Check if an address is a valid signer for a TBA
pub fn check_tba_ownership(
    tba_address: Option<&str>,
    signer_address: Option<&str>,
    chain_id: Option<u64>,
    _state: &HyperwalletState,
) -> OperationResponse {
    let tba_address = match tba_address {
        Some(addr) => addr,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: tba_address",
            ));
        }
    };
    
    let signer_address = match signer_address {
        Some(addr) => addr,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "Missing required parameter: signer_address",
            ));
        }
    };
    
    let chain_id = chain_id.unwrap_or(DEFAULT_CHAIN_ID);
    let provider = Provider::new(chain_id, 60000);
    
    // Check if the signer is valid
    match wallet::tba_is_valid_signer(tba_address, signer_address, &provider) {
        Ok(is_valid) => {
            // Also get TBA token info
            match wallet::tba_get_token_info(tba_address, &provider) {
                Ok((token_chain_id, token_contract, token_id)) => {
                    OperationResponse::success(json!({
                        "tba_address": tba_address,
                        "signer_address": signer_address,
                        "is_valid_signer": is_valid,
                        "token_info": {
                            "chain_id": token_chain_id,
                            "token_contract": token_contract.to_string(),
                            "token_id": token_id.to_string()
                        },
                        "chain_id": chain_id
                    }))
                }
                Err(_) => {
                    // Return basic result without token info
                    OperationResponse::success(json!({
                        "tba_address": tba_address,
                        "signer_address": signer_address,
                        "is_valid_signer": is_valid,
                        "chain_id": chain_id
                    }))
                }
            }
        }
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to check TBA ownership: {}",
            e
        ))),
    }
} 