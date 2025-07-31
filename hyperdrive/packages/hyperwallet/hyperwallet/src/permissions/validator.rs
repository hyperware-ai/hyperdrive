/// Permission validation logic

use hyperware_process_lib::hyperwallet_client::types::{OperationError, HyperwalletMessage, HyperwalletResponse, HyperwalletResponseData};
use hyperware_process_lib::hyperwallet_client::types::Operation;
use hyperware_process_lib::Address;
use crate::permissions::operation_requires_wallet;
use crate::state::HyperwalletState;
use hyperware_process_lib::logging::{error, info};

pub struct PermissionValidator;

impl PermissionValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_with_permissions(
        &self,
        message: HyperwalletMessage,
        address: &Address,
        state: &mut HyperwalletState,
    ) -> HyperwalletResponse<HyperwalletResponseData> {
        
        // Special handling for operations that don't require existing permissions. might be unsafe?
        if matches!(message, HyperwalletMessage::Handshake(_)) {
            info!("Processing Handshake from {}", address);
            return crate::api::messages::execute_message(message, address, state);
        }

        let operation = message.operation_type();

        let permissions = match state.get_permissions(address) {
            Some(perms) => perms,
            None => {
                error!("No permissions found for process: {}", address);
                return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                    "Process {} is not registered. Use the Handshake operation to register first.",
                    address
                )));
            }
        };

        if !permissions.allowed_operations.contains(&operation) {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Operation {:?} is not allowed for process {}",
                operation, address
            )));
        }

        // Note: In the new architecture, wallet ownership is typically managed via session_id
        // TODO: Implement proper wallet ownership checking based on session or explicit wallet_id
        if operation_requires_wallet(&operation) {
            // This is a placeholder - in the new architecture, we might need to extract
            // wallet information from the typed request structs or session context
            info!("Wallet operation {:?} - ownership check needed", operation);
        }

        // TODO: Implement spending limit checking for typed request structs
        // This will require extracting amount from the specific request types
        if matches!(operation, Operation::SendEth | Operation::SendToken | Operation::ExecuteViaTba) {
            info!("Transaction operation {:?} - spending limit check needed", operation);
        }

        info!(
            "Executing operation {:?} for process {}",
            operation, address
        );

        let response = crate::api::messages::execute_message(message, address, state);
        
        // TODO: Implement spending limit updates for successful transactions
        if response.success {
            if matches!(operation, Operation::SendEth | Operation::SendToken | Operation::ExecuteViaTba) {
                info!("Transaction successful, spending limits update needed");
            }
        }
        
        response
    }
}
