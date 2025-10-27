/// External service integration modules
///
/// This module contains integrations with external services and protocols
pub mod erc4337_bundler;
pub mod erc4337_operations;
pub mod gas_optimization;
pub mod hypermap;

// Re-export public APIs
pub use erc4337_operations::*;
pub use hypermap::*;

// Internal modules (not re-exported)
// - erc4337_bundler: Internal bundler client implementation
// - gas_optimization: Internal gas limit optimization logic
