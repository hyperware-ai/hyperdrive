/// Centralized message dispatcher for all Hyperwallet operations
///
/// This module handles incoming HyperwalletMessage requests and routes them to
/// the appropriate core business logic or external integrations.
use crate::state::HyperwalletState;
use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletMessage, HyperwalletRequest, HyperwalletResponse, OperationError,
};
use hyperware_process_lib::logging::{error, info, warn};
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
            let response = HyperwalletResponse::error(error);
            Response::new()
                .body(serde_json::to_vec(&response)?)
                .send()?;
            return Ok(());
        }
    };

    info!(
        "Process request from {}: {:?}",
        source,
        message.operation_type()
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
) -> HyperwalletResponse {
    let session_id = &message.session_id;

    match message.request {
        // Session Management (core/session.rs)
        HyperwalletRequest::Handshake(req) => {
            core::session::handle_handshake_step(req, source, state)
        }
        HyperwalletRequest::UnlockWallet(req) => {
            core::session::handle_unlock_wallet(req, session_id, source, state)
        }

        // Wallet Management (core/wallet_lifecycle.rs)
        HyperwalletRequest::CreateWallet(req) => {
            core::wallet_lifecycle::create_wallet(req, session_id, source, state)
        }
        HyperwalletRequest::ImportWallet(req) => {
            core::wallet_lifecycle::import_wallet(req, session_id, source, state)
        }
        HyperwalletRequest::DeleteWallet(req) => {
            core::wallet_lifecycle::delete_wallet(req, session_id, source, state)
        }
        HyperwalletRequest::RenameWallet(req) => {
            core::wallet_lifecycle::rename_wallet(req, session_id, source, state)
        }
        HyperwalletRequest::ExportWallet(req) => {
            core::wallet_lifecycle::export_wallet(req, session_id, source, state)
        }
        HyperwalletRequest::SetWalletLimits(req) => {
            core::wallet_lifecycle::set_wallet_limits(req, session_id, source, state)
        }

        // Queries (core/wallet_queries.rs)
        HyperwalletRequest::ListWallets => {
            core::wallet_queries::list_wallets(session_id, source, state)
        }
        HyperwalletRequest::GetWalletInfo(req) => {
            core::wallet_queries::get_wallet_info(req, session_id, source, state)
        }
        HyperwalletRequest::GetBalance(req) => {
            core::wallet_queries::get_balance(req, session_id, source, state)
        }
        HyperwalletRequest::GetTokenBalance(req) => {
            core::wallet_queries::get_token_balance(req, session_id, source, state)
        }

        // Transactions (core/transactions.rs)
        HyperwalletRequest::SendEth(req) => {
            core::transactions::send_eth(req, session_id, source, state)
        }
        HyperwalletRequest::SendToken(req) => {
            core::transactions::send_token(req, session_id, source, state)
        }

        // Account Abstraction (integrations/erc4337_operations.rs)
        HyperwalletRequest::BuildAndSignUserOperationForPayment(req) => {
            integrations::erc4337_operations::build_and_sign_user_operation_for_payment(
                req, session_id, source, state,
            )
        }
        // TODO: Update to use typed approach when integrations are migrated
        // Convert OperationResponse to HyperwalletResponse
        HyperwalletRequest::SubmitUserOperation(req) => {
            integrations::erc4337_operations::submit_user_operation(req, session_id)
        }
        HyperwalletRequest::GetUserOperationReceipt(req) => {
            integrations::erc4337_operations::get_user_operation_receipt(req, session_id)
        }

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
            warn!(
                "Received request for unsupported operation from {}",
                source
            );
            let op_name = format!("{:?}", message.operation_type());
            HyperwalletResponse::error(OperationError::operation_not_supported(&op_name))
        }
    }
}
