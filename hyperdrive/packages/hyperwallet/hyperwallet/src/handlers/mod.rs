/// Message handlers for different request types

use crate::state::HyperwalletState;
use hyperware_process_lib::Address;
use anyhow::Result;

pub mod http;
pub mod process;
pub mod terminal;
pub mod api;

/// Trait for handling messages from different sources
pub trait MessageHandler {
    /// Handle a message from a source address
    fn handle(
        &self,
        source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> Result<()>;
}

// Re-export handler types for convenience
pub use http::HttpHandler;
pub use process::ProcessHandler;
pub use terminal::TerminalHandler;
