//! Types module layout:
//! - `model.rs`: core data models (messages, chats, settings, notifications).
//! - `api.rs`: wire request/response DTOs for HTTP/RPC/WebSocket/admin.
//! - `replication.rs`: replication queue/wake types and metrics.
//! - `state.rs`: ChatState, runtime channels, and serde wiring.
//! Re-exported for ergonomic `use crate::types::*`.

mod api;
mod model;
mod replication;
mod state;

pub use api::*;
pub use model::*;
pub use replication::*;
pub use state::*;
