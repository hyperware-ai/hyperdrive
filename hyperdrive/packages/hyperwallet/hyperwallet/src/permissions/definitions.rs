/// Permission types and helper functions
/// 
/// This module provides type definitions and utility functions for managing
/// process permissions and operation validation.

// Import and re-export the process_lib types for local use
pub use hyperware_process_lib::hyperwallet_client::types::{
    Operation, ProcessPermissions, SpendingLimits, UpdatableSetting
};

/// Helper to check if an operation requires wallet access
pub fn operation_requires_wallet(operation: &Operation) -> bool {
    matches!(operation, 
        Operation::SendEth |
        Operation::SendToken |
        Operation::GetBalance |
        Operation::GetTokenBalance |
        Operation::SignMessage |
        Operation::SignTransaction |
        Operation::CallContract |
        Operation::ApproveToken |
        Operation::ExecuteViaTba |
        Operation::GetTransactionHistory |
        Operation::EstimateGas |
        Operation::EncryptWallet |
        Operation::DecryptWallet |
        Operation::ExportWallet |
        Operation::DeleteWallet |
        Operation::GetWalletInfo
    )
}

/// Helper to check if an operation modifies state
pub fn operation_modifies_state(operation: &Operation) -> bool {
    matches!(operation,
        Operation::CreateWallet |
        Operation::ImportWallet |
        Operation::DeleteWallet |
        Operation::SendEth |
        Operation::SendToken |
        Operation::CallContract |
        Operation::ApproveToken |
        Operation::ExecuteViaTba |
        Operation::CreateNote |
        Operation::MintEntry |
        Operation::SetupDelegation |
        Operation::SetupTbaDelegation |
        Operation::EncryptWallet |
        Operation::DecryptWallet |
        Operation::RenameWallet |
        Operation::RegisterProcess |
        Operation::UpdateSpendingLimits
    )
} 