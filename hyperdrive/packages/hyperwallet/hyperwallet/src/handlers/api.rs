/// API endpoint handlers for HTTP requests

use crate::config::{SERVICE_NAME, SERVICE_VERSION};
use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::permissions::validator::PermissionValidator;
use crate::state::HyperwalletState;
use hyperware_process_lib::Address;
use serde_json::json;

/// Handle operation POST request
pub fn handle_operation_request(
    body: &[u8],
    source: &Address,
    state: &mut HyperwalletState,
) -> String {
    // Parse the operation request
    let mut operation_request: OperationRequest = match serde_json::from_slice(body) {
        Ok(req) => req,
        Err(e) => {
            let error = OperationError::invalid_params(&format!("Invalid request format: {}", e));
            return serde_json::to_string(&OperationResponse::error(error)).unwrap();
        }
    };

    // Set the process address from the source
    operation_request.auth.process_address = source.to_string();

    // Execute with permission validation
    let validator = PermissionValidator::new();
    let response = validator.execute_with_permissions(
        operation_request,
        &source.to_string(),
        state,
    );
    
    serde_json::to_string(&response).unwrap()
}

/// Handle GET /api/status request
pub fn handle_status_request(state: &HyperwalletState) -> String {
    // Count total wallets across all processes
    let total_wallets: usize = state.wallets_by_process
        .values()
        .map(|wallets| wallets.len())
        .sum();

    json!({
        "service": SERVICE_NAME,
        "version": SERVICE_VERSION,
        "status": "running",
        "processes_count": state.wallets_by_process.len(),
        "wallets_count": total_wallets,
        "permissions_count": state.process_permissions.len(),
        "chains_count": state.chains.len(),
        "initialized_at": state.initialized_at,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
    .to_string()
}

/// Handle GET /api/wallets request (admin view - all wallets)
pub fn handle_wallets_request(state: &HyperwalletState, source: &Address) -> String {
    let source_str = source.to_string();
    
    // Check if this is the hyperwallet admin
    if source_str.starts_with("hyperwallet:hyperwallet:") {
        // Admin view - show all wallets grouped by process
        let processes: Vec<_> = state.wallets_by_process
            .iter()
            .map(|(process, wallets)| {
                let wallet_list: Vec<_> = wallets.values()
                    .map(|wallet| json!({
                        "address": wallet.address,
                        "name": wallet.name,
                        "chain_id": wallet.chain_id,
                        "created_at": wallet.created_at,
                        "last_used": wallet.last_used
                    }))
                    .collect();
                
                json!({
                    "process": process,
                    "wallets": wallet_list,
                    "count": wallet_list.len()
                })
            })
            .collect();

        json!({
            "processes": processes,
            "total_processes": processes.len()
        })
        .to_string()
    } else {
        // Regular process - show only their wallets
        let wallets = state.list_wallets(&source_str);
        let wallet_list: Vec<_> = wallets
            .into_iter()
            .map(|wallet| json!({
                "address": wallet.address,
                "name": wallet.name,
                "chain_id": wallet.chain_id,
                "created_at": wallet.created_at,
                "last_used": wallet.last_used
            }))
            .collect();

        json!({
            "process": source_str,
            "wallets": wallet_list,
            "count": wallet_list.len()
        })
        .to_string()
    }
}

/// Handle GET /api/permissions request
pub fn handle_permissions_request(state: &HyperwalletState) -> String {
    let permissions: Vec<_> = state
        .process_permissions
        .iter()
        .map(|(process, perms)| {
            json!({
                "process": process,
                "permissions": perms
            })
        })
        .collect();

    json!({
        "permissions": permissions,
        "total": permissions.len()
    })
    .to_string()
}

/// Handle 404 Not Found
pub fn handle_not_found(path: &str, method: &str) -> String {
    json!({
        "error": "Not found",
        "path": path,
        "method": method
    })
    .to_string()
}
