use crate::permissions::definitions::{ProcessPermissions, SpendingLimits, UpdatableSetting};
use crate::state::HyperwalletState;
use hyperware_process_lib::hyperwallet_client::types::{
    HandshakeStep, HyperwalletResponse, HyperwalletResponseData, Operation,
    OperationError, SessionId, UnlockWalletRequest, UnlockWalletResponse,
};
use hyperware_process_lib::logging::{info, warn};
use hyperware_process_lib::Address;

pub fn handle_handshake_step(
    step: HandshakeStep,
    source: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    state.cleanup_expired_sessions();

    info!("Received handshake request {:?} from {:?}", step, source);

    match &step {
        HandshakeStep::ClientHello(hello) => {
            handle_client_hello(hello.client_version.clone(), hello.client_name.clone(), state)
        }
        HandshakeStep::Register(reg) => handle_register(
            reg.required_operations.clone(),
            reg.spending_limits.clone(),
            source,
            state,
        ),
        _ => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Invalid handshake step",
            ))
        }
    }
}

fn handle_client_hello(
    client_version: String,
    client_name: String,
    _state: &HyperwalletState,
) -> HyperwalletResponse {
    // ← Return specific type
    info!(
        "Received ClientHello from {} (version {})",
        client_name, client_version
    );

    if !is_compatible_version(&client_version) {
        warn!(
            "Version incompatibility: client {} vs server {}",
            client_version,
            env!("CARGO_PKG_VERSION")
        );
        return HyperwalletResponse::error(OperationError::invalid_params(&format!(
            "Version incompatibility: client {} vs server {}",
            client_version,
            env!("CARGO_PKG_VERSION")
        )));
    }

    let supported_operations = get_all_supported_operations();

    info!(
        "Sending ServerWelcome with {} operations and {} chains",
        supported_operations.len(),
        1
    );

    HyperwalletResponse::success(HyperwalletResponseData::Handshake(HandshakeStep::ServerWelcome(
        hyperware_process_lib::hyperwallet_client::types::ServerWelcome {
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            supported_operations,
            supported_chains: vec![8453], // Base
            features: vec![
                "spending_limits".to_string(),
                "session_management".to_string(),
                "erc4337".to_string(),
                "gasless_payments".to_string(),
            ],
        }
    )))
}

fn handle_register(
    required_operations: Vec<Operation>,
    spending_limits: Option<SpendingLimits>,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    // ← Return specific type
    info!(
        "Processing Register for {} with {} operations",
        address,
        required_operations.len()
    );

    let supported_ops = get_all_supported_operations();
    for op in &required_operations {
        if !supported_ops.contains(op) {
            return HyperwalletResponse::error(OperationError::invalid_params(&format!(
                "Operation {:?} is not supported by this server",
                op
            )));
        }
    }

    let mut permissions = ProcessPermissions::new(address.clone(), required_operations.clone());

    if let Some(limits) = spending_limits.clone() {
        permissions.spending_limits = Some(limits);
        permissions.updatable_settings = vec![UpdatableSetting::SpendingLimits];
    }

    let was_update = state.get_permissions(address).is_some();
    state.set_permissions(address.clone(), permissions.clone());

    let session_id = state.create_session(address, 0);

    info!(
        "{} process {} with {} operations, created session {}",
        if was_update { "Updated" } else { "Registered" },
        address,
        permissions.allowed_operations.len(),
        session_id
    );

    HyperwalletResponse::success(HyperwalletResponseData::Handshake(HandshakeStep::Complete(
        hyperware_process_lib::hyperwallet_client::types::CompleteHandshake {
            session_id,
            registered_permissions: permissions,
        }
    )))
}

pub fn handle_unlock_wallet(
    req: UnlockWalletRequest,
    _session_id: &SessionId,
    address: &Address,
    state: &mut HyperwalletState,
) -> HyperwalletResponse {
    state.cleanup_expired_sessions();

    match state.validate_session(&req.session_id) {
        Some(session) if session.process_address == *address => {}
        Some(_) => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Session does not belong to this process",
            ));
        }
        None => {
            return HyperwalletResponse::error(OperationError::invalid_params(
                "Invalid or expired session_id",
            ));
        }
    }

    let (wallet_address, key_storage) = match state.get_wallet(address, &req.wallet_id) {
        Some(wallet) => (wallet.address.clone(), wallet.key_storage.clone()),
        None => {
            return HyperwalletResponse::error(OperationError::wallet_not_found(&req.wallet_id));
        }
    };

    let result = match &key_storage {
        crate::state::KeyStorage::Encrypted(encrypted_data) => {
            match hyperware_process_lib::signer::LocalSigner::decrypt(
                encrypted_data,
                &req.password,
            ) {
                Ok(signer) => {
                    state
                        .active_signers
                        .insert((address.clone(), wallet_address.clone()), signer);

                    if let Err(e) =
                        state.add_unlocked_wallet(&req.session_id, wallet_address.clone())
                    {
                        return HyperwalletResponse::error(OperationError::internal_error(&e));
                    }

                    info!(
                        "Unlocked wallet {} for session {}",
                        wallet_address, req.session_id
                    );

                    HyperwalletResponse::success(HyperwalletResponseData::UnlockWallet(UnlockWalletResponse {
                        success: true,
                        wallet_id: req.wallet_id.clone(),
                        message: format!("Wallet {} unlocked successfully", wallet_address),
                    }))
                }
                Err(_) => HyperwalletResponse::error(OperationError::authentication_failed(
                    "Invalid password",
                )),
            }
        }
        crate::state::KeyStorage::Decrypted(_) => {
            if let Err(e) = state.add_unlocked_wallet(&req.session_id, wallet_address.clone()) {
                return HyperwalletResponse::error(OperationError::internal_error(&e));
            }

            info!(
                "Wallet {} was already unlocked, added to session {}",
                wallet_address, req.session_id
            );

            HyperwalletResponse::success(HyperwalletResponseData::UnlockWallet(UnlockWalletResponse {
                success: true,
                wallet_id: req.wallet_id.clone(),
                message: format!("Wallet {} was already unlocked", wallet_address),
            }))
        }
    };

    result
}

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

// this is a placeholder for now. it uses the first cargo.toml it sees
/// Check version compatibility
fn is_compatible_version(client_version: &str) -> bool {
    // For now, accept all versions but log warnings for major differences
    // TODO: Implement proper semantic versioning compatibility
    if client_version != env!("CARGO_PKG_VERSION") {
        warn!(
            "Client version {} differs from server version {}",
            client_version,
            env!("CARGO_PKG_VERSION")
        );
    }
    true
}
