/// Terminal debug command handler

use super::MessageHandler;
use crate::operations::{Operation, OperationRequest, ProcessAuth};
use crate::permissions::validator::PermissionValidator;
use crate::state::{HyperwalletState, KeyStorage, Wallet};
use hyperware_process_lib::eth::Provider;
use hyperware_process_lib::logging::{error, info};
use hyperware_process_lib::signer::{LocalSigner, Signer};
use hyperware_process_lib::wallet as wallet_lib;
use hyperware_process_lib::Address;

pub struct TerminalHandler;

impl TerminalHandler {
    pub fn new() -> Self {
        Self
    }

    fn handle_command(&self, body: &[u8], state: &mut HyperwalletState) -> anyhow::Result<()> {
        let bod = String::from_utf8(body.to_vec())?;
        let command_parts: Vec<&str> = bod.splitn(2, ' ').collect();
        let command_verb = command_parts[0];
        let command_arg = command_parts.get(1).copied();

        // Terminal is always from terminal:terminal:sys
        let terminal_process = "terminal:terminal:sys";

        match command_verb {
            "state" => self.show_state(state),
            "clear-process-perms" => self.clear_process_perms(command_arg, state),
            "create-wallet" => self.create_wallet(command_arg, terminal_process, state),
            "list-wallets" => self.list_wallets(terminal_process, state),
            "get-balance" => self.get_balance(command_arg, terminal_process, state),
            "test-operation" => self.test_operation(state),
            "test-gasless" => self.test_gasless(command_arg, terminal_process, state),
            "permissions" => self.show_permissions(state),
            "export-wallet" => self.export_wallet(command_arg, terminal_process, state),
            "import-test" => self.import_test(terminal_process, state),
            "chains" => self.show_chains(state),
            "help" | "?" => self.show_help(),
            _ => {
                info!(
                    "Unknown command: '{}'. Type 'help' for available commands.",
                    command_verb
                );
            }
        }

        Ok(())
    }

    fn show_state(&self, state: &HyperwalletState) {
        info!("Hyperwallet state\n{:#?}", state);
    }

    fn clear_process_perms(&self, process: Option<&str>, state: &mut HyperwalletState) {
        let process = process.unwrap_or("");
        info!("Clearing process permissions for {}", process);
        
        match state.get_permissions(process) {
            Some(_perms) => {
                info!("Clearing process permissions for {}", process);
                state.clear_process_permissions(process);
            }
            None => {
                error!("Process permissions not found for {}", process);
            }
        }
    }

    fn create_wallet(&self, name_arg: Option<&str>, process: &str, state: &mut HyperwalletState) {
        if let Some(name) = name_arg {
            info!("Creating test wallet with name: {}", name);
            match LocalSigner::new_random(8453) {
                Ok(signer) => {
                    let address = signer.address().to_string();

                    let wallet = Wallet {
                        address: address.clone(),
                        name: Some(name.to_string()),
                        chain_id: 8453,
                        key_storage: KeyStorage::Decrypted(signer),
                        created_at: chrono::Utc::now(),
                        last_used: None,
                        spending_limits: None,
                    };

                    state.add_wallet(process, wallet);
                    info!("Created wallet: {} with address: {}", name, address);
                }
                Err(e) => error!("Failed to create wallet: {}", e),
            }
        } else {
            error!("Usage: create-wallet <name>");
        }
    }

    fn list_wallets(&self, process: &str, state: &HyperwalletState) {
        info!("--- Wallets for {} ---", process);
        let wallets = state.list_wallets(process);
        for wallet in wallets {
            info!(
                "Address: {}, Name: {:?}, Chain: {}, Encrypted: {}",
                wallet.address,
                wallet.name,
                wallet.chain_id,
                matches!(wallet.key_storage, KeyStorage::Encrypted(_))
            );
        }
        
        //// ??
        //if process == "terminal:terminal:sys" {
        //    let total_wallets: usize = state.wallets_by_process
        //        .values()
        //        .map(|wallets| wallets.len())
        //        .sum();
        //    info!("Total wallets across all processes: {}", total_wallets);
        //}
    }

    fn get_balance(&self, wallet_id: Option<&str>, process: &str, state: &HyperwalletState) {
        if let Some(wallet_id) = wallet_id {
            match state.get_wallet(process, wallet_id) {
                Some(wallet) => {
                    info!(
                        "Checking balance for wallet {} ({})",
                        wallet_id, wallet.address
                    );
                    match wallet_lib::get_eth_balance(
                        &wallet.address,
                        wallet.chain_id,
                        Provider::new(wallet.chain_id, 60000),
                    ) {
                        Ok(balance) => {
                            info!(
                                "Balance: {} (wei: {})",
                                balance.to_display_string(),
                                balance.as_wei()
                            );
                        }
                        Err(e) => error!("Failed to get balance: {}", e),
                    }
                }
                None => error!("Wallet not found: {}", wallet_id),
            }
        } else {
            error!("Usage: get-balance <wallet_id>");
        }
    }

    fn test_operation(&self, state: &mut HyperwalletState) {
        info!("Testing operation execution with terminal permissions...");

        // Create a test operation request
        let op_request = OperationRequest {
            operation: Operation::ListWallets,
            auth: ProcessAuth {
                process_address: "operator:operator:grid-beta.hypr".to_string(),
                signature: None,
            },
            params: serde_json::json!({}),
            wallet_id: None,
            chain_id: None,
            request_id: Some("test-123".to_string()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let validator = PermissionValidator::new();
        let response = validator.execute_with_permissions(op_request, "operator:operator:grid-beta.hypr", state);

        info!("Operation response: {:#?}", response);
    }

    fn test_gasless(&self, wallet_id: Option<&str>, process: &str, state: &mut HyperwalletState) {
        info!("Testing gasless transaction functionality...");
        
        if let Some(wallet_id) = wallet_id {
            // Check if wallet exists
            match state.get_wallet(process, wallet_id) {
                Some(wallet) => {
                    info!("Using wallet {} ({})", wallet_id, wallet.address);
                    
                    // Test building a UserOperation
                    let build_request = OperationRequest {
                        operation: Operation::BuildUserOperation,
                        auth: ProcessAuth {
                            process_address: process.to_string(),
                            signature: None,
                        },
                        params: serde_json::json!({
                            "target": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", // USDC on Base
                            "call_data": "0xa9059cbb0000000000000000000000003138fe02bfc273bff633e093bd914f58930d111c0000000000000000000000000000000000000000000000000000000000000001", // transfer 1 USDC
                            "value": "0",
                            "use_paymaster": true,
                        }),
                        wallet_id: Some(wallet_id.to_string()),
                        chain_id: Some(8453), // Base
                        request_id: Some("test-gasless-build".to_string()),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    let validator = PermissionValidator::new();
                    let build_response = validator.execute_with_permissions(build_request, process, state);
                    
                    if build_response.success {
                        info!("✅ UserOperation built successfully!");
                        if let Some(data) = &build_response.data {
                            info!("UserOp data: {:#?}", data);
                            
                            // Extract the user operation for signing
                            if let Some(user_op) = data.get("user_operation") {
                                info!("Testing UserOperation signing...");
                                
                                let sign_request = OperationRequest {
                                    operation: Operation::SignUserOperation,
                                    auth: ProcessAuth {
                                        process_address: process.to_string(),
                                        signature: None,
                                    },
                                    params: serde_json::json!({
                                        "user_operation": user_op,
                                        "entry_point": data.get("entry_point").unwrap_or(&serde_json::json!("")),
                                    }),
                                    wallet_id: Some(wallet_id.to_string()),
                                    chain_id: Some(8453),
                                    request_id: Some("test-gasless-sign".to_string()),
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                };
                                
                                let sign_response = validator.execute_with_permissions(sign_request, process, state);
                                
                                if sign_response.success {
                                    info!("✅ UserOperation signed successfully!");
                                    info!("Signed UserOp: {:#?}", sign_response.data);
                                } else {
                                    error!("❌ Failed to sign UserOperation: {:?}", sign_response.error);
                                }
                            }
                        }
                    } else {
                        error!("❌ Failed to build UserOperation: {:?}", build_response.error);
                    }
                }
                None => error!("Wallet not found: {}", wallet_id),
            }
        } else {
            error!("Usage: test-gasless <wallet_id>");
            info!("This command tests building and signing a gasless UserOperation");
        }
    }

    fn show_permissions(&self, state: &HyperwalletState) {
        info!("--- Process Permissions ---");
        for (process, perms) in &state.process_permissions {
            info!("Process: {}", process);
            info!(
                "  Allowed Operations: {} total",
                perms.allowed_operations.len()
            );
            for op in &perms.allowed_operations {
                info!("    - {:?}", op);
            }
            if let Some(limits) = &perms.spending_limits {
                info!("  Spending Limits:");
                if let Some(per_tx) = &limits.per_tx_eth {
                    info!("    Per TX ETH: {}", per_tx);
                }
                if let Some(daily) = &limits.daily_eth {
                    info!("    Daily ETH: {}", daily);
                }
            }
            info!("  Registered at: {}", perms.registered_at);
        }
    }

    fn export_wallet(&self, wallet_id: Option<&str>, process: &str, state: &HyperwalletState) {
        if let Some(wallet_id) = wallet_id {
            match state.get_wallet(process, wallet_id) {
                Some(wallet) => match &wallet.key_storage {
                    KeyStorage::Decrypted(signer) => {
                        let private_key = signer.export_private_key();
                        info!("Wallet {} private key: {}", wallet_id, private_key);
                        info!("⚠️  WARNING: This is sensitive data! Keep it secure!");
                    }
                    KeyStorage::Encrypted(_) => {
                        error!("Wallet {} is encrypted. Unlock it first.", wallet_id);
                    }
                },
                None => error!("Wallet not found: {}", wallet_id),
            }
        } else {
            error!("Usage: export-wallet <wallet_id>");
        }
    }

    fn import_test(&self, process: &str, state: &mut HyperwalletState) {
        info!("Importing test wallet...");
        let test_key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        match LocalSigner::from_private_key(test_key, 8453) {
            Ok(signer) => {
                let address = signer.address().to_string();

                let wallet = Wallet {
                    address: address.clone(),
                    name: Some("imported-test".to_string()),
                    chain_id: 8453,
                    key_storage: KeyStorage::Decrypted(signer),
                    created_at: chrono::Utc::now(),
                    last_used: None,
                    spending_limits: None,
                };

                state.add_wallet(process, wallet);
                info!(
                    "Imported test wallet with address: {}",
                    address
                );
            }
            Err(e) => error!("Failed to import test wallet: {}", e),
        }
    }

    fn show_chains(&self, state: &HyperwalletState) {
        info!("--- Supported Chains ---");
        for (chain_id, chain_info) in &state.chains {
            info!(
                "Chain ID: {}, Name: {}, RPC: {}",
                chain_id, chain_info.name, chain_info.rpc_url
            );
        }
    }

    fn show_help(&self) {
        info!("--- Hyperwallet Debug Commands ---");
        info!("state              : Print current state");
        info!("create-wallet <name>: Create a new test wallet");
        info!("list-wallets       : List all wallets");
        info!("get-balance <wallet_id>: Get ETH balance for a wallet");
        info!("test-operation     : Test operation execution");
        info!("test-gasless <wallet_id>: Test gasless UserOperation build/sign");
        info!("permissions        : Show all process permissions");
        info!("export-wallet <id> : Export wallet private key (⚠️  SENSITIVE)");
        info!("import-test        : Import a test wallet with known key");
        info!("chains             : List supported chains");
        info!("help or ?          : Show this help message");
        info!("-----------------------------------");
    }
}

impl MessageHandler for TerminalHandler {
    fn handle(
        &self,
        _source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        self.handle_command(&body, state)
    }
}
