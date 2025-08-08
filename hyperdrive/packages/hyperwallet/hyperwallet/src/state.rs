use crate::permissions::ProcessPermissions;
use chrono::{DateTime, Utc};
use hyperware_process_lib::hyperwallet_client::types::Operation;
use hyperware_process_lib::logging::{error, info};
use hyperware_process_lib::signer::{EncryptedSignerData, LocalSigner};
use hyperware_process_lib::wallet::KeyStorage as ProcessLibKeyStorage;
use hyperware_process_lib::Address;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type ProcessAddress = Address; // format: "node@process:package:publisher"
pub type WalletAddress = String; // Ethereum address (TODO: should use Alloy types in the future)
pub type ChainId = u64;
pub type SessionId = String; // Unique session identifier

#[derive(Debug, Clone)]
pub struct SessionData {
    pub process_address: ProcessAddress,
    pub unlocked_wallets: HashSet<WalletAddress>,
    pub expiry: std::time::Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperwalletState {
    pub wallets_by_process: HashMap<ProcessAddress, HashMap<WalletAddress, Wallet>>,

    pub process_permissions: HashMap<ProcessAddress, ProcessPermissions>,

    #[serde(skip)]
    pub active_signers: HashMap<(ProcessAddress, WalletAddress), LocalSigner>,

    #[serde(skip)]
    pub active_sessions: HashMap<SessionId, SessionData>,

    /// Hypermap data cache for performance
    pub identities: HashMap<String, Identity>, // entry_name -> TBA/owner info
    pub notes: HashMap<String, Vec<u8>>, // note_path -> raw data
    //pub facts: HashMap<String, Vec<u8>>,         // TODO: add facts
    pub pending_txs: HashMap<(ProcessAddress, WalletAddress), Vec<PendingTx>>,
    pub nonces: HashMap<(ProcessAddress, WalletAddress, ChainId), u64>,

    pub chains: HashMap<ChainId, Chain>,
    pub tokens: HashMap<ChainId, HashMap<String, Token>>,

    pub version: u32,
    pub initialized_at: u64,
}

impl Default for HyperwalletState {
    fn default() -> Self {
        let mut chains = HashMap::new();

        // Add default chain configurations
        chains.insert(
            8453,
            Chain {
                id: 8453,
                name: "Base".to_string(),
                rpc_url: "goes-here".to_string(), // TODO: not really needed
                block_explorer: "https://basescan.org".to_string(),
                native_currency: "ETH".to_string(),
                enabled: true,
            },
        );

        Self {
            wallets_by_process: HashMap::new(),
            process_permissions: HashMap::new(),
            active_signers: HashMap::new(),
            active_sessions: HashMap::new(),
            identities: HashMap::new(),
            notes: HashMap::new(),
            pending_txs: HashMap::new(),
            nonces: HashMap::new(),
            chains,
            tokens: HashMap::new(),
            version: 2, // Bumped for migration
            initialized_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub address: WalletAddress, // Primary identifier
    pub name: Option<String>,   // Optional friendly name
    pub chain_id: ChainId,
    pub key_storage: KeyStorage,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    /// Wallet-specific spending limits (optional, overrides process limits)
    pub spending_limits: Option<WalletSpendingLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyStorage {
    /// Encrypted private key data
    Encrypted(EncryptedSignerData),
    /// Unencrypted (in-memory only, for active wallets)
    Decrypted(LocalSigner),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSpendingLimits {
    pub max_per_call: Option<String>,
    pub max_total: Option<String>,
    pub currency: String,
    pub total_spent: String,
    pub set_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for WalletSpendingLimits {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            max_per_call: None,
            max_total: None,
            currency: "USDC".to_string(),
            total_spent: "0".to_string(),
            set_at: now,
            updated_at: now,
        }
    }
}

impl From<ProcessLibKeyStorage> for KeyStorage {
    fn from(storage: ProcessLibKeyStorage) -> Self {
        match storage {
            ProcessLibKeyStorage::Encrypted(data) => KeyStorage::Encrypted(data),
            ProcessLibKeyStorage::Decrypted(signer) => KeyStorage::Decrypted(signer),
        }
    }
}

impl From<KeyStorage> for ProcessLibKeyStorage {
    fn from(storage: KeyStorage) -> Self {
        match storage {
            KeyStorage::Encrypted(data) => ProcessLibKeyStorage::Encrypted(data),
            KeyStorage::Decrypted(signer) => ProcessLibKeyStorage::Decrypted(signer),
        }
    }
}

/// Hypermap identity information cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub entry_name: String,
    pub tba_address: String,
    pub owner_address: String,
    pub cached_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTx {
    pub id: String,
    pub chain_id: ChainId,
    pub operation: String,
    pub status: TxStatus,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TxStatus {
    Pending,
    Submitted { hash: String },
    Confirmed { hash: String, block: u64 },
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chain {
    pub id: ChainId,
    pub name: String,
    pub rpc_url: String,
    pub block_explorer: String,
    pub native_currency: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

impl HyperwalletState {
    pub fn initialize() -> Self {
        if let Some(saved_state) = Self::load() {
            info!(
                "Loaded existing wallet state (version {})",
                saved_state.version
            );

            // Migrate if needed
            if saved_state.version == 1 {
                info!("Migrating state from v1 to v2");
                return Self::migrate_v1_to_v2(saved_state);
            }

            return saved_state;
        }

        info!("Creating new wallet state");
        let mut state = Self::default();
        state.initialized_at = chrono::Utc::now().timestamp() as u64;
        state.save();
        state
    }

    pub fn clear_process_permissions(&mut self, process: &Address) {
        self.process_permissions.remove(&process);
        self.save();
    }

    /// Migrate from v1 (flat wallet storage) to v2 (process-isolated)
    fn migrate_v1_to_v2(v1_state: Self) -> Self {
        info!("Starting state migration from v1 to v2");

        let mut v2_state = Self::default();
        v2_state.initialized_at = v1_state.initialized_at;
        v2_state.chains = v1_state.chains;
        v2_state.tokens = v1_state.tokens;
        v2_state.identities = v1_state.identities;
        v2_state.notes = v1_state.notes;

        // Note: We can't migrate wallets without knowing their owners
        // They would need to be re-imported by their respective processes
        info!("Migration complete. Wallets will need to be re-imported by their processes.");

        v2_state.save();
        v2_state
    }

    pub fn load() -> Option<Self> {
        match hyperware_process_lib::get_state() {
            Some(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(state) => Some(state),
                Err(e) => {
                    error!("Failed to deserialize state: {}", e);

                    None
                }
            },
            None => None,
        }
    }

    pub fn save(&self) {
        match serde_json::to_vec(self) {
            Ok(bytes) => {
                hyperware_process_lib::set_state(&bytes);
                info!("State saved successfully");
            }
            Err(e) => {
                error!("Failed to serialize state: {}", e);
            }
        }
    }

    pub fn get_wallet(&self, address: &Address, wallet_identifier: &str) -> Option<&Wallet> {
        let process_wallets = self.wallets_by_process.get(address)?;

        if wallet_identifier.starts_with("0x") && wallet_identifier.len() == 42 {
            return process_wallets.get(wallet_identifier);
        }

        process_wallets
            .values()
            .find(|w| w.name.as_ref() == Some(&wallet_identifier.to_string()))
    }

    pub fn get_wallet_mut(
        &mut self,
        address: &Address,
        wallet_identifier: &str,
    ) -> Option<&mut Wallet> {
        let process_wallets = self.wallets_by_process.get_mut(address)?;

        if wallet_identifier.starts_with("0x") && wallet_identifier.len() == 42 {
            return process_wallets.get_mut(wallet_identifier);
        }

        let address = process_wallets
            .values()
            .find(|w| w.name.as_ref() == Some(&wallet_identifier.to_string()))
            .map(|w| w.address.clone())?;

        process_wallets.get_mut(&address)
    }

    pub fn list_wallets(&self, address: &Address) -> Vec<&Wallet> {
        self.wallets_by_process
            .get(address)
            .map(|wallets| wallets.values().collect())
            .unwrap_or_default()
    }

    pub fn add_wallet(&mut self, address: Address, wallet: Wallet) {
        let process_wallets = self
            .wallets_by_process
            .entry(address)
            .or_insert_with(HashMap::new);

        process_wallets.insert(wallet.address.clone(), wallet);
        self.save();
    }

    pub fn remove_wallet(
        &mut self,
        address: &Address,
        wallet_address: &WalletAddress,
    ) -> Option<Wallet> {
        let process_wallets = self.wallets_by_process.get_mut(address)?;
        let wallet = process_wallets.remove(wallet_address);

        if wallet.is_some() {
            if process_wallets.is_empty() {
                self.wallets_by_process.remove(address);
            }
            self.save();
        }

        wallet
    }

    pub fn get_permissions(&self, address: &Address) -> Option<&ProcessPermissions> {
        self.process_permissions.get(address)
    }

    pub fn set_permissions(&mut self, address: Address, permissions: ProcessPermissions) {
        self.process_permissions
            .insert(address.clone(), permissions);
        self.save();
    }

    pub fn check_permission(&self, address: &Address, operation: &Operation) -> bool {
        let Some(permissions) = self.get_permissions(address) else {
            return false;
        };

        permissions.allowed_operations.contains(operation)
    }

    pub fn check_wallet_ownership(&self, process: &Address, wallet_identifier: &str) -> bool {
        self.get_wallet(process, wallet_identifier).is_some()
    }

    pub fn set_wallet_spending_limits(
        &mut self,
        process: &Address,
        wallet_identifier: &str,
        limits: WalletSpendingLimits,
    ) -> Result<(), String> {
        let wallet = self
            .get_wallet_mut(process, wallet_identifier)
            .ok_or_else(|| {
                format!(
                    "Wallet '{}' not found for process '{}'",
                    wallet_identifier, process
                )
            })?;

        wallet.spending_limits = Some(limits);
        self.save();
        Ok(())
    }

    pub fn get_effective_spending_limits(
        &self,
        process: &Address,
        wallet_identifier: &str,
    ) -> Option<WalletSpendingLimits> {
        if let Some(wallet) = self.get_wallet(process, wallet_identifier) {
            // Return wallet-specific limits if they exist
            if let Some(wallet_limits) = &wallet.spending_limits {
                return Some(wallet_limits.clone());
            }
        }

        // Fall back to process-level limits (convert if needed)
        // For now, return None - could convert ProcessPermissions::SpendingLimits to WalletSpendingLimits
        None
    }

    /// Check if a spending amount is within wallet limits
    pub fn check_spending_limit(
        &self,
        process: &Address,
        wallet_identifier: &str,
        amount: &str,
        currency: &str,
    ) -> Result<bool, String> {
        let Some(wallet) = self.get_wallet(process, wallet_identifier) else {
            return Err(format!("Wallet '{}' not found", wallet_identifier));
        };

        let Some(limits) = &wallet.spending_limits else {
            // No limits set = unlimited
            return Ok(true);
        };

        // Check currency matches
        if limits.currency != currency {
            // Different currency = no limits apply
            return Ok(true);
        }

        // Parse amounts as f64 for comparison
        let amount_f64 = amount
            .parse::<f64>()
            .map_err(|_| format!("Invalid amount: {}", amount))?;

        // Check per-call limit
        if let Some(max_per_call) = &limits.max_per_call {
            let max_per_call_f64 = max_per_call
                .parse::<f64>()
                .map_err(|_| format!("Invalid max_per_call limit: {}", max_per_call))?;

            if amount_f64 > max_per_call_f64 {
                return Ok(false);
            }
        }

        // Check total limit
        if let Some(max_total) = &limits.max_total {
            let max_total_f64 = max_total
                .parse::<f64>()
                .map_err(|_| format!("Invalid max_total limit: {}", max_total))?;

            let total_spent_f64 = limits
                .total_spent
                .parse::<f64>()
                .map_err(|_| format!("Invalid total_spent: {}", limits.total_spent))?;

            if total_spent_f64 + amount_f64 > max_total_f64 {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn record_spending(
        &mut self,
        process: &Address,
        wallet_identifier: &str,
        amount: &str,
        currency: &str,
    ) -> Result<(), String> {
        let wallet = self
            .get_wallet_mut(process, wallet_identifier)
            .ok_or_else(|| format!("Wallet '{}' not found", wallet_identifier))?;

        if let Some(limits) = &mut wallet.spending_limits {
            if limits.currency == currency {
                let amount_f64 = amount
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid amount: {}", amount))?;

                let current_spent = limits.total_spent.parse::<f64>().unwrap_or(0.0);

                limits.total_spent = (current_spent + amount_f64).to_string();
                limits.updated_at = Utc::now();

                self.save();
            }
        }

        Ok(())
    }

    // === Session Management Methods ===

    pub fn create_session(&mut self, address: &Address, duration_secs: u64) -> SessionId {
        let session_id = self.generate_session_id();
        let expiry = if duration_secs == 0 {
            std::time::Instant::now() + std::time::Duration::from_secs(365 * 24 * 60 * 60 * 100)
        } else {
            std::time::Instant::now() + std::time::Duration::from_secs(duration_secs)
        };

        let session_data = SessionData {
            process_address: address.clone(),
            unlocked_wallets: HashSet::new(),
            expiry,
        };

        self.active_sessions
            .insert(session_id.clone(), session_data);
        let duration_msg = if duration_secs == 0 {
            "infinite".to_string()
        } else {
            format!("{}s", duration_secs)
        };
        info!(
            "Created new session {} for process {} (duration: {})",
            session_id, address, duration_msg
        );

        session_id
    }

    fn generate_session_id(&self) -> SessionId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .hash(&mut hasher);

        format!("sess_{:x}", hasher.finish())
    }

    pub fn validate_session(&mut self, session_id: &SessionId) -> Option<&SessionData> {
        self.cleanup_expired_sessions();

        self.active_sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &SessionId) -> Option<&mut SessionData> {
        self.cleanup_expired_sessions();
        self.active_sessions.get_mut(session_id)
    }

    pub fn cleanup_expired_sessions(&mut self) {
        let now = std::time::Instant::now();
        let expired_sessions: Vec<SessionId> = self
            .active_sessions
            .iter()
            .filter(|(_, session)| session.expiry <= now)
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in expired_sessions {
            if let Some(session) = self.active_sessions.remove(&session_id) {
                info!(
                    "Cleaned up expired session {} for process {}",
                    session_id, session.process_address
                );

                // Remove any cached signers for this session's process
                let process = &session.process_address;
                self.active_signers.retain(|(proc, _), _| proc != process);
            }
        }
    }

    pub fn remove_session(&mut self, session_id: &SessionId) -> Option<SessionData> {
        if let Some(session) = self.active_sessions.remove(session_id) {
            info!(
                "Removed session {} for process {}",
                session_id, session.process_address
            );

            // Remove any cached signers for this session's process
            let process = &session.process_address;
            self.active_signers.retain(|(proc, _), _| proc != process);

            Some(session)
        } else {
            None
        }
    }

    pub fn add_unlocked_wallet(
        &mut self,
        session_id: &SessionId,
        wallet_address: WalletAddress,
    ) -> Result<(), String> {
        let session = self
            .get_session_mut(session_id)
            .ok_or_else(|| format!("Session {} not found or expired", session_id))?;

        session.unlocked_wallets.insert(wallet_address.clone());
        info!(
            "Added unlocked wallet {} to session {}",
            wallet_address, session_id
        );

        Ok(())
    }

    pub fn is_wallet_unlocked(
        &self,
        session_id: &SessionId,
        wallet_address: &WalletAddress,
    ) -> bool {
        self.active_sessions
            .get(session_id)
            .map(|session| session.unlocked_wallets.contains(wallet_address))
            .unwrap_or(false)
    }

    pub fn active_session_count(&self) -> usize {
        self.active_sessions.len()
    }

    pub fn get_total_wallet_count(&self) -> usize {
        self.wallets_by_process
            .values()
            .map(|process_wallets| process_wallets.len())
            .sum()
    }

    pub fn get_active_process_count(&self) -> usize {
        self.process_permissions.len()
    }
}
