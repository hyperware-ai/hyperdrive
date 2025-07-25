use crate::operations::types::{OperationError, OperationRequest, OperationResponse};
use crate::state::HyperwalletState;
use crate::operations::ethereum::{get_signer_from_wallet, sign_hash};
use hyperware_process_lib::wallet::{
    UserOperationBuilder, UserOperation, PackedUserOperation,
    get_known_paymaster, get_entry_point_address,
    encode_usdc_paymaster_data, create_tba_userop_calldata,
    resolve_token_symbol,
};
use hyperware_process_lib::eth::{Address as EthAddress, U256, TransactionRequest, TransactionInput};
use hyperware_process_lib::logging::{info, error, warn};
use hyperware_process_lib::signer::Signer;
use alloy_primitives::{hex, Bytes as AlloyBytes, FixedBytes};
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;
use hyperware_process_lib::eth::Provider;
use crate::bundler;

/// Parameters for building a UserOperation
#[derive(Debug, Deserialize)]
pub struct BuildUserOpParams {
    /// Target address for the call
    pub target: String,
    /// Encoded call data (hex string)
    pub call_data: String,
    /// Value to send in wei (optional, defaults to 0)
    pub value: Option<String>,
    /// Whether to use a paymaster for gasless transactions
    pub use_paymaster: Option<bool>,
    /// Optional nonce override
    pub nonce: Option<String>,
    /// Optional gas parameters
    pub gas_params: Option<GasParams>,
    /// Optional metadata for paymaster testing
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct GasParams {
    pub call_gas_limit: Option<String>,
    pub verification_gas_limit: Option<String>,
    pub pre_verification_gas: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
}

/// Parameters for building and signing UserOp with permit
#[derive(Debug, Deserialize)]
pub struct BuildAndSignUserOpParams {
    /// Target address for the call
    pub target: String,
    /// Encoded call data (hex string)
    pub call_data: String,
    /// Value to send in wei (optional, defaults to 0)
    pub value: Option<String>,
    /// Whether to use a paymaster for gasless transactions
    pub use_paymaster: Option<bool>,
    /// Optional nonce override
    pub nonce: Option<String>,
    /// Optional gas parameters
    pub gas_params: Option<GasParams>,
    /// Password for wallet unlock (required for signing)
    pub password: Option<String>,
    /// Optional metadata for paymaster configuration
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Build and sign a UserOperation with dynamic value fetching for operator payment flow
/// This function emulates the behavior of test-submit-userop command
pub fn build_and_sign_user_operation_for_payment(
    request: OperationRequest,
    state: &HyperwalletState,
) -> OperationResponse {
    info!("HEllo.");
    let process_address = &request.auth.process_address;
    let wallet_id = match request.wallet_id.as_deref() {
        Some(id) => id,
        None => return OperationResponse::error(
            OperationError::invalid_params("wallet_id required")
        ),
    };
    
    // Parse parameters
    let params: BuildAndSignUserOpParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => return OperationResponse::error(
            OperationError::invalid_params(&format!("Invalid parameters: {}", e))
        ),
    };
    
    // Parse target address
    let _target = match EthAddress::from_str(&params.target) {
        Ok(addr) => addr,
        Err(_) => return OperationResponse::error(
            OperationError::invalid_params("Invalid target address")
        ),
    };
    
    // Parse call data
    let call_data_bytes = match hex::decode(params.call_data.trim_start_matches("0x")) {
        Ok(data) => data,
        Err(_) => return OperationResponse::error(
            OperationError::invalid_params("Invalid call data hex")
        ),
    };
    
    // Parse value
    let _value = match params.value.as_deref() {
        Some(v) => U256::from_str(v).unwrap_or(U256::ZERO),
        None => U256::ZERO,
    };
    
    let sender = if let Some(tba_addr) = params.metadata.as_ref()
        .and_then(|m| m.get("tba_address"))
        .and_then(|v| v.as_str()) {
        // Use TBA address from metadata as sender
        match EthAddress::from_str(tba_addr) {
            Ok(addr) => {
                info!("Using TBA address from metadata as sender: {}", tba_addr);
                addr
            }
            Err(_) => return OperationResponse::error(
                OperationError::invalid_params("Invalid TBA address in metadata")
            ),
        }
    } else if wallet_id.starts_with("0x") && wallet_id.len() == 42 {
        // Fallback: It's a TBA address, use it directly as sender
        match EthAddress::from_str(wallet_id) {
            Ok(addr) => addr,
            Err(_) => return OperationResponse::error(
                OperationError::invalid_params("Invalid TBA address format")
            ),
        }
    } else {
        // For EOA wallets, this shouldn't happen in operator payment flow
        return OperationResponse::error(
            OperationError::invalid_params("Expected TBA address as wallet_id or in metadata")
        );
    };
    
    info!("Building UserOperation for payment from TBA: {}", sender);
    
    let chain_id = request.chain_id.unwrap_or(8453); // Default to Base
    
    // Get the entry point address
    let entry_point = match get_entry_point_address(chain_id) {
        Some(addr) => addr,
        None => return OperationResponse::error(
            OperationError::chain_not_allowed(chain_id)
        ),
    };
    
    // Create provider for dynamic fetching
    let provider = Provider::new(chain_id as u64, 30);
    
    // ALWAYS fetch nonce dynamically from EntryPoint (like test-submit-userop)
        info!("Fetching dynamic nonce for sender: {}", sender);
    let nonce = match fetch_nonce_from_entry_point(&provider, sender, entry_point) {
            Ok(nonce) => {
                info!("Dynamic nonce fetched: {}", nonce);
            nonce
            }
            Err(e) => {
            error!("Failed to fetch nonce: {}", e);
            return OperationResponse::error(
                OperationError::blockchain_error(&format!("Failed to fetch nonce: {}", e))
            );
        }
    };
    
    // ALWAYS fetch dynamic gas prices (like test-submit-userop)
        info!("Fetching dynamic gas prices");
    let (max_fee_per_gas, max_priority_fee_per_gas) = match fetch_dynamic_gas_prices(&provider) {
            Ok((max_fee, priority_fee)) => {
                info!("Dynamic gas prices set - max fee: {} wei, priority: {} wei", max_fee, priority_fee);
            (U256::from(max_fee), U256::from(priority_fee))
            }
            Err(e) => {
            error!("Failed to fetch gas prices: {}", e);
            return OperationResponse::error(
                OperationError::blockchain_error(&format!("Failed to fetch gas prices: {}", e))
            );
        }
    };
    
    // DYNAMIC GAS ESTIMATION - First try with conservative estimates, then get real estimates from Candide
    let mut call_gas_limit = U256::from(300_000);
    let mut verification_gas_limit = U256::from(150_000);
    let mut pre_verification_gas = U256::from(100_000);
    
    // We'll update these after building the initial UserOp and calling eth_estimateUserOperationGas
    
    // Configure paymaster if requested
    let mut is_circle_paymaster = false;
    let mut paymaster_and_data = Vec::new();
    
    if params.use_paymaster.unwrap_or(false) {
        // Check if it's Circle paymaster from metadata
        is_circle_paymaster = params.metadata.as_ref()
            .and_then(|m| m.get("is_circle_paymaster"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        if is_circle_paymaster {
            info!("Using Circle paymaster with proper gas limits");
            
            // For Circle paymaster, build packed format:
            // paymaster address (20 bytes) + verification gas (16 bytes) + post-op gas (16 bytes)
            
            // Paymaster address
            let paymaster_bytes = hex::decode("0578cFB241215b77442a541325d6A4E6dFE700Ec").unwrap();
            paymaster_and_data.extend_from_slice(&paymaster_bytes);
            
            // Verification gas limit (500000 = 0x7a120) as 16 bytes
            let verif_gas: u128 = 500000;
            paymaster_and_data.extend_from_slice(&verif_gas.to_be_bytes());
            
            // Post-op gas limit (300000 = 0x493e0) as 16 bytes
            let post_op_gas: u128 = 300000;
            paymaster_and_data.extend_from_slice(&post_op_gas.to_be_bytes());
            
            info!("Circle paymaster data (packed): 0x{}", hex::encode(&paymaster_and_data));
        } else {
            // Standard paymaster logic
            if let Some(paymaster) = get_known_paymaster(chain_id) {
                if let Ok(usdc_addr) = resolve_token_symbol("USDC", chain_id) {
                    paymaster_and_data = encode_usdc_paymaster_data(
                paymaster,
                                usdc_addr,
                        U256::ZERO,
                    );
                    info!("Standard paymaster data configured");
                }
            }
        }
    }
    
    // Get wallet for signing
    let wallet_info = match state.wallets_by_process.get(process_address)
        .and_then(|wallets| wallets.get(wallet_id)) {
        Some(info) => info,
        None => return OperationResponse::error(
            OperationError::wallet_not_found(wallet_id)
        ),
    };
    
    // Pack gas limits into accountGasLimits (verificationGasLimit << 128 | callGasLimit)
    let mut account_gas_limits = [0u8; 32];
    account_gas_limits[..16].copy_from_slice(&verification_gas_limit.to_be_bytes::<32>()[16..]);
    account_gas_limits[16..].copy_from_slice(&call_gas_limit.to_be_bytes::<32>()[16..]);
    
    // Pack gas fees (maxPriorityFeePerGas << 128 | maxFeePerGas)
    let mut gas_fees = [0u8; 32];
    gas_fees[..16].copy_from_slice(&max_priority_fee_per_gas.to_be_bytes::<32>()[16..]);
    gas_fees[16..].copy_from_slice(&max_fee_per_gas.to_be_bytes::<32>()[16..]);
    
    // STEP 1: Create initial UserOp for gas estimation (clone the data since we'll use it again)
    let _initial_user_op = PackedUserOperation {
        sender,
        nonce,
        initCode: Vec::new().into(), // Empty for existing accounts
        callData: call_data_bytes.clone().into(),
        accountGasLimits: account_gas_limits.into(),
        preVerificationGas: pre_verification_gas,
        gasFees: gas_fees.into(),
        paymasterAndData: paymaster_and_data.clone().into(),
        signature: Vec::new().into(), // Empty for now, will be filled after signing
    };
    
    // STEP 2: Get the wallet's signer for gas estimation signature
    let password_value = params.password.as_deref().map(|s| serde_json::Value::String(s.to_string()));
    let signer = match get_signer_from_wallet(wallet_info, password_value.as_ref()) {
        Ok(s) => s,
        Err(e) => return e,
    };
    
    info!("Using EOA {} to sign for TBA {}", signer.address(), sender);
    
    // STEP 3: Calculate UserOperation hash for gas estimation
    info!("Calculating UserOp hash for gas estimation signature...");
    let estimation_paymaster_data = if is_circle_paymaster { Vec::new() } else { paymaster_and_data.clone() };
    let estimation_hash = match get_user_op_hash_for_estimation(
        &provider,
        sender,
        nonce,
        &call_data_bytes,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        &estimation_paymaster_data,
        entry_point,
    ) {
        Ok(hash) => hash,
        Err(e) => {
            error!("Failed to calculate estimation hash: {}", e);
            return OperationResponse::error(
                OperationError::blockchain_error(&format!("Failed to calculate estimation hash: {}", e))
            );
        }
    };
    
    // STEP 4: Sign with real delegated signer
    let real_signature = match signer.sign_hash(&estimation_hash) {
        Ok(sig) => sig,
        Err(e) => {
            error!("Failed to sign estimation hash: {}", e);
            return OperationResponse::error(
                OperationError::blockchain_error(&format!("Failed to sign estimation hash: {}", e))
            );
        }
    };
    
    info!("Real signature generated for gas estimation: 0x{}", hex::encode(&real_signature));
    
    // STEP 5: Get dynamic gas estimates from Candide with REAL signature
    info!("Estimating gas dynamically from Candide with real signature...");
    
    // Convert to the format expected by eth_estimateUserOperationGas (Candide v0.8 format)
    let estimation_request = if is_circle_paymaster {
        serde_json::json!({
            "sender": sender.to_string(),
            "nonce": format!("0x{:x}", nonce),
            "callData": format!("0x{}", hex::encode(&call_data_bytes)),
            "callGasLimit": format!("0x{:x}", call_gas_limit),
            "verificationGasLimit": format!("0x{:x}", verification_gas_limit),
            "preVerificationGas": format!("0x{:x}", pre_verification_gas),
            "maxFeePerGas": format!("0x{:x}", max_fee_per_gas),
            "maxPriorityFeePerGas": format!("0x{:x}", max_priority_fee_per_gas),
                "factory": serde_json::Value::Null,
                "factoryData": serde_json::Value::Null,
                "paymaster": "0x0578cFB241215b77442a541325d6A4E6dFE700Ec",
                "paymasterVerificationGasLimit": "0x7a120",
                "paymasterPostOpGasLimit": "0x493e0",
            "paymasterData": "0x",
            "signature": format!("0x{}", hex::encode(&real_signature)) // REAL SIGNATURE from delegated signer!
        })
    } else {
        // Fallback format - shouldn't happen for Circle paymaster
        serde_json::json!({
            "sender": sender.to_string(),
            "nonce": format!("0x{:x}", nonce),
            "callData": format!("0x{}", hex::encode(&call_data_bytes)),
            "signature": format!("0x{}", hex::encode(&real_signature)) // REAL SIGNATURE
        })
    };
    
    // Call eth_estimateUserOperationGas
    match crate::bundler::estimate_user_operation_gas(estimation_request, entry_point.to_string()) {
        Ok(gas_estimates) => {
            info!("Gas estimation successful: {:?}", gas_estimates);
            
            // Update gas limits with Candide's estimates (with sufficient buffer for TBA)
            info!("Processing gas estimates: {:?}", gas_estimates);
            
            if let Some(estimated_call_gas) = gas_estimates.get("callGasLimit").and_then(|v| v.as_str()) {
                info!("Found callGasLimit: {}", estimated_call_gas);
                if let Ok(gas) = U256::from_str_radix(estimated_call_gas.trim_start_matches("0x"), 16) {
                    call_gas_limit = gas + U256::from(10000); // +10k buffer
                    info!("Updated call gas limit to: {}", call_gas_limit);
                } else {
                    warn!("Failed to parse callGasLimit: {}", estimated_call_gas);
                }
            } else {
                warn!("No callGasLimit found in gas estimates");
            }
            
            if let Some(estimated_verif_gas) = gas_estimates.get("verificationGasLimit").and_then(|v| v.as_str()) {
                info!("Found verificationGasLimit: {}", estimated_verif_gas);
                if let Ok(gas) = U256::from_str_radix(estimated_verif_gas.trim_start_matches("0x"), 16) {
                    // TBA signature verification needs extra gas - use 20k buffer
                    verification_gas_limit = gas + U256::from(20000); // +20k buffer for TBA signature verification
                    info!("Updated verification gas limit to: {} (+20k buffer for TBA)", verification_gas_limit);
                } else {
                    warn!("Failed to parse verificationGasLimit: {}", estimated_verif_gas);
                }
            } else {
                warn!("No verificationGasLimit found in gas estimates");
            }
            
            if let Some(estimated_pre_gas) = gas_estimates.get("preVerificationGas").and_then(|v| v.as_str()) {
                info!("Found preVerificationGas: {}", estimated_pre_gas);
                if let Ok(gas) = U256::from_str_radix(estimated_pre_gas.trim_start_matches("0x"), 16) {
                    pre_verification_gas = gas + U256::from(5000); // +5k buffer
                    info!("Updated pre-verification gas to: {}", pre_verification_gas);
                } else {
                    warn!("Failed to parse preVerificationGas: {}", estimated_pre_gas);
                }
            } else {
                warn!("No preVerificationGas found in gas estimates");
            }
            }
            Err(e) => {
            warn!("Gas estimation failed, using defaults: {:?}", e);
            // Keep the existing values if estimation fails
        }
    }
    
    // STEP 6: Rebuild with updated gas limits
    // Re-pack gas limits into accountGasLimits with updated values
    let mut account_gas_limits = [0u8; 32];
    account_gas_limits[..16].copy_from_slice(&verification_gas_limit.to_be_bytes::<32>()[16..]);
    account_gas_limits[16..].copy_from_slice(&call_gas_limit.to_be_bytes::<32>()[16..]);
    
    // Re-pack gas fees (unchanged)
    let mut gas_fees = [0u8; 32];
    gas_fees[..16].copy_from_slice(&max_priority_fee_per_gas.to_be_bytes::<32>()[16..]);
    gas_fees[16..].copy_from_slice(&max_fee_per_gas.to_be_bytes::<32>()[16..]);
    
    // Create the final PackedUserOperation with updated gas limits
    let user_op = PackedUserOperation {
        sender,
        nonce,
        initCode: Vec::new().into(), // Empty for existing accounts
        callData: call_data_bytes.into(),
        accountGasLimits: account_gas_limits.into(),
        preVerificationGas: pre_verification_gas,
        gasFees: gas_fees.into(),
        paymasterAndData: paymaster_and_data.into(),
        signature: Vec::new().into(), // Empty for now, will be filled after signing
    };
    
    // Get the UserOperation hash from EntryPoint
    let user_op_hash = match get_user_op_hash(&provider, &user_op, entry_point) {
        Ok(hash) => hash,
        Err(e) => return OperationResponse::error(
            OperationError::blockchain_error(&format!("Failed to get UserOp hash: {}", e))
        ),
    };
    
    info!("UserOp hash: 0x{}", hex::encode(&user_op_hash));
    
    // Sign the hash using the same signer (reuse from gas estimation)
    let signature = match signer.sign_hash(&user_op_hash) {
        Ok(sig) => sig,
        Err(e) => return OperationResponse::error(
            OperationError::blockchain_error(&format!("Failed to sign UserOp: {}", e))
        ),
    };
    
    info!("UserOperation signed successfully");
    info!("Signature: 0x{}", hex::encode(&signature));
    
    // Create signed UserOperation with UPDATED gas limits
    // Re-pack the updated gas limits into accountGasLimits
    let mut final_account_gas_limits = [0u8; 32];
    final_account_gas_limits[..16].copy_from_slice(&verification_gas_limit.to_be_bytes::<32>()[16..]);
    final_account_gas_limits[16..].copy_from_slice(&call_gas_limit.to_be_bytes::<32>()[16..]);
    
    let signed_user_op = PackedUserOperation {
        sender: user_op.sender,
        nonce: user_op.nonce,
        initCode: user_op.initCode,
        callData: user_op.callData,
        accountGasLimits: final_account_gas_limits.into(), // Use updated gas limits
        preVerificationGas: pre_verification_gas, // Use updated pre-verification gas
        gasFees: user_op.gasFees, // Gas fees unchanged
        paymasterAndData: user_op.paymasterAndData,
        signature: signature.into(),
    };
    
    info!("Built and signed UserOperation for payment from TBA {}", signed_user_op.sender);
    
    // Return the appropriate format based on paymaster type
    if is_circle_paymaster {
        // For Circle paymaster, return the unpacked format expected by Candide
        // Use the updated gas limits directly (not from packed format)
        info!("Circle paymaster response - call_gas_limit: {}, verification_gas_limit: {}", call_gas_limit, verification_gas_limit);
        
        // Extract gas fees (maxPriorityFeePerGas << 128 | maxFeePerGas)
        let gas_fees_bytes = signed_user_op.gasFees;
        let max_fee_per_gas = U256::from_be_slice(&gas_fees_bytes[16..32]);
        let max_priority_fee_per_gas = U256::from_be_slice(&gas_fees_bytes[0..16]);
        
        OperationResponse::success(json!({
            "signed_user_operation": {
                "sender": signed_user_op.sender.to_string(),
                "nonce": format!("0x{:x}", signed_user_op.nonce),
                "callData": format!("0x{}", hex::encode(&signed_user_op.callData)),
                "callGasLimit": format!("0x{:x}", call_gas_limit),
                "verificationGasLimit": format!("0x{:x}", verification_gas_limit),
                "preVerificationGas": format!("0x{:x}", signed_user_op.preVerificationGas),
                "maxFeePerGas": format!("0x{:x}", max_fee_per_gas),
                "maxPriorityFeePerGas": format!("0x{:x}", max_priority_fee_per_gas),
                "signature": format!("0x{}", hex::encode(&signed_user_op.signature)),
                "factory": null,
                "factoryData": null,
                "paymaster": "0x0578cFB241215b77442a541325d6A4E6dFE700Ec",
                "paymasterVerificationGasLimit": "0x7a120", // 500000
                "paymasterPostOpGasLimit": "0x493e0", // 300000
                "paymasterData": "0x"
            },
            "entry_point": entry_point.to_string(),
            "chain_id": chain_id,
            "ready_to_submit": true,
        }))
    } else {
        // Return the standard packed format
        OperationResponse::success(json!({
            "signed_user_operation": {
                "sender": signed_user_op.sender.to_string(),
                "nonce": format!("0x{:x}", signed_user_op.nonce),
                "init_code": format!("0x{}", hex::encode(&signed_user_op.initCode)),
                "call_data": format!("0x{}", hex::encode(&signed_user_op.callData)),
                "account_gas_limits": format!("0x{}", hex::encode(&signed_user_op.accountGasLimits)),
                "pre_verification_gas": format!("0x{:x}", signed_user_op.preVerificationGas),
                "gas_fees": format!("0x{}", hex::encode(&signed_user_op.gasFees)),
                "paymaster_and_data": format!("0x{}", hex::encode(&signed_user_op.paymasterAndData)),
                "signature": format!("0x{}", hex::encode(&signed_user_op.signature)),
            },
            "entry_point": entry_point.to_string(),
            "chain_id": chain_id,
            "ready_to_submit": true,
        }))
    }
}

/// Parameters for submitting to bundler
#[derive(Debug, Deserialize)]
pub struct SubmitUserOpParams {
    /// The signed UserOperation
    pub signed_user_operation: serde_json::Value,
    /// Entry point address
    pub entry_point: String,
    /// Optional bundler URL (uses default if not provided)
    pub bundler_url: Option<String>,
}

/// Submit UserOperation to bundler
pub fn submit_user_operation(
    request: OperationRequest,
    _state: &HyperwalletState,
) -> OperationResponse {
    info!("Submitting UserOperation to bundler ^^");
    
    // Parse parameters
    let params: SubmitUserOpParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => return OperationResponse::error(
            OperationError::invalid_params(&format!("Invalid parameters: {}", e))
        ),
    };
    
    // Use the bundler client to submit the UserOperation
    match crate::bundler::submit_user_operation(
        params.signed_user_operation,
        params.entry_point,
    ) {
        Ok(user_op_hash) => {
            info!("UserOperation submitted successfully: {}", user_op_hash);
            OperationResponse::success(json!({
                "user_op_hash": user_op_hash,
                "message": "UserOperation submitted to bundler"
            }))
        }
        Err(e) => {
            error!("Failed to submit UserOperation: {:?}", e);
            OperationResponse::error(e)
        }
    }
}

/// Get receipt for a UserOperation
pub fn get_user_operation_receipt(
    request: OperationRequest,
    _state: &HyperwalletState,
) -> OperationResponse {
    info!("Getting UserOperation receipt");
    
    // Parse user_op_hash from params
    let user_op_hash = match request.params.get("user_op_hash").and_then(|v| v.as_str()) {
        Some(hash) => hash,
        None => return OperationResponse::error(
            OperationError::invalid_params("user_op_hash required")
        ),
    };
    
    // Use the bundler client to get the receipt
    match crate::bundler::get_user_operation_receipt(user_op_hash.to_string()) {
        Ok(receipt_data) => {
            info!("UserOperation receipt retrieved successfully");
            OperationResponse::success(receipt_data)
        }
        Err(e) => {
            error!("Failed to get UserOperation receipt: {:?}", e);
            OperationResponse::error(e)
        }
    }
}

/// Fetch nonce from EntryPoint contract
fn fetch_nonce_from_entry_point(
    provider: &Provider,
    sender: EthAddress,
    entry_point: EthAddress,
) -> Result<U256, String> {
    use alloy_sol_types::*;
    use hyperware_process_lib::eth::{TransactionRequest, TransactionInput};
    
    sol! {
        function getNonce(address sender, uint192 key) external view returns (uint256 nonce);
    }
    
    let get_nonce_call = getNonceCall {
        sender: sender.to_string().parse().map_err(|_| "Invalid address")?,
        key: alloy_primitives::U256::ZERO, // Nonce key 0
    };
    
    let nonce_call_data = get_nonce_call.abi_encode();
    let nonce_tx_req = TransactionRequest::default()
        .input(TransactionInput::new(nonce_call_data.into()))
        .to(entry_point);
    
    match provider.call(nonce_tx_req, None) {
        Ok(bytes) => {
            let decoded = U256::from_be_slice(&bytes);
            Ok(decoded)
        }
        Err(e) => {
            Err(format!("Failed to fetch nonce: {}", e))
        }
    }
}

/// Fetch dynamic gas prices from the network
fn fetch_dynamic_gas_prices(provider: &Provider) -> Result<(u128, u128), String> {
    use hyperware_process_lib::eth::BlockNumberOrTag;
    
    match provider.get_block_by_number(BlockNumberOrTag::Latest, false) {
        Ok(Some(block)) => {
            let base_fee = block.header.inner.base_fee_per_gas.unwrap_or(1_000_000_000) as u128;
            
            // Calculate dynamic gas prices based on current network conditions
            let max_fee = base_fee + (base_fee / 3); // Add 33% buffer
            let priority_fee = std::cmp::max(100_000_000u128, base_fee / 10); // At least 0.1 gwei
            
            Ok((max_fee, priority_fee))
        }
        Ok(None) => {
            Err("No latest block found".to_string())
        }
        Err(e) => {
            Err(format!("Failed to get latest block: {}", e))
        }
    }
}

/// Get UserOperation hash for gas estimation (simplified)
fn get_user_op_hash_for_estimation(
    provider: &Provider,
    sender: EthAddress,
    nonce: U256,
    call_data: &[u8],
    call_gas_limit: U256,
    verification_gas_limit: U256,
    pre_verification_gas: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    paymaster_data: &[u8],
    entry_point: EthAddress,
) -> Result<Vec<u8>, String> {
    use alloy_sol_types::*;
    
    // Pack gas limits into accountGasLimits (verificationGasLimit << 128 | callGasLimit)
    let mut account_gas_limits = [0u8; 32];
    account_gas_limits[..16].copy_from_slice(&verification_gas_limit.to_be_bytes::<32>()[16..]);
    account_gas_limits[16..].copy_from_slice(&call_gas_limit.to_be_bytes::<32>()[16..]);
    
    // Pack gas fees (maxPriorityFeePerGas << 128 | maxFeePerGas)
    let mut gas_fees = [0u8; 32];
    gas_fees[..16].copy_from_slice(&max_priority_fee_per_gas.to_be_bytes::<32>()[16..]);
    gas_fees[16..].copy_from_slice(&max_fee_per_gas.to_be_bytes::<32>()[16..]);
    
    // Define the PackedUserOperation type and getUserOpHash function
    sol! {
        struct PackedUserOperation {
            address sender;
            uint256 nonce;
            bytes initCode;
            bytes callData;
            bytes32 accountGasLimits;
            uint256 preVerificationGas;
            bytes32 gasFees;
            bytes paymasterAndData;
            bytes signature;
        }
        
        function getUserOpHash(PackedUserOperation userOp) external view returns (bytes32);
    }
    
    // Create the PackedUserOperation instance for the ABI call
    let packed_user_op = PackedUserOperation {
        sender: sender.to_string().parse().map_err(|_| "Invalid address")?,
        nonce,
        initCode: AlloyBytes::new(), // Empty for existing accounts
        callData: AlloyBytes::from(call_data.to_vec()),
        accountGasLimits: FixedBytes::from_slice(&account_gas_limits),
        preVerificationGas: pre_verification_gas,
        gasFees: FixedBytes::from_slice(&gas_fees),
        paymasterAndData: AlloyBytes::from(paymaster_data.to_vec()),
        signature: AlloyBytes::new(), // Empty for hash calculation
    };
    
    // Create the function call
    let get_hash_call = getUserOpHashCall {
        userOp: packed_user_op,
    };
    
    // Encode the call
    let call_data = get_hash_call.abi_encode();
    
    // Create transaction request to call EntryPoint.getUserOpHash()
    let tx_req = TransactionRequest::default()
        .input(TransactionInput::new(call_data.into()))
        .to(entry_point);
    
    // Make the call to get the hash
    match provider.call(tx_req, None) {
        Ok(bytes) => {
            info!("Got UserOp hash from EntryPoint for estimation: 0x{}", hex::encode(&bytes));
            
            // Decode the result (should be 32 bytes - the hash)
            if bytes.len() == 32 {
                Ok(bytes.to_vec())
            } else {
                // If the result is longer, it might be ABI-encoded, try to decode
                match getUserOpHashCall::abi_decode_returns(&bytes, false) {
                    Ok(decoded_hash) => Ok(decoded_hash._0.to_vec()),
                    Err(_) => {
                        error!("Failed to decode getUserOpHash result, using raw bytes");
                        Ok(bytes.to_vec())
                    }
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to call EntryPoint.getUserOpHash(): {}", e))
        }
    }
}

/// Get UserOperation hash from EntryPoint contract
fn get_user_op_hash(
    provider: &Provider,
    user_op: &PackedUserOperation,
    entry_point: EthAddress,
) -> Result<Vec<u8>, String> {
    use alloy_sol_types::*;
    
    // Define the PackedUserOperation type and getUserOpHash function
    sol! {
        struct PackedUserOperation {
            address sender;
            uint256 nonce;
            bytes initCode;
            bytes callData;
            bytes32 accountGasLimits;
            uint256 preVerificationGas;
            bytes32 gasFees;
            bytes paymasterAndData;
            bytes signature;
        }
        
        function getUserOpHash(PackedUserOperation userOp) external view returns (bytes32);
    }
    
    // Create the PackedUserOperation instance for the ABI call
    let packed_user_op = PackedUserOperation {
        sender: user_op.sender.to_string().parse().map_err(|_| "Invalid address")?,
        nonce: user_op.nonce,
        initCode: AlloyBytes::from(user_op.initCode.to_vec()),
        callData: AlloyBytes::from(user_op.callData.to_vec()),
        accountGasLimits: FixedBytes::from_slice(user_op.accountGasLimits.as_ref()),
        preVerificationGas: user_op.preVerificationGas,
        gasFees: FixedBytes::from_slice(user_op.gasFees.as_ref()),
        paymasterAndData: AlloyBytes::from(user_op.paymasterAndData.to_vec()),
        signature: AlloyBytes::new(), // Empty for hash calculation
    };
    
    // Create the function call
    let get_hash_call = getUserOpHashCall {
        userOp: packed_user_op,
    };
    
    // Encode the call
    let call_data = get_hash_call.abi_encode();
    
    // Create transaction request to call EntryPoint.getUserOpHash()
    let tx_req = TransactionRequest::default()
        .input(TransactionInput::new(call_data.into()))
        .to(entry_point);
    
    // Make the call to get the hash
    match provider.call(tx_req, None) {
        Ok(bytes) => {
            info!("Got UserOp hash from EntryPoint: 0x{}", hex::encode(&bytes));
            
            // Decode the result (should be 32 bytes - the hash)
            if bytes.len() == 32 {
                Ok(bytes.to_vec())
            } else {
                // If the result is longer, it might be ABI-encoded, try to decode
                match getUserOpHashCall::abi_decode_returns(&bytes, false) {
                    Ok(decoded_hash) => Ok(decoded_hash._0.to_vec()),
                    Err(_) => {
                        error!("Failed to decode getUserOpHash result, using raw bytes");
                        Ok(bytes.to_vec())
                    }
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to call EntryPoint.getUserOpHash(): {}", e))
        }
    }
}