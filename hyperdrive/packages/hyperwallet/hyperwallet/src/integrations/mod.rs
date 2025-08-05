/// External service integration modules
/// 
/// This module contains integrations with external services and protocols

pub mod erc4337_bundler;
pub mod erc4337_operations;
pub mod hypermap;

// Re-export specific items to avoid conflicts
// From erc4337_bundler
pub use erc4337_bundler::{
    submit_user_operation as bundler_submit_user_operation,
    get_user_operation_receipt as bundler_get_user_operation_receipt,
};

// From erc4337_operations
pub use erc4337_operations::{
    build_and_sign_user_operation_for_payment,
    submit_user_operation,
    get_user_operation_receipt,
    OperationRequest as Erc4337OperationRequest,
};

// From hypermap
pub use hypermap::{
    OperationRequest as HypermapOperationRequest,
    OperationResponse as HypermapOperationResponse,
    resolve_identity,
    create_note,
    execute_via_tba,
};

// Re-export PaymasterConfig from hyperwallet_client::types
pub use hyperware_process_lib::hyperwallet_client::types::PaymasterConfig;
