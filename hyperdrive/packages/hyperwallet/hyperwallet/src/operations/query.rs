/// Query operations for retrieving wallet and blockchain information

use crate::config::DEFAULT_CHAIN_ID;
use crate::operations::{OperationError, OperationResponse};
use crate::state::{HyperwalletState, KeyStorage};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet as wallet_lib;
use serde_json::json;

/// Get wallet information
pub fn get_wallet_info(
    wallet_id: Option<&str>,
    process_address: &str,
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

    match state.get_wallet(process_address, wallet_id) {
        Some(wallet) => OperationResponse::success(json!({
            "address": wallet.address,
            "name": wallet.name,
            "chain_id": wallet.chain_id,
            "created_at": wallet.created_at,
            "last_used": wallet.last_used,
            "encrypted": matches!(wallet.key_storage, KeyStorage::Encrypted(_)),
            "spending_limits": wallet.spending_limits
        })),
        None => OperationResponse::error(OperationError::wallet_not_found(wallet_id)),
    }
}

/// List wallets for the requesting process
pub fn list_wallets(process_address: &str, state: &HyperwalletState) -> OperationResponse {
    let wallets: Vec<_> = state
        .list_wallets(process_address)
        .into_iter()
        .map(|wallet| {
            json!({
                "address": wallet.address,
                "name": wallet.name,
                "chain_id": wallet.chain_id,
                "created_at": wallet.created_at,
                "encrypted": matches!(wallet.key_storage, KeyStorage::Encrypted(_))
            })
        })
        .collect();

    OperationResponse::success(json!({
        "process": process_address,
        "wallets": wallets,
        "total": wallets.len()
    }))
}

/// Get ETH balance for a wallet
pub fn get_balance(
    wallet_id: Option<&str>,
    process_address: &str,
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
    let chain_id = chain_id.unwrap_or(DEFAULT_CHAIN_ID);

    // Get the wallet
    let wallet = match state.get_wallet(process_address, wallet_id) {
        Some(w) => w,
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_id));
        }
    };

    // Use the wallet module's get_eth_balance function
    match wallet_lib::get_eth_balance(
        &wallet.address,
        chain_id,
        Provider::new(chain_id, 60000),
    ) {
        Ok(balance) => OperationResponse::success(json!({
            "address": wallet.address,
            "balance": balance.to_display_string(),
            "balance_wei": balance.as_wei().to_string(),
            "currency": "ETH",
            "chain_id": chain_id
        })),
        Err(e) => OperationResponse::error(OperationError::blockchain_error(&format!(
            "Failed to query balance: {}",
            e
        ))),
    }
} 