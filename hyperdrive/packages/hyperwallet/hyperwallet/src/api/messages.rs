/// Centralized message dispatcher for all Hyperwallet operations
/// 
/// This module handles incoming HyperwalletMessage requests and routes them to 
/// the appropriate core business logic or external integrations.

use crate::state::HyperwalletState;
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletMessage, HyperwalletResponse, OperationError, HyperwalletResponseData,
};
use hyperware_process_lib::logging::{error, info};
use hyperware_process_lib::{Address, Response};

use crate::core;
use crate::integrations;

pub fn handle_process_message(
    source: &Address,
    body: Vec<u8>,
    state: &mut HyperwalletState,
) -> anyhow::Result<()> {
    let message: HyperwalletMessage = match serde_json::from_slice(&body) {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to parse process message from {}: {}", source, e);
            let error = OperationError::invalid_params(&format!("Invalid message format: {}", e));
            let response = HyperwalletResponse::<serde_json::Value>::error(error);
            Response::new()
                .body(serde_json::to_vec(&response)?)
                .send()?;
            return Ok(());
        }
    };

    info!(
        "Process request from {}: {:?}",
        source, message.operation_type()
    );

    let response = execute_message(message, source, state);

    Response::new()
        .body(serde_json::to_vec(&response)?)
        .send()?;

    Ok(())
}

pub fn execute_message(
    message: HyperwalletMessage,
    source: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse<HyperwalletResponseData> {
    match message {
        // Session Management (core/session.rs)
        HyperwalletMessage::Handshake(req) => core::session::handle_handshake_step(req, source, state).map(HyperwalletResponseData::Handshake),
        HyperwalletMessage::UnlockWallet(req) => core::session::handle_unlock_wallet(req, source, state),

        // Wallet Management (core/wallet_lifecycle.rs)
        HyperwalletMessage::CreateWallet(req) => core::wallet_lifecycle::create_wallet(req, source, state),
        HyperwalletMessage::ImportWallet(req) => core::wallet_lifecycle::import_wallet(req, source, state),
        HyperwalletMessage::DeleteWallet(req) => core::wallet_lifecycle::delete_wallet(req, source, state),
        HyperwalletMessage::RenameWallet(req) => core::wallet_lifecycle::rename_wallet(req, source, state),
        HyperwalletMessage::ExportWallet(req) => core::wallet_lifecycle::export_wallet(req, source, state),

        // Queries (core/wallet_queries.rs)
        HyperwalletMessage::ListWallets(req) => core::wallet_queries::list_wallets(req, source, state),
        HyperwalletMessage::GetWalletInfo(req) => core::wallet_queries::get_wallet_info(req, source, state),
        HyperwalletMessage::GetBalance(req) => core::wallet_queries::get_balance(req, source, state),
        HyperwalletMessage::GetTokenBalance(req) => core::wallet_queries::get_token_balance(req, source, state),

        // Transactions (core/transactions.rs)
        HyperwalletMessage::SendEth(req) => core::transactions::send_eth(req, source, state),
        HyperwalletMessage::SendToken(req) => core::transactions::send_token(req, source, state),

        // Account Abstraction (integrations/erc4337_operations.rs)
        HyperwalletMessage::BuildAndSignUserOperationForPayment(req) => integrations::erc4337_operations::build_and_sign_user_operation_for_payment(req, source, state),
        // TODO: Update to use typed approach when integrations are migrated
            // Convert OperationResponse to HyperwalletResponse
        HyperwalletMessage::SubmitUserOperation(req) => integrations::erc4337_operations::submit_user_operation(req),
        HyperwalletMessage::GetUserOperationReceipt(req) => integrations::erc4337_operations::get_user_operation_receipt(req),

        // Hypermap Operations (integrations/hypermap.rs)
        //HyperwalletMessage::CheckTbaOwnership(req) => integrations::hypermap::check_tba_ownership(req, source),
        // TODO: Update to use typed approach when integrations are migrated. These should just be added from process_lib.
        //HyperwalletMessage::CreateNote(_req) => { 
        //    HyperwalletResponse::error(OperationError::invalid_params(
        //        "Hypermap operations temporarily disabled during reorganization"
        //    ))
        //},

        //// Token Bound Account Operations (core/transactions.rs)
        //HyperwalletMessage::ExecuteViaTba(_req) => {
        //    HyperwalletResponse::error(OperationError::invalid_params(
        //        "TBA operations temporarily disabled during reorganization"
        //    ))
        //},

        _ => {
            HyperwalletResponse::error(OperationError::invalid_params(
                "Operation not yet implemented in new architecture"
            ))
        }
    }
} 