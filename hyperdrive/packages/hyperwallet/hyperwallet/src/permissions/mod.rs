use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use hyperware_process_lib::hyperwallet_client::types::Operation;

/// Process permissions - defines what operations a process can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPermissions {
    /// The process address this permission set is for
    pub process_address: String,
    
    /// Operations this process is allowed to perform
    pub allowed_operations: HashSet<Operation>,
    
    /// Optional spending limits
    pub spending_limits: Option<SpendingLimits>,
    
    /// Settings that the process can update itself
    pub updatable_settings: Vec<UpdatableSetting>,
    
    /// When these permissions were registered
    pub registered_at: u64,
}

/// Spending limits for a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingLimits {
    /// Maximum ETH per transaction
    pub per_tx_eth: Option<String>,
    
    /// Maximum ETH per day
    pub daily_eth: Option<String>,
    
    /// Maximum USDC per transaction
    pub per_tx_usdc: Option<String>,
    
    /// Maximum USDC per day
    pub daily_usdc: Option<String>,
    
    /// Reset time for daily limits (Unix timestamp)
    pub daily_reset_at: u64,
    
    /// Accumulated spending today
    pub spent_today_eth: String,
    pub spent_today_usdc: String,
}

/// Settings that a process can update about itself
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdatableSetting {
    SpendingLimits,
    // Future: could add more like rate limits, allowed chains, etc.
}

impl ProcessPermissions {
    /// Create new permissions for a process
    pub fn new(process_address: String, operations: Vec<Operation>) -> Self {
        Self {
            process_address,
            allowed_operations: operations.into_iter().collect(),
            spending_limits: None,
            updatable_settings: vec![],
            registered_at: chrono::Utc::now().timestamp() as u64,
        }
    }
    
    /// Check if this process can perform an operation
    pub fn can_perform(&self, operation: &Operation) -> bool {
        self.allowed_operations.contains(operation)
    }
    
    /// Add spending limits
    pub fn with_spending_limits(mut self, limits: SpendingLimits) -> Self {
        self.spending_limits = Some(limits);
        self
    }
    
    /// Add updatable settings
    pub fn with_updatable_settings(mut self, settings: Vec<UpdatableSetting>) -> Self {
        self.updatable_settings = settings;
        self
    }
    
    /// Check if a setting can be updated by this process
    pub fn can_update(&self, setting: &UpdatableSetting) -> bool {
        self.updatable_settings.contains(setting)
    }
}

impl Default for SpendingLimits {
    fn default() -> Self {
        Self {
            per_tx_eth: None,
            daily_eth: None,
            per_tx_usdc: None,
            daily_usdc: None,
            daily_reset_at: 0,
            spent_today_eth: "0".to_string(),
            spent_today_usdc: "0".to_string(),
        }
    }
}

/// Check and update spending limits
impl SpendingLimits {
    /// Check if a transaction would exceed limits
    pub fn check_transaction(&self, amount: &str, is_eth: bool) -> Result<(), String> {
        let (per_tx_limit, daily_limit, spent_today) = if is_eth {
            (&self.per_tx_eth, &self.daily_eth, &self.spent_today_eth)
        } else {
            (&self.per_tx_usdc, &self.daily_usdc, &self.spent_today_usdc)
        };
        
        // Check per-transaction limit
        if let Some(limit) = per_tx_limit {
            // Parse amounts for comparison
            let amount_f: f64 = amount.parse().map_err(|_| "Invalid amount")?;
            let limit_f: f64 = limit.parse().map_err(|_| "Invalid limit")?;
            if amount_f > limit_f {
                return Err(format!("Transaction exceeds per-tx limit of {}", limit));
            }
        }
        
        // Check daily limit
        if let Some(daily_limit) = daily_limit {
            // Parse amounts (simplified - in production use proper decimal handling)
            let amount_f: f64 = amount.parse().map_err(|_| "Invalid amount")?;
            let spent_f: f64 = spent_today.parse().map_err(|_| "Invalid spent amount")?;
            let limit_f: f64 = daily_limit.parse().map_err(|_| "Invalid limit")?;
            
            if amount_f + spent_f > limit_f {
                return Err(format!("Transaction would exceed daily limit of {}", daily_limit));
            }
        }
        
        Ok(())
    }
    
    /// Update spending after a successful transaction
    pub fn record_spending(&mut self, amount: &str, is_eth: bool) {
        // Check if we need to reset daily limits
        let now = chrono::Utc::now().timestamp() as u64;
        if now >= self.daily_reset_at + 86400 {
            // Reset daily counters
            self.spent_today_eth = "0".to_string();
            self.spent_today_usdc = "0".to_string();
            self.daily_reset_at = now;
        }
        
        // Update spent amount
        let spent_today = if is_eth {
            &mut self.spent_today_eth
        } else {
            &mut self.spent_today_usdc
        };
        
        // Parse and add (simplified - in production use proper decimal handling)
        if let (Ok(spent_f), Ok(amount_f)) = (spent_today.parse::<f64>(), amount.parse::<f64>()) {
            *spent_today = (spent_f + amount_f).to_string();
        }
    }
}

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

pub mod validator; 