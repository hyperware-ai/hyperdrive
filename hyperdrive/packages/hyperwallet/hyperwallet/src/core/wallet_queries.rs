/// Read-only query operations
/// 
/// This module handles all read-only data retrieval:
/// - Wallet information and balances
/// - Transaction history
/// - Token balances and details

use crate::config::DEFAULT_CHAIN_ID;
use crate::state::HyperwalletState;
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletResponse, HyperwalletResponseData, OperationError, SessionId,
    GetBalanceRequest, GetBalanceResponse, GetWalletInfoRequest, GetWalletInfoResponse,
    GetTokenBalanceRequest, GetTokenBalanceResponse, ListWalletsResponse,
    Balance, Wallet
};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::wallet;
use hyperware_process_lib::Address;

pub fn get_balance(
    req: GetBalanceRequest,
    _session_id: &SessionId,
    source: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    let chain_id = DEFAULT_CHAIN_ID;

    let wallet = match state.get_wallet(source, &req.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &req.wallet_id)));
        }
    };

    let provider = Provider::new(chain_id, 60000);

    match wallet::get_eth_balance(&wallet.address, chain_id, provider) {
        Ok(balance) => {
            HyperwalletResponse::success(HyperwalletResponseData::GetBalance(GetBalanceResponse {
                wallet_id: wallet.address.clone(),
                balance: Balance {
                    formatted: balance.to_string(),
                    raw: balance.to_string(),
                },
                chain_id,
            }))
        },
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to get balance: {}",
            e
        ))),
    }
}

pub fn list_wallets(
    _session_id: &SessionId,
    address: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    let wallets: Vec<Wallet> = state
        .list_wallets(address)
        .into_iter()
        .map(|wallet| {
            Wallet {
                address: wallet.address.clone(),
                name: wallet.name.clone(),
                chain_id: wallet.chain_id,
                encrypted: matches!(wallet.key_storage, crate::state::KeyStorage::Encrypted(_)),
                created_at: Some(wallet.created_at.to_rfc3339()),
                last_used: wallet.last_used.map(|dt| dt.to_rfc3339()),
                spending_limits: None,
            }
        })
        .collect();

    let wallet_count = wallets.len();
    HyperwalletResponse::success(HyperwalletResponseData::ListWallets(ListWalletsResponse {
        process: address.to_string(),
        wallets,
        total: wallet_count as u64,
    }))
}

pub fn get_wallet_info(
    req: GetWalletInfoRequest,
    _session_id: &SessionId,
    address: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    let wallet = match state.get_wallet(address, &req.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &req.wallet_id)));
        }
    };

    HyperwalletResponse::success(HyperwalletResponseData::GetWalletInfo(GetWalletInfoResponse {
        wallet_id: wallet.address.clone(),
        address: wallet.address.clone(),
        name: wallet.name.clone().unwrap_or_else(|| "Unnamed Wallet".to_string()),
        chain_id: wallet.chain_id,
        is_locked: matches!(wallet.key_storage, crate::state::KeyStorage::Encrypted(_)),
    }))
}

pub fn get_token_balance(
    req: GetTokenBalanceRequest,
    _session_id: &SessionId,
    address: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    let chain_id = DEFAULT_CHAIN_ID;

    // Get the wallet
    let wallet = match state.get_wallet(address, &req.wallet_id) {
        Some(w) => w,
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!("Wallet not found: {}", &req.wallet_id)));
        }
    };

    // Get provider for the chain
    let provider = Provider::new(chain_id, 60000);

    // Get token details
    match wallet::get_token_details(&req.token_address, &wallet.address, &provider) {
        Ok(details) => {
            HyperwalletResponse::success(HyperwalletResponseData::GetTokenBalance(GetTokenBalanceResponse {
                formatted: Some(details.formatted_balance),
                balance: details.balance,
                decimals: Some(details.decimals),
            }))
        },
        Err(e) => HyperwalletResponse::error(OperationError::internal_error(&format!(
            "Failed to get token balance: {}",
            e
        ))),
    }
} 