use crate::types::core::ProcessId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// IPC Requests for the notifications:distro:sys runtime module.
#[derive(Serialize, Deserialize, Debug)]
pub enum NotificationsAction {
    /// Send a push notification
    SendNotification {
        subscription: PushSubscription,
        title: String,
        body: String,
        icon: Option<String>,
        data: Option<serde_json::Value>,
    },
    /// Get the public key for VAPID authentication
    GetPublicKey,
    /// Initialize or regenerate VAPID keys
    InitializeKeys,
}

/// Push subscription information from the client
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PushSubscription {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

/// Responses for the notifications:distro:sys runtime module.
#[derive(Serialize, Deserialize, Debug)]
pub enum NotificationsResponse {
    NotificationSent,
    PublicKey(String),
    KeysInitialized,
    Err(NotificationsError),
}

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum NotificationsError {
    #[error("failed to send notification: {error}")]
    SendError { error: String },
    #[error("failed to generate VAPID keys: {error}")]
    KeyGenerationError { error: String },
    #[error("failed to load keys from state: {error}")]
    StateError { error: String },
    #[error("bad request error: {error}")]
    BadRequest { error: String },
    #[error("Bad JSON blob: {error}")]
    BadJson { error: String },
    #[error("VAPID keys not initialized")]
    KeysNotInitialized,
    #[error("web push error: {error}")]
    WebPushError { error: String },
    #[error("unauthorized request from {process}")]
    Unauthorized { process: ProcessId },
}

impl NotificationsError {
    pub fn kind(&self) -> &str {
        match *self {
            NotificationsError::SendError { .. } => "SendError",
            NotificationsError::KeyGenerationError { .. } => "KeyGenerationError",
            NotificationsError::StateError { .. } => "StateError",
            NotificationsError::BadRequest { .. } => "BadRequest",
            NotificationsError::BadJson { .. } => "BadJson",
            NotificationsError::KeysNotInitialized => "KeysNotInitialized",
            NotificationsError::WebPushError { .. } => "WebPushError",
            NotificationsError::Unauthorized { .. } => "Unauthorized",
        }
    }
}
