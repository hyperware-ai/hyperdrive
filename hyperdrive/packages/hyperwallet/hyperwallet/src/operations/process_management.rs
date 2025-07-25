/// Process management operations - registration and settings

use crate::operations::{OperationError, OperationResponse, Operation};
use crate::permissions::{ProcessPermissions, SpendingLimits, UpdatableSetting};
use crate::state::HyperwalletState;
use hyperware_process_lib::logging::{info, warn};
use serde_json::Value;

// should the regiter process return the permissions if the process is already registered?
// or should it return an error, like it does now?
//
/// Register a process with the hyperwallet service
pub fn register_process(
    params: Value,
    process_address: &str,
    state: &mut HyperwalletState,
) -> OperationResponse {
    // Check if process is already registered
    if let Some(existing_perms) = state.get_permissions(process_address) {
        warn!("Process {} is already registered, returning existing permissions", process_address);
        return OperationResponse::success(serde_json::json!({
            "success": true,
            "already_registered": true,
            "process": process_address,
            "operations_count": existing_perms.allowed_operations.len(),
            "has_spending_limits": existing_perms.spending_limits.is_some(),
            "permissions": existing_perms
        }));
    }
    
    // Parse requested operations
    let operations = match params.get("operations") {
        Some(Value::Array(ops)) => {
            let mut parsed_ops = Vec::new();
            for op in ops {
                if let Value::String(op_str) = op {
                    match serde_json::from_value::<Operation>(Value::String(op_str.clone())) {
                        Ok(operation) => parsed_ops.push(operation),
                        Err(_) => {
                            return OperationResponse::error(OperationError::invalid_params(
                                &format!("Invalid operation: {}", op_str)
                            ));
                        }
                    }
                } else {
                    return OperationResponse::error(OperationError::invalid_params(
                        "Operations must be an array of strings"
                    ));
                }
            }
            parsed_ops
        }
        _ => {
            return OperationResponse::error(OperationError::invalid_params(
                "operations field is required and must be an array"
            ));
        }
    };
    
    // Create permissions
    let mut permissions = ProcessPermissions::new(process_address.to_string(), operations);
    
    // Parse optional spending limits
    if let Some(limits_value) = params.get("spending_limits") {
        let limits = SpendingLimits {
            per_tx_eth: limits_value.get("per_tx_eth")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            daily_eth: limits_value.get("daily_eth")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            per_tx_usdc: limits_value.get("per_tx_usdc")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            daily_usdc: limits_value.get("daily_usdc")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            daily_reset_at: chrono::Utc::now().timestamp() as u64,
            spent_today_eth: "0".to_string(),
            spent_today_usdc: "0".to_string(),
        };
        
        permissions = permissions.with_spending_limits(limits);
        
        // If spending limits are set, allow the process to update them
        permissions = permissions.with_updatable_settings(vec![UpdatableSetting::SpendingLimits]);
    }
    
    // Save permissions
    state.set_permissions(process_address.to_string(), permissions);
    
    info!("Registered process {} with {} operations", 
        process_address, 
        state.get_permissions(process_address).unwrap().allowed_operations.len()
    );
    
    OperationResponse::success(serde_json::json!({
        "success": true,
        "process": process_address,
        "operations_count": state.get_permissions(process_address).unwrap().allowed_operations.len(),
        "has_spending_limits": state.get_permissions(process_address).unwrap().spending_limits.is_some()
    }))
}

/// Update spending limits for a process
pub fn update_spending_limits(
    params: Value,
    process_address: &str,
    state: &mut HyperwalletState,
) -> OperationResponse {
    // Check if process exists and can update spending limits
    let can_update = match state.get_permissions(process_address) {
        Some(perms) => perms.can_update(&UpdatableSetting::SpendingLimits),
        None => {
            return OperationResponse::error(OperationError::permission_denied(
                "Process is not registered"
            ));
        }
    };
    
    if !can_update {
        return OperationResponse::error(OperationError::permission_denied(
            "Process is not allowed to update spending limits"
        ));
    }
    
    // Get the new limits
    let new_limits = match params.get("spending_limits") {
        Some(limits_value) => {
            // Get current limits or default
            let mut limits = state.get_permissions(process_address)
                .and_then(|p| p.spending_limits.clone())
                .unwrap_or_default();
            
            // Update only provided fields
            if let Some(v) = limits_value.get("per_tx_eth").and_then(|v| v.as_str()) {
                limits.per_tx_eth = Some(v.to_string());
            }
            if let Some(v) = limits_value.get("daily_eth").and_then(|v| v.as_str()) {
                limits.daily_eth = Some(v.to_string());
            }
            if let Some(v) = limits_value.get("per_tx_usdc").and_then(|v| v.as_str()) {
                limits.per_tx_usdc = Some(v.to_string());
            }
            if let Some(v) = limits_value.get("daily_usdc").and_then(|v| v.as_str()) {
                limits.daily_usdc = Some(v.to_string());
            }
            
            limits
        }
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "spending_limits field is required"
            ));
        }
    };
    
    // Update permissions
    if let Some(permissions) = state.process_permissions.get_mut(process_address) {
        permissions.spending_limits = Some(new_limits.clone());
        state.save();
        
        info!("Updated spending limits for process {}", process_address);
        
        OperationResponse::success(serde_json::json!({
            "success": true,
            "process": process_address,
            "spending_limits": new_limits
        }))
    } else {
        OperationResponse::error(OperationError::internal_error("Failed to update permissions"))
    }
} 