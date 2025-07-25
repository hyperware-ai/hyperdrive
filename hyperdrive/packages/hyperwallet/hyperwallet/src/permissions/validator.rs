/// Permission validation logic

use crate::operations::{Operation, OperationError, OperationRequest, OperationResponse};
use crate::permissions::operation_requires_wallet;
use crate::state::HyperwalletState;
use hyperware_process_lib::logging::{error, info};

pub struct PermissionValidator;

impl PermissionValidator {
    pub fn new() -> Self {
        Self
    }

    /// Execute operation with full permission validation
    pub fn execute_with_permissions(
        &self,
        request: OperationRequest,
        source_address: &str,
        state: &mut HyperwalletState,
    ) -> OperationResponse {
        // Special handling for RegisterProcess - doesn't require existing permissions
        if matches!(request.operation, Operation::RegisterProcess) {
            info!("Processing RegisterProcess from {}", source_address);
            return crate::operations::execute_operation(request, state);
        }

        // Get permissions for the source process
        let permissions = match state.get_permissions(source_address) {
            Some(perms) => perms,
            None => {
                error!("No permissions found for process: {}", source_address);
                return OperationResponse::error(OperationError::permission_denied(&format!(
                    "Process {} is not registered. Use RegisterProcess first.",
                    source_address
                )));
            }
        };

        // Validate the operation is allowed
        if !permissions.can_perform(&request.operation) {
            return OperationResponse::error(OperationError::permission_denied(&format!(
                "Operation {:?} is not allowed for process {}",
                request.operation, source_address
            )));
        }

        // For wallet operations, verify ownership
        if operation_requires_wallet(&request.operation) {
            if let Some(wallet_id) = &request.wallet_id {
                if !state.check_wallet_ownership(source_address, wallet_id) {
                    return OperationResponse::error(OperationError::permission_denied(&format!(
                        "Process {} does not own wallet '{}'",
                        source_address, wallet_id
                    )));
                }
            }
        }

        // Check spending limits for transaction operations
        if matches!(request.operation, Operation::SendEth | Operation::SendToken | Operation::ExecuteViaTba) {
            if let Some(limits) = &permissions.spending_limits {
                // Extract amount from params
                if let Some(amount) = request.params.get("amount").and_then(|v| v.as_str()) {
                    let is_eth = matches!(request.operation, Operation::SendEth);
                    
                    if let Err(e) = limits.check_transaction(amount, is_eth) {
                        return OperationResponse::error(
                            OperationError::spending_limit_exceeded(&amount, &e)
                        );
                    }
                }
            }
        }

        info!(
            "Executing operation {:?} for process {}",
            request.operation, source_address
        );

        // Clone operation type and amount for spending limit update after execution
        let operation_type = request.operation.clone();
        let amount_str = request.params.get("amount").and_then(|v| v.as_str()).map(|s| s.to_string());

        // Execute the operation
        let response = crate::operations::execute_operation(request, state);
        
        // Update spending limits if transaction was successful
        if response.success {
            if matches!(operation_type, Operation::SendEth | Operation::SendToken | Operation::ExecuteViaTba) {
                if let Some(perms) = state.process_permissions.get_mut(source_address) {
                    if let Some(limits) = &mut perms.spending_limits {
                        if let Some(amount) = amount_str {
                            let is_eth = matches!(operation_type, Operation::SendEth);
                            limits.record_spending(&amount, is_eth);
                            state.save();
                        }
                    }
                }
            }
        }
        
        response
    }
}
