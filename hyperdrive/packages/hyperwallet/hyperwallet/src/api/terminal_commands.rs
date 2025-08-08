/// Terminal debug command handler

// MessageHandler trait definition moved here since it's no longer shared
pub trait MessageHandler {
    fn handle(
        &self,
        source: &hyperware_process_lib::Address,
        body: Vec<u8>,
        state: &mut crate::state::HyperwalletState,
    ) -> anyhow::Result<()>;
}
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

    fn handle_command(
        &self,
        body: &[u8],
        source: &Address,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        let bod = String::from_utf8(body.to_vec())?;
        let command_parts: Vec<&str> = bod.splitn(2, ' ').collect();
        let command_verb = command_parts[0];
        let command_arg = command_parts.get(1).copied();

        match command_verb {
            "state" => self.show_state(state),
            "clear-process-perms" => self.clear_process_perms(source, state),
            "create-wallet" => self.create_wallet(command_arg, source, state),
            "list-wallets" => self.list_wallets(source, state),
            "get-balance" => self.get_balance(command_arg, source, state),
            "permissions" => self.show_permissions(state),
            "export-wallet" => self.export_wallet(command_arg, source, state),
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

    fn clear_process_perms(&self, process: &Address, state: &mut HyperwalletState) {
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

    fn create_wallet(
        &self,
        name_arg: Option<&str>,
        address: &Address,
        state: &mut HyperwalletState,
    ) {
        if let Some(name) = name_arg {
            info!("Creating test wallet with name: {}", name);
            match LocalSigner::new_random(8453) {
                Ok(signer) => {
                    let eth_address = signer.address().to_string();

                    let wallet = Wallet {
                        address: eth_address,
                        name: Some(name.to_string()),
                        chain_id: 8453,
                        key_storage: KeyStorage::Decrypted(signer),
                        created_at: chrono::Utc::now(),
                        last_used: None,
                        spending_limits: None,
                    };

                    state.add_wallet(address.clone(), wallet);
                    info!("Created wallet: {} with address: {}", name, address);
                }
                Err(e) => error!("Failed to create wallet: {}", e),
            }
        } else {
            error!("Usage: create-wallet <name>");
        }
    }

    fn list_wallets(&self, address: &Address, state: &HyperwalletState) {
        info!("--- Wallets for {} ---", address);
        let wallets = state.list_wallets(address);
        for wallet in wallets {
            info!(
                "Address: {}, Name: {:?}, Chain: {}, Encrypted: {}",
                wallet.address,
                wallet.name,
                wallet.chain_id,
                matches!(wallet.key_storage, KeyStorage::Encrypted(_))
            );
        }
    }

    fn get_balance(&self, wallet_id: Option<&str>, address: &Address, state: &HyperwalletState) {
        if let Some(wallet_id) = wallet_id {
            match state.get_wallet(address, wallet_id) {
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

    fn export_wallet(&self, wallet_id: Option<&str>, address: &Address, state: &HyperwalletState) {
        if let Some(wallet_id) = wallet_id {
            match state.get_wallet(address, wallet_id) {
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
        info!("permissions        : Show all process permissions");
        info!("export-wallet <id> : Export wallet private key (⚠️  SENSITIVE)");
        info!("chains             : List supported chains");
        info!("help or ?          : Show this help message");
        info!("-----------------------------------");
    }
}

impl MessageHandler for TerminalHandler {
    fn handle(
        &self,
        source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        self.handle_command(&body, source, state)
    }
}
