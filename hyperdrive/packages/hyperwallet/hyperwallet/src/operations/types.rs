use serde::{Deserialize, Serialize};

// Import Operation enum from the standard library - this is the core shared type
pub use hyperware_process_lib::hyperwallet_client::types::Operation;

// Keep our local implementation of request/response types for now
// These provide the helper methods that the server needs

/// Request wrapper for operations sent to hyperwallet service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRequest {
    /// The operation to perform
    pub operation: Operation,
    
    /// Operation-specific parameters as JSON
    pub params: serde_json::Value,
    
    /// Optional wallet ID (if operation targets specific wallet)
    pub wallet_id: Option<String>,
    
    /// Optional chain ID (if operation is chain-specific)
    pub chain_id: Option<u64>,
    
    /// Process authentication information
    pub auth: ProcessAuth,
    
    /// Request metadata
    pub request_id: Option<String>,
    pub timestamp: u64,
}

/// Process authentication for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAuth {
    /// The calling process address (e.g., "operator:operator:alice.hypr")
    pub process_address: String,
    
    /// Optional signature for request validation
    pub signature: Option<Vec<u8>>,
}

/// Response wrapper for operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResponse {
    /// Whether operation succeeded
    pub success: bool,
    
    /// Operation result data (if successful)
    pub data: Option<serde_json::Value>,
    
    /// Error information (if failed)
    pub error: Option<OperationError>,
    
    /// Response metadata
    pub request_id: Option<String>,
    pub timestamp: u64,
}

/// Error information for failed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationError {
    /// Error code
    pub code: ErrorCode,
    
    /// Human-readable error message
    pub message: String,
    
    /// Additional error details
    pub details: Option<serde_json::Value>,
}

/// Standardized error codes for the wallet service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCode {
    PermissionDenied,
    WalletNotFound,
    InsufficientFunds,
    InvalidOperation,
    InvalidParams,
    RateLimitExceeded,
    SpendingLimitExceeded,
    ChainNotAllowed,
    ContractNotWhitelisted,
    BlockchainError,
    InternalError,
    AuthenticationFailed,
    WalletLocked,
    OperationNotSupported,
}

impl OperationError {
    pub fn permission_denied(message: &str) -> Self {
        Self {
            code: ErrorCode::PermissionDenied,
            message: message.to_string(),
            details: None,
        }
    }
    
    pub fn wallet_not_found(wallet_id: &str) -> Self {
        Self {
            code: ErrorCode::WalletNotFound,
            message: format!("Wallet '{}' not found", wallet_id),
            details: Some(serde_json::json!({ "wallet_id": wallet_id })),
        }
    }
    
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: ErrorCode::InvalidParams,
            message: message.to_string(),
            details: None,
        }
    }
    
    pub fn blockchain_error(message: &str) -> Self {
        Self {
            code: ErrorCode::BlockchainError,
            message: message.to_string(),
            details: None,
        }
    }
    
    pub fn internal_error(message: &str) -> Self {
        Self {
            code: ErrorCode::InternalError,
            message: message.to_string(),
            details: None,
        }
    }
    
    pub fn authentication_failed(message: &str) -> Self {
        Self {
            code: ErrorCode::AuthenticationFailed,
            message: message.to_string(),
            details: None,
        }
    }
    
    pub fn rate_limit_exceeded(limit: u32, window: &str) -> Self {
        Self {
            code: ErrorCode::RateLimitExceeded,
            message: format!("Rate limit exceeded: {} operations per {}", limit, window),
            details: Some(serde_json::json!({ "limit": limit, "window": window })),
        }
    }
    
    pub fn spending_limit_exceeded(limit: &str, period: &str) -> Self {
        Self {
            code: ErrorCode::SpendingLimitExceeded,
            message: format!("Spending limit exceeded: {} per {}", limit, period),
            details: Some(serde_json::json!({ "limit": limit, "period": period })),
        }
    }
    
    pub fn chain_not_allowed(chain_id: u64) -> Self {
        Self {
            code: ErrorCode::ChainNotAllowed,
            message: format!("Chain ID {} is not allowed for this operation", chain_id),
            details: Some(serde_json::json!({ "chain_id": chain_id })),
        }
    }
    
    pub fn wallet_locked(wallet_id: &str) -> Self {
        Self {
            code: ErrorCode::WalletLocked,
            message: format!("Wallet '{}' is locked and requires password", wallet_id),
            details: Some(serde_json::json!({ "wallet_id": wallet_id })),
        }
    }
    
    pub fn password_required() -> Self {
        Self {
            code: ErrorCode::AuthenticationFailed,
            message: "Password required for encrypted wallet".to_string(),
            details: None,
        }
    }
    
    pub fn decryption_failed() -> Self {
        Self {
            code: ErrorCode::AuthenticationFailed,
            message: "Failed to decrypt wallet - incorrect password".to_string(),
            details: None,
        }
    }
    
    pub fn operation_not_supported(message: &str) -> Self {
        Self {
            code: ErrorCode::OperationNotSupported,
            message: message.to_string(),
            details: None,
        }
    }
}

impl OperationRequest {
    pub fn new(operation: Operation, params: serde_json::Value) -> Self {
        Self {
            operation,
            params,
            wallet_id: None,
            chain_id: None,
            auth: ProcessAuth {
                process_address: String::new(), // Will be filled by handler
                signature: None,
            },
            request_id: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn with_wallet(mut self, wallet_id: String) -> Self {
        self.wallet_id = Some(wallet_id);
        self
    }
    
    pub fn with_chain(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }
    
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

impl OperationResponse {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            request_id: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn error(error: OperationError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            request_id: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}