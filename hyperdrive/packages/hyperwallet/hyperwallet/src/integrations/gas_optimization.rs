//! Gas Optimization for ERC-4337 UserOperations
//!
//! This module provides smart gas limit optimization based on transaction type detection.
//! It helps reduce gas costs by applying appropriate limits for different operations
//! while ensuring transactions don't fail due to insufficient gas.

use hyperware_process_lib::eth::U256;

// ============================================================================
// Operation Type Detection
// ============================================================================

/// Types of operations we can detect and optimize for
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Erc20Transfer,
    Erc721Transfer,
    SimpleExecute,
    ComplexContract,
    Unknown,
}

impl OperationType {
    /// Returns optimized (target) gas limits for this operation type
    pub fn optimized_limits(&self) -> (U256, U256, U256) {
        // Returns (call_gas, verification_gas, pre_verification_gas)
        match self {
            Self::Erc20Transfer => (
                U256::from(80_000),  // ERC20 transfer + TBA overhead
                U256::from(150_000), // TBA validation is complex
                U256::from(60_000),  // Reasonable default for TBA calldata
            ),
            Self::Erc721Transfer => (U256::from(100_000), U256::from(60_000), U256::from(40_000)),
            Self::SimpleExecute => (U256::from(100_000), U256::from(70_000), U256::from(40_000)),
            Self::ComplexContract => (U256::from(200_000), U256::from(100_000), U256::from(50_000)),
            Self::Unknown => (U256::from(150_000), U256::from(80_000), U256::from(45_000)),
        }
    }

    /// Returns maximum allowed gas limits (hard caps) for this operation type
    pub fn max_limits(&self) -> (U256, U256, U256) {
        // Returns (call_gas, verification_gas, pre_verification_gas)
        match self {
            Self::Erc20Transfer => (
                U256::from(100_000), // Hard cap for ERC20
                U256::from(220_000), // TBAs need high verification gas
                U256::from(65_000),  // Buffer for TBA calldata (bundler may require more)
            ),
            Self::Erc721Transfer => (U256::from(130_000), U256::from(80_000), U256::from(50_000)),
            _ => (U256::from(250_000), U256::from(150_000), U256::from(70_000)),
        }
    }
}

/// Detect the type of operation from TBA execute calldata
pub fn detect_operation_type(calldata: &[u8]) -> OperationType {
    // TBA execute function selector: 0x51945447
    const TBA_EXECUTE_SELECTOR: &[u8] = &[0x51, 0x94, 0x54, 0x47];

    // Check if this is a TBA execute call
    if calldata.len() < 4 || !calldata.starts_with(TBA_EXECUTE_SELECTOR) {
        return OperationType::Unknown;
    }

    // For TBA execute, check the nested operation
    if calldata.len() < 136 {
        return OperationType::SimpleExecute;
    }

    // Common ERC20/721 function selectors
    const ERC20_TRANSFER: &[u8] = &[0xa9, 0x05, 0x9c, 0xbb]; // transfer(address,uint256)
    const ERC20_APPROVE: &[u8] = &[0x09, 0x5e, 0xa7, 0xb3]; // approve(address,uint256)
    const ERC721_SAFE_TRANSFER: &[u8] = &[0x42, 0x84, 0x2e, 0x0e]; // safeTransferFrom
    const ERC721_TRANSFER: &[u8] = &[0x23, 0xb8, 0x72, 0xdd]; // transferFrom

    // Check for known function selectors in the nested data
    if contains_bytes(calldata, ERC20_TRANSFER) || contains_bytes(calldata, ERC20_APPROVE) {
        return OperationType::Erc20Transfer;
    }

    if contains_bytes(calldata, ERC721_SAFE_TRANSFER) || contains_bytes(calldata, ERC721_TRANSFER) {
        return OperationType::Erc721Transfer;
    }

    // If we can't identify it, assume it's complex
    OperationType::ComplexContract
}

// ============================================================================
// Gas Limit Optimization
// ============================================================================

/// Apply smart gas limits based on operation type and bundler estimates
///
/// This function takes potentially inflated estimates from the bundler and
/// applies operation-specific caps and optimizations to reduce costs while
/// ensuring the transaction will succeed.
pub fn apply_smart_gas_limits(
    estimated_call: Option<U256>,
    estimated_verification: Option<U256>,
    estimated_pre_verification: Option<U256>,
    operation_type: &OperationType,
) -> (U256, U256, U256) {
    let (opt_call, opt_verif, opt_pre) = operation_type.optimized_limits();
    let (max_call, max_verif, max_pre) = operation_type.max_limits();

    // For call gas: use estimate if available, cap at maximum
    let call_gas = estimated_call
        .map(|est| est.min(max_call))
        .unwrap_or(opt_call);

    // For verification gas: TBAs need high verification gas
    let verification_gas = estimated_verification
        .map(|est| est.min(max_verif))
        .unwrap_or(opt_verif);

    let pre_verification_gas = estimated_pre_verification
        .unwrap_or_else(|| opt_pre.max(max_pre)); // Fallback should never be reached (we error earlier)

    (call_gas, verification_gas, pre_verification_gas)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a byte sequence contains a specific pattern
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_erc20_transfer() {
        // Example TBA execute calldata with nested ERC20 transfer
        let calldata_hex = "51945447000000000000000000000000833589fcd6edb6e08f4c7c32d4f71b54bda029130000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000044a9059cbb000000000000000000000000c82b8f3bb5d8e4526a7f2f5096725af7264ed10c0000000000000000000000000000000000000000000000000000000000000064";
        let calldata = alloy_primitives::hex::decode(calldata_hex).unwrap();

        let op_type = detect_operation_type(&calldata);
        assert_eq!(op_type, OperationType::Erc20Transfer);
    }

    #[test]
    fn test_gas_optimization_caps_inflated_estimates() {
        let op_type = OperationType::Erc20Transfer;

        // Test with inflated estimates from bundler
        let (call_gas, verif_gas, pre_verif_gas) = apply_smart_gas_limits(
            Some(U256::from(300_000)), // Inflated estimate
            Some(U256::from(200_000)), // Reasonable for TBA
            Some(U256::from(100_000)), // Bundler's estimate
            &op_type,
        );

        // Should cap call/verification but trust pre-verification
        assert_eq!(call_gas, U256::from(100_000)); // Capped at max
        assert_eq!(verif_gas, U256::from(200_000)); // Within cap, so unchanged
        assert_eq!(pre_verif_gas, U256::from(100_000)); // Trust bundler's estimate
    }

    #[test]
    fn test_gas_optimization_uses_defaults_when_no_estimates() {
        let op_type = OperationType::Erc20Transfer;

        // Test without estimates
        let (call_gas, verif_gas, pre_verif_gas) =
            apply_smart_gas_limits(None, None, None, &op_type);

        // Should use optimized defaults for call/verification, max for pre-verification
        assert_eq!(call_gas, U256::from(80_000));
        assert_eq!(verif_gas, U256::from(150_000));
        assert_eq!(pre_verif_gas, U256::from(65_000)); // max(60k optimized, 65k max) = 65k
    }

    #[test]
    fn test_gas_optimization_respects_bundler_pre_verification_gas() {
        let op_type = OperationType::Erc20Transfer;

        // Test with the exact scenario from production: bundler requires 58,531 (0xe4a3)
        let (call_gas, verif_gas, pre_verif_gas) = apply_smart_gas_limits(
            Some(U256::from(80_000)),   // Reasonable call gas
            Some(U256::from(150_000)),  // Reasonable verification
            Some(U256::from(58_531)),   // Bundler's exact requirement: 0xe4a3
            &op_type,
        );

        // CRITICAL: Pre-verification gas must NOT be capped below bundler's requirement
        assert_eq!(pre_verif_gas, U256::from(58_531)); // Must use bundler's exact value
        
        // Call and verification can still be capped
        assert_eq!(call_gas, U256::from(80_000));
        assert_eq!(verif_gas, U256::from(150_000));
    }
}
