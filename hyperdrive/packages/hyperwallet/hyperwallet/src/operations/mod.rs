/// Operations module for handling all wallet operations

pub mod types;
pub mod process_management;
pub mod wallet_management;
pub mod ethereum;
pub mod token;
pub mod hypermap;
pub mod query;
pub mod account_abstraction;
pub mod handshake;

// Re-export commonly used types
pub use types::{
    Operation, OperationError, OperationRequest, OperationResponse,
    ProcessAuth,
};

use crate::state::HyperwalletState;
use hyperware_process_lib::logging::warn;

/// Execute a validated operation
pub fn execute_operation(
    request: OperationRequest,
    state: &mut HyperwalletState,
) -> OperationResponse {
    // Add request ID to response if provided
    let request_id = request.request_id.clone();
    
    let mut response = match request.operation {
        // Process Management Operations
        Operation::RegisterProcess => {
            warn!("RegisterProcess is deprecated. Please use the Handshake operation instead.");
            process_management::register_process(request.params, &request.auth.process_address, state)
        },
        Operation::UpdateSpendingLimits => process_management::update_spending_limits(request.params, &request.auth.process_address, state),
        Operation::Handshake => handshake::handle_handshake_step(request, state),
        Operation::UnlockWallet => handshake::handle_unlock_wallet(request, state),
        
        // Wallet Management Operations
        Operation::CreateWallet => wallet_management::create_wallet(request.params.clone(), &request, state),
        Operation::ImportWallet => wallet_management::import_wallet(request.params.clone(), &request, state),
        Operation::DeleteWallet => {
            match request.wallet_id.as_deref() {
                Some(wallet_id) => wallet_management::delete_wallet(wallet_id, &request, state),
                None => OperationResponse::error(OperationError::invalid_params("wallet_id required")),
            }
        },
        Operation::RenameWallet => {
            match request.wallet_id.as_deref() {
                Some(wallet_id) => wallet_management::rename_wallet(wallet_id, request.params.clone(), &request, state),
                None => OperationResponse::error(OperationError::invalid_params("wallet_id required")),
            }
        },
        Operation::ExportWallet => {
            match request.wallet_id.as_deref() {
                Some(wallet_id) => wallet_management::export_wallet(wallet_id, request.params.clone(), &request, state),
                None => OperationResponse::error(OperationError::invalid_params("wallet_id required")),
            }
        },
        Operation::GetWalletInfo => query::get_wallet_info(request.wallet_id.as_deref(), &request.auth.process_address, state),
        Operation::ListWallets => query::list_wallets(&request.auth.process_address, state),
        Operation::SetWalletLimits => {
            match request.wallet_id.as_deref() {
                Some(wallet_id) => wallet_management::set_wallet_limits(wallet_id, request.params.clone(), &request, state),
                None => OperationResponse::error(OperationError::invalid_params("wallet_id required")),
            }
        },
        
        // Query Operations
        Operation::GetBalance => query::get_balance(request.wallet_id.as_deref(), &request.auth.process_address, request.chain_id, state),
        Operation::GetTokenBalance => {
            let token_address = request.params.get("token_address").and_then(|v| v.as_str());
            token::get_token_balance(request.wallet_id.as_deref(), &request.auth.process_address, token_address, request.chain_id, state)
        },
        
        // Ethereum Operations
        Operation::SendEth => ethereum::send_eth(request, state),
        
        // Token Operations
        Operation::SendToken => token::send_token(request, state),
        Operation::ApproveToken => token::approve_token(request, state),
        
        // Hypermap Operations
        Operation::ResolveIdentity => {
            let entry_name = request.params.get("entry_name").and_then(|v| v.as_str());
            hypermap::resolve_identity(entry_name, request.chain_id, state)
        },
        Operation::CreateNote => hypermap::create_note(request, state),
        
        // TBA Operations
        Operation::ExecuteViaTba => hypermap::execute_via_tba(request, state),
        Operation::CheckTbaOwnership => {
            let tba_address = request.params.get("tba_address").and_then(|v| v.as_str());
            let signer_address = request.params.get("signer_address").and_then(|v| v.as_str());
            hypermap::check_tba_ownership(tba_address, signer_address, request.chain_id, state)
        },
        
        // ERC-4337 Account Abstraction Operations
        Operation::BuildAndSignUserOperationForPayment => account_abstraction::build_and_sign_user_operation_for_payment(request, state),
        Operation::SubmitUserOperation => account_abstraction::submit_user_operation(request, state),
        Operation::GetUserOperationReceipt => account_abstraction::get_user_operation_receipt(request, state),
        // these might be revived in some manner in the future, depending on need. They're currently not also up to date.
        //Operation::SignUserOperation => account_abstraction::sign_user_operation(request, state),
        //Operation::BuildUserOperation => account_abstraction::build_user_operation(request, state),
        //Operation::ConfigurePaymaster => account_abstraction::configure_paymaster(request, state),
        //Operation::EstimateUserOperationGas => account_abstraction::estimate_user_operation_gas(request, state),
        //Operation::BuildAndSignUserOperation => account_abstraction::build_and_sign_user_operation(request, state),
        
        // Unimplemented operations
        _ => OperationResponse::error(OperationError::internal_error(&format!(
            "Operation {:?} not yet implemented",
            request.operation
        ))),
    };
    
    // Add request ID if provided
    if let Some(id) = request_id {
        response = response.with_request_id(id);
    }
    
    response
} 