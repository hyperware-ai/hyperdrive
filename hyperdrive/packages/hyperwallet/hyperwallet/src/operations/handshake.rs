/// Server-side handshake protocol implementation

use crate::operations::{OperationError, OperationRequest, OperationResponse};
use crate::permissions::{ProcessPermissions, SpendingLimits, UpdatableSetting};
use crate::state::HyperwalletState;
use hyperware_process_lib::hyperwallet_client::types::Operation;
use hyperware_process_lib::logging::{info, warn};
use serde::{Deserialize, Serialize};

/// Steps in the handshake protocol
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "step")]
pub enum HandshakeStep {
    ClientHello { 
        client_version: String, 
        client_name: String 
    },
    ServerWelcome { 
        server_version: String,
        supported_operations: Vec<Operation>,
        supported_chains: Vec<u64>,
        features: Vec<String>
    },
    Register { 
        required_operations: Vec<Operation>, 
        spending_limits: Option<SpendingLimits> 
    },
    Complete { 
        registered_permissions: ProcessPermissions,
        session_id: String 
    },
}

/// Server capabilities sent in ServerWelcome
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub version: String,
    pub supported_operations: Vec<Operation>,
    pub supported_chains: Vec<u64>,
    pub features: Vec<String>,
}

/// Session information returned to client
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub server_version: String,
    pub session_id: String,
    pub registered_permissions: ProcessPermissions,
}

/// Handshake-specific errors
#[derive(Debug)]
pub enum HandshakeError {
    VersionMismatch { client: String, server: String },
    OperationNotSupported { operation: Operation },
    Communication(anyhow::Error),
    ServerError(String),
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandshakeError::VersionMismatch { client, server } => {
                write!(f, "Version incompatibility: client {} vs server {}", client, server)
            }
            HandshakeError::OperationNotSupported { operation } => {
                write!(f, "Required operation not supported: {:?}", operation)
            }
            HandshakeError::Communication(err) => {
                write!(f, "Communication error: {}", err)
            }
            HandshakeError::ServerError(msg) => {
                write!(f, "Server error: {}", msg)
            }
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HandshakeError::Communication(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for HandshakeError {
    fn from(err: anyhow::Error) -> Self {
        HandshakeError::Communication(err)
    }
}

/// Main handshake handler - processes ClientHello and Register steps
pub fn handle_handshake_step(
    request: OperationRequest,
    state: &mut HyperwalletState,
) -> OperationResponse {
    // Clean up expired sessions first
    state.cleanup_expired_sessions();
    
    match serde_json::from_value::<HandshakeStep>(request.params) {
        Ok(HandshakeStep::ClientHello { client_version, client_name }) => {
            handle_client_hello(client_version, client_name, state)
        },
        Ok(HandshakeStep::Register { required_operations, spending_limits }) => {
            handle_register(required_operations, spending_limits, &request.auth.process_address, state)
        },
        _ => OperationResponse::error(OperationError::invalid_params("Invalid handshake step")),
    }
}

/// Handle ClientHello step - respond with server capabilities
fn handle_client_hello(
    client_version: String,
    client_name: String,
    _state: &HyperwalletState,
) -> OperationResponse {
    info!("Received ClientHello from {} (version {})", client_name, client_version);
    
    // Version compatibility check
    if !is_compatible_version(&client_version) {
        warn!("Version incompatibility: client {} vs server {}", client_version, env!("CARGO_PKG_VERSION"));
        return OperationResponse::error(OperationError::invalid_params(&format!(
            "Version incompatibility: client {} vs server {}",
            client_version, 
            env!("CARGO_PKG_VERSION")
        )));
    }

    // Build server capabilities using the shared Operation enum
    let supported_operations = get_all_supported_operations();
    let capabilities = ServerCapabilities {
        version: env!("CARGO_PKG_VERSION").to_string(),
        supported_operations: supported_operations.clone(),
        supported_chains: vec![1, 8453, 42161], // Mainnet, Base, Arbitrum
        features: vec!["spending_limits".to_string(), "session_management".to_string()],
    };

    info!("Sending ServerWelcome with {} operations and {} chains", 
          capabilities.supported_operations.len(), 
          capabilities.supported_chains.len());

    OperationResponse::success(serde_json::json!({
        "step": "ServerWelcome",
        "server_version": capabilities.version,
        "supported_operations": capabilities.supported_operations,
        "supported_chains": capabilities.supported_chains,
        "features": capabilities.features
    }))
}

/// Handle Register step - create/update permissions and establish session
fn handle_register(
    required_operations: Vec<Operation>,
    spending_limits: Option<SpendingLimits>,
    process_address: &str,
    state: &mut HyperwalletState,
) -> OperationResponse {
    info!("Processing Register for {} with {} operations", process_address, required_operations.len());
    
    // Validate that all requested operations are supported
    let supported_ops = get_all_supported_operations();
    for op in &required_operations {
        if !supported_ops.contains(op) {
            return OperationResponse::error(OperationError::invalid_params(&format!(
                "Operation {:?} is not supported by this server", op
            )));
        }
    }
    
    // Create new permissions (declarative model - overwrites existing for seamless updates)
    let mut permissions = ProcessPermissions::new(process_address.to_string(), required_operations.clone());
    
    if let Some(limits) = spending_limits.clone() {
        permissions = permissions.with_spending_limits(limits);
        permissions = permissions.with_updatable_settings(vec![UpdatableSetting::SpendingLimits]);
    }

    // Always overwrite - this enables seamless permission updates on app version changes
    let was_update = state.get_permissions(process_address).is_some();
    state.set_permissions(process_address.to_string(), permissions.clone());
    
    // Create a new session for the client (1500 minute default)
    // Create a new session for the client (infinite duration by default)
    let session_id = state.create_session(process_address.to_string(), 0);
    
    info!("{} process {} with {} operations, created session {}", 
          if was_update { "Updated" } else { "Registered" },
          process_address, 
          permissions.allowed_operations.len(),
          session_id);

    OperationResponse::success(serde_json::json!({
        "step": "Complete",
        "registered_permissions": permissions,
        "session_id": session_id
    }))
}

/// Get all operations supported by this server
fn get_all_supported_operations() -> Vec<Operation> {
    use Operation::*;
    
    vec![
        // Process Management
        RegisterProcess,
        UpdateSpendingLimits,
        Handshake,
        UnlockWallet,
        
        // Wallet Management
        CreateWallet,
        ImportWallet,
        DeleteWallet,
        RenameWallet,
        ExportWallet,
        EncryptWallet,
        DecryptWallet,
        GetWalletInfo,
        ListWallets,
        SetWalletLimits,
        
        // Ethereum Operations
        SendEth,
        SendToken,
        ApproveToken,
        CallContract,
        SignTransaction,
        SignMessage,
        
        // TBA Operations
        ExecuteViaTba,
        CheckTbaOwnership,
        SetupTbaDelegation,
        
        // ERC-4337 Operations
        BuildUserOperation,
        SignUserOperation,
        BuildAndSignUserOperation,
        BuildAndSignUserOperationForPayment,
        SubmitUserOperation,
        EstimateUserOperationGas,
        GetUserOperationReceipt,
        ConfigurePaymaster,
        
        // Hypermap Operations
        ResolveIdentity,
        CreateNote,
        ReadNote,
        SetupDelegation,
        VerifyDelegation,
        MintEntry,
        
        // Query Operations
        GetBalance,
        GetTokenBalance,
        GetTransactionHistory,
        EstimateGas,
        GetGasPrice,
        GetTransactionReceipt,
        
        // Advanced Operations
        BatchOperations,
        ScheduleOperation,
        CancelOperation,
    ]
}

/// Check version compatibility
fn is_compatible_version(client_version: &str) -> bool {
    // For now, accept all versions but log warnings for major differences
    // TODO: Implement proper semantic versioning compatibility
    if client_version != env!("CARGO_PKG_VERSION") {
        warn!("Client version {} differs from server version {}", 
              client_version, env!("CARGO_PKG_VERSION"));
    }
    true
}

/// Handle UnlockWallet operation - cache decrypted signer for session
pub fn handle_unlock_wallet(
    request: OperationRequest,
    state: &mut HyperwalletState,
) -> OperationResponse {
    // Clean up expired sessions first
    state.cleanup_expired_sessions();
    
    let process_address = &request.auth.process_address;
    
    // Extract session_id and wallet parameters
    let session_id = match request.params.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "session_id is required for UnlockWallet"
            ));
        }
    };
    
    let wallet_identifier = match request.params.get("wallet_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "wallet_id is required for UnlockWallet"
            ));
        }
    };
    
    let password = match request.params.get("password").and_then(|v| v.as_str()) {
        Some(pwd) => pwd,
        None => {
            return OperationResponse::error(OperationError::invalid_params(
                "password is required for UnlockWallet"
            ));
        }
    };
    
    // Validate session belongs to this process (don't hold references)
    {
        match state.validate_session(&session_id.to_string()) {
            Some(session) if session.process_address == *process_address => {},
            Some(_) => {
                return OperationResponse::error(OperationError::permission_denied(
                    "Session does not belong to this process"
                ));
            }
            None => {
                return OperationResponse::error(OperationError::invalid_params(
                    "Invalid or expired session_id"
                ));
            }
        }
    }
    
    // Get the wallet (clone needed data to avoid borrowing issues)
    let (wallet_address, key_storage) = match state.get_wallet(process_address, wallet_identifier) {
        Some(wallet) => (wallet.address.clone(), wallet.key_storage.clone()),
        None => {
            return OperationResponse::error(OperationError::wallet_not_found(wallet_identifier));
        }
    };
    
    // Decrypt the wallet with the provided password
    match &key_storage {
        crate::state::KeyStorage::Encrypted(encrypted_data) => {
            match hyperware_process_lib::signer::LocalSigner::decrypt(encrypted_data, password) {
                Ok(signer) => {
                    // Cache the decrypted signer
                    state.active_signers.insert(
                        (process_address.to_string(), wallet_address.clone()), 
                        signer
                    );
                    
                    // Mark wallet as unlocked in the session
                    if let Err(e) = state.add_unlocked_wallet(&session_id.to_string(), wallet_address.clone()) {
                        return OperationResponse::error(OperationError::internal_error(&e));
                    }
                    
                    info!("Unlocked wallet {} for session {}", wallet_address, session_id);
                    
                    OperationResponse::success(serde_json::json!({
                        "wallet_address": wallet_address,
                        "unlocked": true,
                        "session_id": session_id
                    }))
                }
                Err(_) => {
                    OperationResponse::error(OperationError::decryption_failed())
                }
            }
        }
        crate::state::KeyStorage::Decrypted(_) => {
            // Wallet is already decrypted, just mark it as unlocked in the session
            if let Err(e) = state.add_unlocked_wallet(&session_id.to_string(), wallet_address.clone()) {
                return OperationResponse::error(OperationError::internal_error(&e));
            }
            
            info!("Wallet {} was already unlocked, added to session {}", wallet_address, session_id);
            
            OperationResponse::success(serde_json::json!({
                "wallet_address": wallet_address,
                "unlocked": true,
                "session_id": session_id
            }))
        }
    }
} 