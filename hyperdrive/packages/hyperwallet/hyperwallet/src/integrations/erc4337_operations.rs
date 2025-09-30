//! ERC-4337 UserOperation Building and Management
//!
//! This module handles the construction, signing, and submission of UserOperations
//! for Token Bound Account (TBA) interactions through the ERC-4337 protocol.

use alloy_primitives::{hex, Bytes as AlloyBytes, FixedBytes};
use alloy_sol_types::{sol, SolCall};
use hyperware_process_lib::eth::{
    Address as EthAddress, BlockNumberOrTag, Provider, TransactionInput, TransactionRequest, U256,
};
use hyperware_process_lib::hyperwallet_client::types::{
    BuildAndSignUserOperationForPaymentRequest, BuildAndSignUserOperationResponse,
    GetUserOperationReceiptRequest, HyperwalletResponse,
    HyperwalletResponseData, OperationError, PaymasterConfig, SubmitUserOperationRequest,
    SubmitUserOperationResponse, UserOperationReceiptResponse,
};
use hyperware_process_lib::logging::{error, info};
use hyperware_process_lib::signer::Signer;
use hyperware_process_lib::wallet::{get_entry_point_address, PackedUserOperation};
use hyperware_process_lib::Address;
use serde_json::json;
use std::str::FromStr;

use crate::config::DEFAULT_CHAIN_ID;
use crate::core::transactions::get_signer_from_wallet;
use crate::integrations::erc4337_bundler as bundler;
use crate::integrations::gas_optimization::{apply_smart_gas_limits, detect_operation_type};
use crate::state::HyperwalletState;

// ============================================================================
// Public API Functions
// ============================================================================

/// Build and sign a UserOperation for a payment transaction
pub fn build_and_sign_user_operation_for_payment(
    request: BuildAndSignUserOperationForPaymentRequest,
    _session_id: &str,
    address: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    info!("Building UserOperation for payment");

    // Parse and validate inputs
    let operation_context = match prepare_operation_context(&request, address, state) {
        Ok(ctx) => ctx,
        Err(e) => return HyperwalletResponse::error(e),
    };

    // Estimate gas with smart optimization
    let gas_limits = match estimate_optimized_gas(&operation_context) {
        Ok(limits) => limits,
        Err(e) => return HyperwalletResponse::error(e),
    };

    // Build and sign the UserOperation
    let signed_user_op = match build_and_sign_user_op(&operation_context, &gas_limits) {
        Ok(op) => op,
        Err(e) => return HyperwalletResponse::error(e),
    };

    // Format response for Candide bundler
    let response_json = format_user_op_response(&signed_user_op, &operation_context);

    let response_data = BuildAndSignUserOperationResponse {
        signed_user_operation: response_json.to_string(),
        entry_point: operation_context.entry_point.to_string(),
        ready_to_submit: true,
    };

    HyperwalletResponse::success(HyperwalletResponseData::BuildAndSignUserOperationForPayment(response_data))
}

/// Submit a signed UserOperation to the bundler
pub fn submit_user_operation(
    request: SubmitUserOperationRequest,
    _session_id: &str,
) -> HyperwalletResponse {
    info!("Submitting UserOperation to bundler");

    match bundler::submit_user_operation(
        serde_json::from_str(&request.signed_user_operation).unwrap_or_else(|_| serde_json::Value::String(request.signed_user_operation.clone())),
        request.entry_point.clone(),
    ) {
        Ok(user_op_hash) => {
            info!("UserOperation submitted successfully: {}", user_op_hash);
            let response = SubmitUserOperationResponse { user_op_hash };
            HyperwalletResponse::success(HyperwalletResponseData::SubmitUserOperation(response))
        }
        Err(e) => {
            error!("Failed to submit UserOperation: {:?}", e);
            HyperwalletResponse::error(e)
        }
    }
}

/// Get the receipt for a submitted UserOperation
/// Returns both the userOp hash and the actual transaction hash for proof of payment
pub fn get_user_operation_receipt(
    request: GetUserOperationReceiptRequest,
    _session_id: &str,
) -> HyperwalletResponse {
    info!(
        "Getting UserOperation receipt for {}",
        request.user_op_hash
    );

    match bundler::get_user_operation_receipt(request.user_op_hash.clone()) {
        Ok(receipt) => {
            info!(
                "UserOperation {} confirmed in transaction {}",
                receipt.user_op_hash, receipt.transaction_hash
            );

            // Build response with transaction hash for proof of payment
            let response_json = json!({
                "userOpHash": receipt.user_op_hash,
                "transactionHash": receipt.transaction_hash,
                "success": receipt.success,
                "actualGasUsed": receipt.actual_gas_used,
                "actualGasCost": receipt.actual_gas_cost,
                "receipt": receipt.raw_receipt,
            });

            let response = UserOperationReceiptResponse {
                user_op_hash: receipt.user_op_hash,
                status: if receipt.success { "success" } else { "failed" }.to_string(),
                receipt: Some(response_json.to_string()),
            };

            HyperwalletResponse::success(HyperwalletResponseData::GetUserOperationReceipt(response))
        }
        Err(e) => {
            error!("Failed to get UserOperation receipt: {:?}", e);
            HyperwalletResponse::error(e)
        }
    }
}

/// Submit UserOperation and wait for confirmation
/// This combines submission and receipt fetching for a complete payment flow
pub fn submit_and_wait_for_confirmation(
    submit_request: SubmitUserOperationRequest,
    max_wait_seconds: u64,
) -> HyperwalletResponse {
    info!("Submitting UserOperation and waiting for confirmation");

    // First submit the UserOperation
    let user_op_hash = match bundler::submit_user_operation(
        serde_json::from_str(&submit_request.signed_user_operation).unwrap_or_else(|_| serde_json::Value::String(submit_request.signed_user_operation.clone())),
        submit_request.entry_point.clone(),
    ) {
        Ok(hash) => {
            info!("UserOperation submitted: {}", hash);
            hash
        }
        Err(e) => {
            error!("Failed to submit UserOperation: {:?}", e);
            return HyperwalletResponse::error(e);
        }
    };

    // Then wait for confirmation
    match bundler::get_user_operation_receipt_wait(user_op_hash.clone(), max_wait_seconds) {
        Ok(receipt) => {
            info!(
                "Payment confirmed! UserOp {} in transaction {}",
                receipt.user_op_hash, receipt.transaction_hash
            );

            let response_json = json!({
                "userOpHash": receipt.user_op_hash,
                "transactionHash": receipt.transaction_hash,
                "success": receipt.success,
                "actualGasUsed": receipt.actual_gas_used,
                "actualGasCost": receipt.actual_gas_cost,
                "receipt": receipt.raw_receipt,
            });

            let response = UserOperationReceiptResponse {
                user_op_hash: receipt.user_op_hash,
                status: if receipt.success {
                    "confirmed"
                } else {
                    "failed"
                }
                .to_string(),
                receipt: Some(response_json.to_string()),
            };

            HyperwalletResponse::success(HyperwalletResponseData::GetUserOperationReceipt(response))
        }
        Err(e) => {
            error!("Failed to get confirmation: {:?}", e);
            // Return partial success with just the userOp hash
            let response = UserOperationReceiptResponse {
                user_op_hash,
                status: "pending".to_string(),
                receipt: None,
            };
            HyperwalletResponse::success(HyperwalletResponseData::GetUserOperationReceipt(response))
        }
    }
}

// ============================================================================
// Core Data Structures
// ============================================================================

/// Context for building a UserOperation
struct OperationContext {
    sender: EthAddress,
    entry_point: EthAddress,
    provider: Provider,
    nonce: U256,
    call_data: Vec<u8>,
    gas_prices: GasPrices,
    paymaster_data: Vec<u8>,
    paymaster_config: Option<PaymasterConfig>,
    signer: Box<dyn Signer>,
}

/// Gas pricing information
struct GasPrices {
    max_fee: U256,
    priority_fee: U256,
}

/// Optimized gas limits for a UserOperation
struct GasLimits {
    call: U256,
    verification: U256,
    pre_verification: U256,
}

// ============================================================================
// Operation Context Preparation
// ============================================================================

fn prepare_operation_context(
    request: &BuildAndSignUserOperationForPaymentRequest,
    address: &Address,
    state: &HyperwalletState,
) -> Result<OperationContext, OperationError> {
    // Parse sender address
    let sender = EthAddress::from_str(&request.tba_address)
        .map_err(|_| OperationError::invalid_params("Invalid TBA address"))?;

    // Get chain configuration
    let chain_id = DEFAULT_CHAIN_ID;
    let entry_point = get_entry_point_address(chain_id)
        .ok_or_else(|| OperationError::internal_error("Failed to get entry point address"))?;
    let provider = Provider::new(chain_id as u64, 30);

    // Fetch nonce and gas prices
    let nonce = fetch_nonce(&provider, sender, entry_point)?;
    let gas_prices = fetch_gas_prices(&provider)?;

    // Parse calldata
    let call_data = hex::decode(request.call_data.trim_start_matches("0x"))
        .map_err(|_| OperationError::invalid_params("Invalid call data hex"))?;

    // Build paymaster data if needed
    let (paymaster_data, paymaster_config) = if request.use_paymaster {
        let config = override_paymaster_config(request.paymaster_config.clone());
        let data = build_paymaster_data(&config)?;
        (data, Some(config))
    } else {
        (Vec::new(), None)
    };

    // Get signer
    let wallet_info = state
        .get_wallet(address, &request.eoa_wallet_id)
        .ok_or_else(|| OperationError::invalid_params("Wallet not found"))?;

    let password_value = request
        .password
        .as_ref()
        .map(|s| serde_json::Value::String(s.clone()));

    let signer = get_signer_from_wallet(wallet_info, password_value.as_ref()).map_err(|e| {
        e.error
            .unwrap_or_else(|| OperationError::internal_error("Failed to get signer"))
    })?;

    Ok(OperationContext {
        sender,
        entry_point,
        provider,
        nonce,
        call_data,
        gas_prices,
        paymaster_data,
        paymaster_config,
        signer: Box::new(signer),
    })
}

// ============================================================================
// Gas Estimation and Optimization
// ============================================================================

fn estimate_optimized_gas(context: &OperationContext) -> Result<GasLimits, OperationError> {
    // Build estimation request with high limits to avoid AA26 errors
    let estimation_limits = GasLimits {
        call: U256::from(300_000),
        verification: U256::from(200_000),
        pre_verification: U256::from(100_000),
    };

    let estimation_request = build_estimation_user_op(context, &estimation_limits)?;

    // Get estimates from bundler
    let gas_estimates =
        bundler::estimate_user_operation_gas(estimation_request, context.entry_point.to_string())?;

    // Detect operation type and apply smart optimization
    let operation_type = detect_operation_type(&context.call_data);
    info!("Detected operation type: {:?}", operation_type);

    let estimated_call = parse_gas_estimate(&gas_estimates, "callGasLimit");
    let estimated_verification = parse_gas_estimate(&gas_estimates, "verificationGasLimit");
    let estimated_pre_verification = parse_gas_estimate(&gas_estimates, "preVerificationGas");

    let (call, verification, pre_verification) = apply_smart_gas_limits(
        estimated_call,
        estimated_verification,
        estimated_pre_verification,
        &operation_type,
    );

    log_gas_optimization(
        estimated_call,
        estimated_verification,
        estimated_pre_verification,
        call,
        verification,
        pre_verification,
    );

    Ok(GasLimits {
        call,
        verification,
        pre_verification,
    })
}

fn parse_gas_estimate(estimates: &serde_json::Value, field: &str) -> Option<U256> {
    estimates
        .get(field)
        .and_then(|v| v.as_str())
        .and_then(|s| U256::from_str_radix(s.trim_start_matches("0x"), 16).ok())
}

fn log_gas_optimization(
    est_call: Option<U256>,
    est_verif: Option<U256>,
    est_pre: Option<U256>,
    actual_call: U256,
    actual_verif: U256,
    actual_pre: U256,
) {
    if let (Some(ec), Some(ev), Some(ep)) = (est_call, est_verif, est_pre) {
        let estimated_total = ec + ev + ep;
        let actual_total = actual_call + actual_verif + actual_pre;
        let saved = estimated_total.saturating_sub(actual_total);

        info!(
            "Gas optimization: Candide estimated {}/{}/{}, using {}/{}/{} (saved {} gas)",
            ec, ev, ep, actual_call, actual_verif, actual_pre, saved
        );
    }
}

// ============================================================================
// UserOperation Building and Signing
// ============================================================================

fn build_and_sign_user_op(
    context: &OperationContext,
    gas_limits: &GasLimits,
) -> Result<PackedUserOperation, OperationError> {
    // Build the UserOperation
    let user_op = build_packed_user_operation(
        context.sender,
        context.nonce,
        context.call_data.clone(),
        gas_limits,
        &context.gas_prices,
        context.paymaster_data.clone(),
    );

    // Sign it
    let hash = get_user_op_hash(&context.provider, &user_op, context.entry_point)?;
    let signature = context
        .signer
        .sign_hash(&hash)
        .map_err(|e| OperationError::internal_error(&format!("Failed to sign: {}", e)))?;

    Ok(PackedUserOperation {
        signature: signature.into(),
        ..user_op
    })
}

fn build_packed_user_operation(
    sender: EthAddress,
    nonce: U256,
    call_data: Vec<u8>,
    gas_limits: &GasLimits,
    gas_prices: &GasPrices,
    paymaster_data: Vec<u8>,
) -> PackedUserOperation {
    // Pack gas limits (verification << 128 | call)
    let mut account_gas_limits = [0u8; 32];
    account_gas_limits[..16].copy_from_slice(&gas_limits.verification.to_be_bytes::<32>()[16..]);
    account_gas_limits[16..].copy_from_slice(&gas_limits.call.to_be_bytes::<32>()[16..]);

    // Pack gas fees (priority << 128 | max)
    let mut gas_fees = [0u8; 32];
    gas_fees[..16].copy_from_slice(&gas_prices.priority_fee.to_be_bytes::<32>()[16..]);
    gas_fees[16..].copy_from_slice(&gas_prices.max_fee.to_be_bytes::<32>()[16..]);

    PackedUserOperation {
        sender,
        nonce,
        initCode: Vec::new().into(),
        callData: call_data.into(),
        accountGasLimits: account_gas_limits.into(),
        preVerificationGas: gas_limits.pre_verification,
        gasFees: gas_fees.into(),
        paymasterAndData: paymaster_data.into(),
        signature: Vec::new().into(),
    }
}

// ============================================================================
// Chain Interaction Functions
// ============================================================================

fn fetch_nonce(
    provider: &Provider,
    sender: EthAddress,
    entry_point: EthAddress,
) -> Result<U256, OperationError> {
    sol! {
        function getNonce(address sender, uint192 key) external view returns (uint256 nonce);
    }

    let call = getNonceCall {
        sender: sender.to_string().parse().unwrap(),
        key: alloy_primitives::U256::ZERO,
    };

    let tx_req = TransactionRequest::default()
        .input(TransactionInput::new(call.abi_encode().into()))
        .to(entry_point);

    provider
        .call(tx_req, None)
        .map(|bytes| U256::from_be_slice(&bytes))
        .map_err(|e| OperationError::internal_error(&format!("Failed to fetch nonce: {}", e)))
}

fn fetch_gas_prices(provider: &Provider) -> Result<GasPrices, OperationError> {
    let block = provider
        .get_block_by_number(BlockNumberOrTag::Latest, false)
        .map_err(|e| OperationError::internal_error(&format!("Failed to get block: {}", e)))?
        .ok_or_else(|| OperationError::internal_error("No latest block found"))?;

    let base_fee = block.header.inner.base_fee_per_gas.unwrap_or(1_000_000_000) as u128;

    // Cost optimization: minimal priority fee
    let priority_fee = 10_000_000u128; // 0.01 gwei

    // Spike protection: 2x buffer
    let buffer = base_fee * 2;
    let max_fee = (base_fee + priority_fee + buffer).min(500_000_000_000u128);

    info!(
        "Gas pricing: base={} gwei, priority={} gwei, max={} gwei",
        base_fee / 1_000_000_000,
        priority_fee / 1_000_000_000,
        max_fee / 1_000_000_000
    );

    Ok(GasPrices {
        max_fee: U256::from(max_fee),
        priority_fee: U256::from(priority_fee),
    })
}

fn get_user_op_hash(
    provider: &Provider,
    user_op: &PackedUserOperation,
    entry_point: EthAddress,
) -> Result<Vec<u8>, OperationError> {
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

    let packed_user_op = PackedUserOperation {
        sender: user_op.sender.to_string().parse().unwrap(),
        nonce: user_op.nonce,
        initCode: AlloyBytes::from(user_op.initCode.to_vec()),
        callData: AlloyBytes::from(user_op.callData.to_vec()),
        accountGasLimits: FixedBytes::from_slice(user_op.accountGasLimits.as_ref()),
        preVerificationGas: user_op.preVerificationGas,
        gasFees: FixedBytes::from_slice(user_op.gasFees.as_ref()),
        paymasterAndData: AlloyBytes::from(user_op.paymasterAndData.to_vec()),
        signature: AlloyBytes::new(),
    };

    let call = getUserOpHashCall {
        userOp: packed_user_op,
    };

    let tx_req = TransactionRequest::default()
        .input(TransactionInput::new(call.abi_encode().into()))
        .to(entry_point);

    provider
        .call(tx_req, None)
        .map(|bytes| {
            info!("Got UserOp hash from EntryPoint: 0x{}", hex::encode(&bytes));
            bytes.to_vec()
        })
        .map_err(|e| OperationError::internal_error(&format!("Failed to get UserOp hash: {}", e)))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn override_paymaster_config(config: Option<PaymasterConfig>) -> PaymasterConfig {
    let mut config = config.unwrap_or_else(PaymasterConfig::default);
    // Force reasonable gas limits
    config.paymaster_verification_gas = "0x13880".to_string(); // 80,000
    config.paymaster_post_op_gas = "0xc350".to_string(); // 50,000
    config
}

fn build_paymaster_data(config: &PaymasterConfig) -> Result<Vec<u8>, OperationError> {
    let address_bytes = hex::decode(config.paymaster_address.trim_start_matches("0x"))
        .map_err(|_| OperationError::invalid_params("Invalid paymaster address"))?;

    if address_bytes.len() != 20 {
        return Err(OperationError::invalid_params(
            "Paymaster address must be 20 bytes",
        ));
    }

    let verif_gas = U256::from_str_radix(
        config.paymaster_verification_gas.trim_start_matches("0x"),
        16,
    )
    .map_err(|_| OperationError::invalid_params("Invalid verification gas"))?;

    let post_gas = U256::from_str_radix(config.paymaster_post_op_gas.trim_start_matches("0x"), 16)
        .map_err(|_| OperationError::invalid_params("Invalid post-op gas"))?;

    let mut data = Vec::new();
    data.extend_from_slice(&address_bytes);
    data.extend_from_slice(&verif_gas.to_be_bytes::<32>()[16..]);
    data.extend_from_slice(&post_gas.to_be_bytes::<32>()[16..]);
    Ok(data)
}

fn build_estimation_user_op(
    context: &OperationContext,
    gas_limits: &GasLimits,
) -> Result<serde_json::Value, OperationError> {
    // Get a hash for signing
    let temp_op = build_packed_user_operation(
        context.sender,
        context.nonce,
        context.call_data.clone(),
        gas_limits,
        &context.gas_prices,
        context.paymaster_data.clone(),
    );

    let hash = get_user_op_hash(&context.provider, &temp_op, context.entry_point)?;
    let signature = context
        .signer
        .sign_hash(&hash)
        .map_err(|e| OperationError::internal_error(&format!("Failed to sign: {}", e)))?;

    Ok(json!({
        "sender": context.sender.to_string(),
        "nonce": format!("0x{:x}", context.nonce),
        "callData": format!("0x{}", hex::encode(&context.call_data)),
        "callGasLimit": format!("0x{:x}", gas_limits.call),
        "verificationGasLimit": format!("0x{:x}", gas_limits.verification),
        "preVerificationGas": format!("0x{:x}", gas_limits.pre_verification),
        "maxFeePerGas": format!("0x{:x}", context.gas_prices.max_fee),
        "maxPriorityFeePerGas": format!("0x{:x}", context.gas_prices.priority_fee),
        "factory": serde_json::Value::Null,
        "factoryData": serde_json::Value::Null,
        "paymaster": context.paymaster_config.as_ref()
            .map(|c| c.paymaster_address.clone())
            .unwrap_or_default(),
        "paymasterVerificationGasLimit": "0x13880",
        "paymasterPostOpGasLimit": "0xc350",
        "paymasterData": "0x",
        "signature": format!("0x{}", hex::encode(&signature))
    }))
}

fn format_user_op_response(
    user_op: &PackedUserOperation,
    context: &OperationContext,
) -> serde_json::Value {
    // Unpack gas limits for Candide format
    let verification_gas = U256::from_be_slice(&user_op.accountGasLimits[0..16]);
    let call_gas = U256::from_be_slice(&user_op.accountGasLimits[16..32]);
    let priority_fee = U256::from_be_slice(&user_op.gasFees[0..16]);
    let max_fee = U256::from_be_slice(&user_op.gasFees[16..32]);

    json!({
        "sender": user_op.sender.to_string(),
        "nonce": format!("0x{:x}", user_op.nonce),
        "callData": format!("0x{}", hex::encode(&user_op.callData)),
        "callGasLimit": format!("0x{:x}", call_gas),
        "verificationGasLimit": format!("0x{:x}", verification_gas),
        "preVerificationGas": format!("0x{:x}", user_op.preVerificationGas),
        "maxFeePerGas": format!("0x{:x}", max_fee),
        "maxPriorityFeePerGas": format!("0x{:x}", priority_fee),
        "signature": format!("0x{}", hex::encode(&user_op.signature)),
        "factory": serde_json::Value::Null,
        "factoryData": serde_json::Value::Null,
        "paymaster": context.paymaster_config.as_ref()
            .map(|c| c.paymaster_address.clone())
            .unwrap_or_default(),
        "paymasterVerificationGasLimit": "0x13880",
        "paymasterPostOpGasLimit": "0xc350",
        "paymasterData": "0x"
    })
}

// ============================================================================
// Gas Cost Estimation (for external use)
// ============================================================================

/// Estimate the gas cost in USDC for a payment operation
pub fn estimate_payment_gas_cost_usdc(
    tba_address: &str,
    usdc_contract: &str,
    recipient_address: &str,
    amount_usdc: u128,
) -> Result<f64, String> {
    info!(
        "Estimating gas cost for USDC payment from TBA {}",
        tba_address
    );

    // Parse addresses
    let sender = EthAddress::from_str(tba_address).map_err(|_| "Invalid TBA address")?;
    let usdc_addr = usdc_contract
        .parse::<EthAddress>()
        .map_err(|_| "Invalid USDC contract address")?;
    let recipient_addr = recipient_address
        .parse::<EthAddress>()
        .map_err(|_| "Invalid recipient address")?;

    // Create payment calldata
    let erc20_calldata = hyperware_process_lib::wallet::create_erc20_transfer_calldata(
        recipient_addr,
        U256::from(amount_usdc),
    );
    let tba_calldata = hyperware_process_lib::wallet::create_tba_userop_calldata(
        usdc_addr,
        U256::ZERO,
        erc20_calldata,
        0,
    );

    // Get chain configuration
    let chain_id = DEFAULT_CHAIN_ID;
    let entry_point =
        get_entry_point_address(chain_id).ok_or("Failed to get entry point address")?;
    let provider = Provider::new(chain_id as u64, 30);

    // Fetch current state
    let nonce = fetch_nonce(&provider, sender, entry_point)
        .map_err(|e| format!("Failed to fetch nonce: {:?}", e))?;
    let gas_prices =
        fetch_gas_prices(&provider).map_err(|e| format!("Failed to fetch gas prices: {:?}", e))?;

    // Build estimation request with high limits
    let estimation_request = json!({
        "sender": sender.to_string(),
        "nonce": format!("0x{:x}", nonce),
        "callData": format!("0x{}", hex::encode(&tba_calldata)),
        "callGasLimit": "0x493e0",        // 300,000
        "verificationGasLimit": "0x30d40", // 200,000
        "preVerificationGas": "0x186a0",   // 100,000
        "maxFeePerGas": format!("0x{:x}", gas_prices.max_fee),
        "maxPriorityFeePerGas": format!("0x{:x}", gas_prices.priority_fee),
        "factory": serde_json::Value::Null,
        "factoryData": serde_json::Value::Null,
        "paymaster": "0x0578cFB241215b77442a541325d6A4E6dFE700Ec",
        "paymasterVerificationGasLimit": "0x13880",
        "paymasterPostOpGasLimit": "0xc350",
        "paymasterData": "0x",
        "signature": format!("0x{}", hex::encode(vec![0u8; 65]))
    });

    // Get gas estimates
    let gas_estimates =
        bundler::estimate_user_operation_gas(estimation_request, entry_point.to_string())
            .map_err(|e| format!("Failed to estimate gas: {:?}", e))?;

    // Optimize gas limits
    let operation_type = detect_operation_type(&tba_calldata);
    let estimated_call = parse_gas_estimate(&gas_estimates, "callGasLimit");
    let estimated_verification = parse_gas_estimate(&gas_estimates, "verificationGasLimit");
    let estimated_pre_verification = parse_gas_estimate(&gas_estimates, "preVerificationGas");

    let (call_gas, verification_gas, pre_verification_gas) = apply_smart_gas_limits(
        estimated_call,
        estimated_verification,
        estimated_pre_verification,
        &operation_type,
    );

    // Calculate total gas including paymaster
    let paymaster_verification = U256::from(80_000);
    let paymaster_post_op = U256::from(50_000);

    let total_gas = call_gas
        + verification_gas
        + pre_verification_gas
        + paymaster_verification
        + paymaster_post_op;

    // Calculate ETH cost
    let gas_cost_wei = total_gas.saturating_mul(gas_prices.max_fee);
    let eth_decimals = U256::from(10).pow(U256::from(18));
    let gas_cost_eth = gas_cost_wei.to::<u128>() as f64 / eth_decimals.to::<u128>() as f64;

    // Convert to USD (hardcoded for now, should fetch from oracle)
    let eth_price_usd = 3500.0;
    let gas_cost_usd = gas_cost_eth * eth_price_usd;

    info!(
        "Gas estimation: {} total gas, {:.6} ETH, ${:.2} USD",
        total_gas, gas_cost_eth, gas_cost_usd
    );

    Ok(gas_cost_usd)
}
