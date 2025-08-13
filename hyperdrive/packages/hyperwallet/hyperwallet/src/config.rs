/// Configuration constants and types for the Hyperwallet service

// Service metadata
pub const SERVICE_NAME: &str = "hyperwallet";
pub const SERVICE_VERSION: &str = "0.1.0";

// Chain configurations
pub const DEFAULT_CHAIN_ID: u64 = 8453; // Base
pub const SUPPORTED_CHAIN_IDS: &[u64] = &[1, 8453]; // Ethereum mainnet, Base
pub const DEFAULT_GAS_LIMIT: u64 = 21000; // Standard ETH transfer

// Contract addresses
pub const ENTRY_POINT_ADDRESS: &str = "0x4337084D9E255Ff0702461CF8895CE9E3b5Ff108"; // ERC-4337 EntryPoint on Base
pub const CIRCLE_PAYMASTER_BASE: &str = "0x0578cFB241215b77442a541325d6A4E6dFE700Ec"; // Circle's USDC paymaster on Base
                                                                                      //pub const PIMLICO_PAYMASTER_BASE: &str = "0x888888888888Ec68A58AB8094Cc1AD20Ba3D2402";
pub const USDC_ADDRESS_BASE: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"; // USDC on Base

// Bundler configuration
pub const PIMLICO_BASE_URL: &str = "https://api.pimlico.io/v2/base/rpc"; // Pimlico bundler endpoint for Base
pub const BUNDLER_TIMEOUT_MS: u64 = 30000; // 30 second timeout for bundler operations

// Operation defaults
pub const DEFAULT_OPERATION_TIMEOUT: u64 = 30000;

// Process addresses
pub const TERMINAL_PROCESS: &str = "terminal:terminal:sys";
pub const TESTER_PROCESS_PATTERN: &str = "*:tester:sys";
pub const OPERATOR_PROCESS: &str = "operator:operator:grid-beta.hypr";

// Wallet management
pub const WALLET_ID_PREFIX_LENGTH: usize = 8;

/// HTTP server configuration
pub const HTTP_BIND_AUTHENTICATED: bool = true;

/// Anvil test network constants
#[cfg(test)]
pub mod test_constants {
    /// Anvil test network chain ID
    pub const ANVIL_CHAIN_ID: u64 = 31337;

    /// Anvil test accounts (first 3 accounts)
    pub const ANVIL_ACCOUNT_0: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    pub const ANVIL_PRIVATE_KEY_0: &str =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    pub const ANVIL_ACCOUNT_1: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
    pub const ANVIL_PRIVATE_KEY_1: &str =
        "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

    pub const ANVIL_ACCOUNT_2: &str = "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC";
    pub const ANVIL_PRIVATE_KEY_2: &str =
        "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
}
