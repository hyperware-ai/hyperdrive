# Hyperwallet - Wallet Service for Hyperware

A process-isolated service that provides cryptographic operations to Hyperware applications.

## Quick Start

To use Hyperwallet, a process first initializes a session via a handshake, then uses the resulting session object to perform operations.

```rust
// In your process init() function
use hyperware_process_lib::hyperwallet_client::{self, HandshakeConfig, Operation, SpendingLimits};

// 1. Define the process's required operations.
const REQUIRED_OPERATIONS: &[Operation] = &[
    Operation::CreateWallet,
    Operation::GetBalance,
    Operation::SendEth,
];

let config = HandshakeConfig::new()
    .with_operations(REQUIRED_OPERATIONS)
    .with_spending_limits(SpendingLimits {
        per_tx_eth: Some("1.0".to_string()),
        ..Default::default()
    });

// 2. Perform the handshake to establish a session.
let session = hyperwallet_client::initialize(config)?;

// 3. Use the session object to perform operations.
let wallet = hyperwallet_client::create_wallet(&session, "main-wallet", None)?;
hyperwallet_client::send_eth(&session, &wallet.address, "bob.hypr", "0.1 ETH")?;
```

## Core Concepts

### Process Isolation

Every wallet is exclusively owned by the process that created it. There are no shared wallets between processes, ensuring complete isolation by default.

### Handshake Protocol

On startup, a process performs a multi-step handshake with Hyperwallet. This protocol is used to exchange version information, declare required permissions, and establish a temporary session for subsequent operations.

## Key Features

### Declarative Permissions

During the handshake, a client process declares its full set of required permissions. Hyperwallet uses this declaration to create or update the process's access rights. This allows an application's permissions to be updated simply by restarting the process after an upgrade.

### Session-Based Key Caching

The handshake returns a `session_id`. For encrypted wallets, this session can be used to cache decrypted keys. A client calls the `UnlockWallet` operation once with a password. For the remainder of the session, it can perform signing operations without resending the password, as Hyperwallet uses the `session_id` to access the cached key. Sessions expire after a period of inactivity.

## Security Model

The service's security is based on several mechanisms:

  * **Process Isolation**: Wallets are stored in a `HashMap` keyed by the owning process's address, preventing cross-process access.
  * **Declared Permissions**: Operations are limited to what the process declared during its most recent handshake.
  * **Session Timeouts**: Sessions expire after a configurable duration, removing any decrypted keys from the in-memory cache.
  * **Key Storage**: Private keys are encrypted at rest and never leave the Hyperwallet service process.

## Architecture

### State Structure

The `HyperwalletState` includes a non-persisted cache for active sessions in addition to the persisted wallet and permission data.

```rust
HyperwalletState {
    // Persisted: Wallets indexed by process address
    wallets_by_process: HashMap<ProcessAddress, HashMap<WalletAddress, Wallet>>,
    
    // Persisted: Permissions indexed by process address
    process_permissions: HashMap<ProcessAddress, ProcessPermissions>,
    
    // In-Memory: Decrypted signers for performance
    #[serde(skip)]
    active_signers: HashMap<...>,
    
    // In-Memory: Active session data indexed by session_id
    #[serde(skip)]
    active_sessions: HashMap<SessionId, SessionData>,
}
```

### Message Protocol

Communication is defined by the shared types in `hyperware_process_lib::hyperwallet_client`. The primary entrypoint for a new process is the `Handshake` operation.