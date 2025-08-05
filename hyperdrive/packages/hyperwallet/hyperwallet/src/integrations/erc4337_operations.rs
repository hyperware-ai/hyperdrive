use hyperware_process_lib::hyperwallet_client::types::{
    HyperwalletResponse, HyperwalletResponseData, OperationError, SessionId,
    BuildAndSignUserOperationForPaymentRequest, BuildAndSignUserOperationResponse,
    SubmitUserOperationRequest, SubmitUserOperationResponse,
    GetUserOperationReceiptRequest, UserOperationReceiptResponse,
    PaymasterConfig};
use hyperware_process_lib::Address;
// TODO: These are legacy types - need to be migrated to new typed approach
#[derive(serde::Serialize, serde::Deserialize)]
pub struct OperationRequest {
    pub params: serde_json::Value,
    pub wallet_id: Option<String>,
    pub chain_id: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct OperationResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<OperationError>,
}

impl OperationResponse {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    
    pub fn error(error: OperationError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
    
    // Helper to convert from HyperwalletResponse<T> to OperationResponse
    pub fn from_hyperwallet_response(response: hyperware_process_lib::hyperwallet_client::types::HyperwalletResponse) -> Self {
        if response.success {
            if let Some(data) = response.data {
                match serde_json::to_value(data) {
                    Ok(json_data) => Self::success(json_data),
                    Err(_) => Self::error(OperationError::internal_error("Failed to serialize response data"))
                }
            } else {
                Self::success(serde_json::Value::Null)
            }
        } else {
            Self::error(response.error.unwrap_or_else(|| 
                OperationError::internal_error("Unknown error")
            ))
        }
    }
}
use crate::state::HyperwalletState;
use crate::core::transactions::{get_signer_from_wallet, sign_hash};
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
use crate::integrations::erc4337_bundler as bundler;

use crate::config::DEFAULT_CHAIN_ID;

pub fn build_and_sign_user_operation_for_payment(
    request: BuildAndSignUserOperationForPaymentRequest,
    session_id: &SessionId,
    address: &Address,
    state: &HyperwalletState,
) -> HyperwalletResponse {
    info!("Building UserOperation for payment");

    let sender = EthAddress::from_str(&request.tba_address).unwrap();

    let chain_id = DEFAULT_CHAIN_ID;
    let entry_point = get_entry_point_address(chain_id)
        .expect("Failed to get entry point address for the specified chain");

    let provider = Provider::new(chain_id as u64, 30);

    let nonce = match fetch_nonce(&provider, sender, entry_point) {
        Ok(n) => n,
        Err(e) => return HyperwalletResponse::error(OperationError::internal_error(&format!("Failed to fetch nonce: {}", e.error.as_ref().map(|e| &e.message).unwrap_or(&"Unknown error".to_string())))),
    };
    let (max_fee_per_gas, max_priority_fee_per_gas) = match fetch_gas_prices(&provider) {
        Ok(fees) => fees,
        Err(e) => return HyperwalletResponse::error(OperationError::internal_error(&format!("Failed to fetch gas prices: {}", e.error.as_ref().map(|e| &e.message).unwrap_or(&"Unknown error".to_string())))),
    };

    let mut call_gas_limit = U256::from(300_000);
    let mut verification_gas_limit = U256::from(150_000);
    let mut pre_verification_gas = U256::from(100_000);

    let call_data_bytes = match parse_hex(&request.call_data) {
        Ok(bytes) => bytes,
        Err(e) => return HyperwalletResponse::error(e.error.unwrap_or_else(|| OperationError::invalid_params("Failed to parse call data"))),
    };

    // Capture paymaster config early to avoid moving it
    let paymaster_config = request.paymaster_config;
    let use_paymaster = request.use_paymaster;

    let paymaster_and_data = if use_paymaster {
        let default_config = PaymasterConfig::default();
        let config = paymaster_config.as_ref().unwrap_or(&default_config);
        match build_circle_paymaster_data(config) {
            Ok(data) => data,
            Err(e) => return HyperwalletResponse::error(e.error.unwrap_or_else(|| OperationError::invalid_params("Failed to build paymaster data"))),
        }
    } else {
        Vec::new()
    };

    let wallet_info = match state.get_wallet(address, &request.eoa_wallet_id) {
        Some(info) => info,
        None => return HyperwalletResponse::error(OperationError::invalid_params("Wallet not found")),
    };

    let password_value = request.password.as_ref().map(|s| serde_json::Value::String(s.clone()));
    let signer = match get_signer_from_wallet(wallet_info, password_value.as_ref()) {
        Ok(s) => s,
        Err(e) => return HyperwalletResponse::error(e.error.unwrap_or_else(|| OperationError::internal_error("Failed to get signer"))),
    };

    let estimation_request = match build_estimation_request(
        &provider,
        sender,
        nonce,
        &call_data_bytes,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        &paymaster_and_data,
        entry_point,
        &signer,
        &paymaster_config,
    ) {
        Ok(req) => req,
        Err(e) => return HyperwalletResponse::error(e.error.unwrap_or_else(|| OperationError::invalid_params("Failed to build estimation request"))),
    };
    
    let gas_estimates = bundler::estimate_user_operation_gas(estimation_request, entry_point.to_string())
        .unwrap_or_default();

    // Update gas limits
    call_gas_limit = gas_estimates.get("callGasLimit")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_u256(s).ok())
        .map(|gas| gas + U256::from(10000))
        .unwrap_or(call_gas_limit);
    verification_gas_limit = gas_estimates.get("verificationGasLimit")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_u256(s).ok())
        .map(|gas| gas + U256::from(25000))  // Increased buffer for Candide requirements
        .unwrap_or(verification_gas_limit);
    pre_verification_gas = gas_estimates.get("preVerificationGas")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_u256(s).ok())
        .map(|gas| gas + U256::from(5000))
        .unwrap_or(pre_verification_gas);

    // Build and sign UserOperation
    let user_op = build_user_operation(
        sender,
        nonce,
        call_data_bytes.clone(),
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data.clone(),
    );
    let signature = match sign_user_op(&provider, &user_op, entry_point, &signer) {
        Ok(sig) => sig,
        Err(e) => return HyperwalletResponse::error(e.error.unwrap_or_else(|| OperationError::internal_error("Failed to sign UserOp"))),
    };

    let signed_user_op = PackedUserOperation {
        signature: signature.into(),
        ..user_op
    };

    info!("Built and signed UserOperation for payment from TBA {}", signed_user_op.sender);

    // Return response - unpack the gas limits for Candide compatibility
    let verification_gas_limit = U256::from_be_slice(&signed_user_op.accountGasLimits[0..16]);
    let call_gas_limit = U256::from_be_slice(&signed_user_op.accountGasLimits[16..32]);
    let max_priority_fee_per_gas = U256::from_be_slice(&signed_user_op.gasFees[0..16]);
    let max_fee_per_gas = U256::from_be_slice(&signed_user_op.gasFees[16..32]);
    
    let signed_user_op_json = serde_json::json!({
        "sender": signed_user_op.sender.to_string(),
        "nonce": format!("0x{:x}", signed_user_op.nonce),
        "callData": format!("0x{}", hex::encode(&signed_user_op.callData)),
        "callGasLimit": format!("0x{:x}", call_gas_limit),
        "verificationGasLimit": format!("0x{:x}", verification_gas_limit),
        "preVerificationGas": format!("0x{:x}", signed_user_op.preVerificationGas),
        "maxFeePerGas": format!("0x{:x}", max_fee_per_gas),
        "maxPriorityFeePerGas": format!("0x{:x}", max_priority_fee_per_gas),
        "signature": format!("0x{}", hex::encode(&signed_user_op.signature)),
        "factory": serde_json::Value::Null,
        "factoryData": serde_json::Value::Null,
        "paymaster": "0x0578cFB241215b77442a541325d6A4E6dFE700Ec",
        "paymasterVerificationGasLimit": "0x7a120",
        "paymasterPostOpGasLimit": "0x493e0", 
        "paymasterData": "0x"
    });
    
    let response_data = BuildAndSignUserOperationResponse {
        signed_user_operation: signed_user_op_json,
        entry_point: entry_point.to_string(),
        ready_to_submit: true,
    };
    
    HyperwalletResponse::success(HyperwalletResponseData::BuildAndSignUserOperationForPayment(response_data))
}

fn parse_hex(hex: &str) -> Result<Vec<u8>, OperationResponse> {
    hex::decode(hex.trim_start_matches("0x")).map_err(|_| OperationResponse::error(OperationError::invalid_params("Invalid hex string")))
}

fn fetch_nonce(provider: &Provider, sender: EthAddress, entry_point: EthAddress) -> Result<U256, OperationResponse> {
    fetch_nonce_from_entry_point(provider, sender, entry_point)
        .map_err(|e| OperationResponse::error(OperationError::internal_error(&format!("Failed to fetch nonce: {}", e))))
}

fn fetch_gas_prices(provider: &Provider) -> Result<(U256, U256), OperationResponse> {
    fetch_dynamic_gas_prices(provider)
        .map(|(max_fee, priority_fee)| (U256::from(max_fee), U256::from(priority_fee)))
        .map_err(|e| OperationResponse::error(OperationError::internal_error(&format!("Failed to fetch gas prices: {}", e))))
}

fn build_circle_paymaster_data(config: &PaymasterConfig) -> Result<Vec<u8>, OperationResponse> {
    let paymaster_bytes = parse_hex(&config.paymaster_address)?;
    if paymaster_bytes.len() != 20 {
        return Err(OperationResponse::error(OperationError::invalid_params("Paymaster address must be 20 bytes")));
    }
    let verif_gas = parse_u256(&config.paymaster_verification_gas)?;
    let post_op_gas = parse_u256(&config.paymaster_post_op_gas)?;
    let mut data = Vec::new();
    data.extend_from_slice(&paymaster_bytes);
    data.extend_from_slice(&verif_gas.to_be_bytes::<32>()[16..]);
    data.extend_from_slice(&post_op_gas.to_be_bytes::<32>()[16..]);
    Ok(data)
}

fn parse_u256(hex: &str) -> Result<U256, OperationResponse> {
    U256::from_str_radix(hex.trim_start_matches("0x"), 16)
        .map_err(|_| OperationResponse::error(OperationError::invalid_params("Invalid U256 hex string")))
}

fn build_estimation_request(
    provider: &Provider,
    sender: EthAddress,
    nonce: U256,
    call_data: &[u8],
    call_gas_limit: U256,
    verification_gas_limit: U256,
    pre_verification_gas: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    paymaster_and_data: &[u8],
    entry_point: EthAddress,
    signer: &impl Signer,
    paymaster_config: &Option<PaymasterConfig>,
) -> Result<serde_json::Value, OperationResponse> {
    let hash = match get_user_op_hash_for_estimation(
        provider,
        sender,
        nonce,
        call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data,
        entry_point,
    ) {
        Ok(h) => h,
        Err(e) => return Err(OperationResponse::error(OperationError::internal_error(&e))),
    };
    
    let signature = match signer.sign_hash(&hash) {
        Ok(sig) => sig,
        Err(e) => return Err(OperationResponse::error(OperationError::internal_error(&format!("Failed to sign: {}", e)))),
    };
    
    let default_config = PaymasterConfig::default();
    let config = paymaster_config.as_ref().unwrap_or(&default_config);
    Ok(serde_json::json!({
            "sender": sender.to_string(),
            "nonce": format!("0x{:x}", nonce),
        "callData": format!("0x{}", hex::encode(call_data)),
            "callGasLimit": format!("0x{:x}", call_gas_limit),
            "verificationGasLimit": format!("0x{:x}", verification_gas_limit),
            "preVerificationGas": format!("0x{:x}", pre_verification_gas),
            "maxFeePerGas": format!("0x{:x}", max_fee_per_gas),
            "maxPriorityFeePerGas": format!("0x{:x}", max_priority_fee_per_gas),
                "factory": serde_json::Value::Null,
                "factoryData": serde_json::Value::Null,
        "paymaster": config.paymaster_address.clone(),
        "paymasterVerificationGasLimit": config.paymaster_verification_gas.clone(),
        "paymasterPostOpGasLimit": config.paymaster_post_op_gas.clone(),
            "paymasterData": "0x",
        "signature": format!("0x{}", hex::encode(&signature))
    }))
}

fn build_user_operation(
    sender: EthAddress,
    nonce: U256,
    call_data: Vec<u8>,
    call_gas_limit: U256,
    verification_gas_limit: U256,
    pre_verification_gas: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    paymaster_and_data: Vec<u8>,
) -> PackedUserOperation {
    let mut account_gas_limits = [0u8; 32];
    account_gas_limits[..16].copy_from_slice(&verification_gas_limit.to_be_bytes::<32>()[16..]);
    account_gas_limits[16..].copy_from_slice(&call_gas_limit.to_be_bytes::<32>()[16..]);
    let mut gas_fees = [0u8; 32];
    gas_fees[..16].copy_from_slice(&max_priority_fee_per_gas.to_be_bytes::<32>()[16..]);
    gas_fees[16..].copy_from_slice(&max_fee_per_gas.to_be_bytes::<32>()[16..]);
    PackedUserOperation {
        sender,
        nonce,
        initCode: Vec::new().into(),
        callData: call_data.into(),
        accountGasLimits: account_gas_limits.into(),
        preVerificationGas: pre_verification_gas,
        gasFees: gas_fees.into(),
        paymasterAndData: paymaster_and_data.into(),
        signature: Vec::new().into(),
    }
}

fn sign_user_op(
    provider: &Provider,
    user_op: &PackedUserOperation,
    entry_point: EthAddress,
    signer: &impl Signer,
) -> Result<Vec<u8>, OperationResponse> {
    let hash = get_user_op_hash(provider, user_op, entry_point)
        .map_err(|e| OperationResponse::error(OperationError::internal_error(&format!("Failed to get UserOp hash: {}", e))))?;
    signer.sign_hash(&hash)
        .map_err(|e| OperationResponse::error(OperationError::internal_error(&format!("Failed to sign UserOp: {}", e))))
}

fn build_response(
    signed_user_op: PackedUserOperation,
    entry_point: EthAddress,
    chain_id: u64,
    use_paymaster: bool,
    paymaster_config: &Option<PaymasterConfig>,
) -> OperationResponse {
    if use_paymaster {
        let default_config = PaymasterConfig::default();
        let config = paymaster_config.as_ref().unwrap_or(&default_config);
        
        // Extract gas limits from packed format
        let verification_gas_limit = U256::from_be_slice(&signed_user_op.accountGasLimits[0..16]);
        let call_gas_limit = U256::from_be_slice(&signed_user_op.accountGasLimits[16..32]);
        let max_priority_fee_per_gas = U256::from_be_slice(&signed_user_op.gasFees[0..16]);
        let max_fee_per_gas = U256::from_be_slice(&signed_user_op.gasFees[16..32]);
        
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
                "factory": serde_json::Value::Null,
                "factoryData": serde_json::Value::Null,
                "paymaster": config.paymaster_address.clone(),
                "paymasterVerificationGasLimit": config.paymaster_verification_gas.clone(),
                "paymasterPostOpGasLimit": config.paymaster_post_op_gas.clone(),
                "paymasterData": "0x"
            },
            "entry_point": entry_point.to_string(),
            "chain_id": chain_id,
            "ready_to_submit": true,
        }))
    } else {
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

#[derive(Debug, Deserialize)]
pub struct SubmitUserOpParams {
    pub signed_user_operation: serde_json::Value,
    pub entry_point: String,
    pub bundler_url: Option<String>,
}

pub fn submit_user_operation(
    req: SubmitUserOperationRequest,
    session_id: &SessionId,
) -> HyperwalletResponse {
    info!("Submitting UserOperation to bundler ^^");
    
    match bundler::submit_user_operation(
        req.signed_user_operation.clone(),
        req.entry_point.clone(),
    ) {
        Ok(user_op_hash) => {
            info!("UserOperation submitted successfully: {}", user_op_hash);
            HyperwalletResponse::success(HyperwalletResponseData::SubmitUserOperation(SubmitUserOperationResponse {
                user_op_hash,
            }))
        }
        Err(e) => {
            error!("Failed to submit UserOperation: {:?}", e);
            HyperwalletResponse::error(e)
        }
    }
}

pub fn get_user_operation_receipt(
    req: GetUserOperationReceiptRequest,
    session_id: &SessionId,
) -> HyperwalletResponse {
    info!("Getting UserOperation receipt");
    
    match bundler::get_user_operation_receipt(req.user_op_hash.clone()) {
        Ok(receipt_data) => {
            info!("UserOperation receipt retrieved successfully");
            HyperwalletResponse::success(HyperwalletResponseData::GetUserOperationReceipt(UserOperationReceiptResponse {
                user_op_hash: req.user_op_hash.clone(),
                status: "success".to_string(),
                receipt: Some(receipt_data),
            }))
        }
        Err(e) => {
            error!("Failed to get UserOperation receipt: {:?}", e);
            HyperwalletResponse::error(e)
        }
    }
}

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

fn fetch_dynamic_gas_prices(provider: &Provider) -> Result<(u128, u128), String> {
    use hyperware_process_lib::eth::BlockNumberOrTag;
    
    match provider.get_block_by_number(BlockNumberOrTag::Latest, false) {
        Ok(Some(block)) => {
            let base_fee = block.header.inner.base_fee_per_gas.unwrap_or(1_000_000_000) as u128;
            
            // Calculate priority fee first (at least 0.1 gwei, or 10% of base fee)
            let priority_fee = std::cmp::max(100_000_000u128, base_fee / 10);
            
            // Calculate max fee to ensure: maxFeePerGas >= maxPriorityFeePerGas + baseFee + buffer
            // Formula: maxFee = baseFee + priorityFee + buffer
            let buffer = std::cmp::max(base_fee / 4, 50_000_000u128); // At least 25% of base fee or 0.05 gwei buffer
            let max_fee = base_fee + priority_fee + buffer;
            
            // Ensure max_fee is reasonable (cap at 100 gwei to avoid extreme gas spikes)
            let max_fee = std::cmp::min(max_fee, 100_000_000_000u128);
            
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