use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use lib::types::core::{
    KernelMessage, LazyLoadBlob, Message, MessageReceiver, MessageSender, PrintSender, Printout,
    ProcessId, Request, Response, NOTIFICATIONS_PROCESS_ID,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use web_push::WebPushClient;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

// Import our types from lib
use lib::core::StateAction;
use lib::notifications::{
    NotificationsAction, NotificationsError, NotificationsResponse, PushSubscription,
};

/// VAPID keys for web push notifications
#[derive(Serialize, Deserialize, Clone)]
pub struct VapidKeys {
    pub public_key: String,
    pub private_key: String,
}

impl VapidKeys {
    /// Generate a new pair of VAPID keys
    pub fn generate() -> Result<Self, NotificationsError> {
        // Use a simple method to generate compatible keys
        // Generate random bytes for private key (32 bytes for P-256)
        let mut private_key_bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut private_key_bytes);

        // Use p256 crate to generate proper keys
        use p256::ecdsa::SigningKey;

        let signing_key = SigningKey::from_bytes(&private_key_bytes.into()).map_err(|e| {
            NotificationsError::KeyGenerationError {
                error: format!("Failed to create signing key: {:?}", e),
            }
        })?;

        let verifying_key = signing_key.verifying_key();
        let public_key_point = verifying_key.to_encoded_point(false); // false = uncompressed
        let public_key_bytes = public_key_point.as_bytes();

        if public_key_bytes.len() != 65 || public_key_bytes[0] != 0x04 {
            return Err(NotificationsError::KeyGenerationError {
                error: format!(
                    "Invalid public key format: len={}, first_byte=0x{:02x}",
                    public_key_bytes.len(),
                    if public_key_bytes.len() > 0 {
                        public_key_bytes[0]
                    } else {
                        0
                    }
                ),
            });
        }

        // Encode keys for storage
        let public_key = URL_SAFE_NO_PAD.encode(public_key_bytes);
        let private_key = URL_SAFE_NO_PAD.encode(&private_key_bytes);

        // Key generation logging moved to caller with send_to_terminal access

        Ok(VapidKeys {
            public_key,
            private_key,
        })
    }
}

use std::collections::VecDeque;

#[derive(Clone)]
pub struct QueuedNotification {
    title: String,
    body: String,
    icon: Option<String>,
    data: Option<serde_json::Value>,
}

pub struct NotificationsState {
    vapid_keys: Option<VapidKeys>,
    subscriptions: Vec<PushSubscription>,
    last_push_timestamp: Option<tokio::time::Instant>,
    notification_queue: VecDeque<QueuedNotification>,
    queue_processor_handle: Option<tokio::task::JoinHandle<()>>,
}

pub async fn notifications(
    our_node: Arc<String>,
    send_to_loop: MessageSender,
    send_to_terminal: PrintSender,
    mut recv_notifications: MessageReceiver,
    send_to_state: MessageSender,
) -> Result<(), anyhow::Error> {
    Printout::new(
        2,
        NOTIFICATIONS_PROCESS_ID.clone(),
        "notifications: starting notifications module".to_string(),
    )
    .send(&send_to_terminal)
    .await;

    let state = Arc::new(RwLock::new(NotificationsState {
        vapid_keys: None,
        subscriptions: Vec::new(),
        last_push_timestamp: None,
        notification_queue: VecDeque::new(),
        queue_processor_handle: None,
    }));

    // Try to load existing keys from state
    Printout::new(
        2,
        NOTIFICATIONS_PROCESS_ID.clone(),
        "notifications: loading keys from state".to_string(),
    )
    .send(&send_to_terminal)
    .await;
    load_keys_from_state(
        &our_node,
        &mut recv_notifications,
        &send_to_state,
        &send_to_loop,
        &send_to_terminal,
        &state,
    )
    .await;
    Printout::new(
        2,
        NOTIFICATIONS_PROCESS_ID.clone(),
        "notifications: finished loading keys from state".to_string(),
    )
    .send(&send_to_terminal)
    .await;

    while let Some(km) = recv_notifications.recv().await {
        if *our_node != km.source.node {
            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                format!(
                    "notifications: got request from {}, but requests must come from our node {our_node}",
                    km.source.node
                ),
            )
            .send(&send_to_terminal)
            .await;
            continue;
        }

        let state = state.clone();
        let our_node = our_node.clone();
        let send_to_loop = send_to_loop.clone();
        let send_to_terminal = send_to_terminal.clone();
        let send_to_state = send_to_state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_request(
                &our_node,
                km,
                &send_to_loop,
                &send_to_terminal,
                &send_to_state,
                &state,
            )
            .await
            {
                Printout::new(
                    0,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    format!("notifications: error handling request: {:?}", e),
                )
                .send(&send_to_terminal)
                .await;
            }
        });
    }

    Ok(())
}

async fn load_keys_from_state(
    our_node: &str,
    recv_notifications: &mut MessageReceiver,
    send_to_state: &MessageSender,
    send_to_loop: &MessageSender,
    send_to_terminal: &PrintSender,
    state: &Arc<RwLock<NotificationsState>>,
) {
    // Load VAPID keys
    let request_id = rand::random::<u64>();

    let km = KernelMessage::builder()
        .id(request_id)
        .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
        .target((our_node, ProcessId::new(Some("state"), "distro", "sys")))
        .message(Message::Request(Request {
            inherit: false,
            expects_response: Some(5),
            body: serde_json::to_vec(&StateAction::GetState(NOTIFICATIONS_PROCESS_ID.clone()))
                .unwrap(),
            metadata: None,
            capabilities: vec![],
        }))
        .build()
        .unwrap();

    km.send(send_to_state).await;

    // Wait for response with timeout
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                // Timeout reached, keys not found in state
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: no saved keys found in state, will generate on first use".to_string(),
                )
                .send(send_to_terminal)
                .await;
                break;
            }
            Some(km) = recv_notifications.recv() => {
                // Check if this is our response
                if km.id == request_id {
                    if let Message::Response((response, _context)) = km.message {
                        // Check if we got the state successfully
                        if let Ok(state_response) = serde_json::from_slice::<lib::core::StateResponse>(&response.body) {
                            match state_response {
                                lib::core::StateResponse::GetState => {
                                    // We got the state, deserialize the keys from context
                                    if let Some(blob) = km.lazy_load_blob {
                                        if let Ok(keys) = serde_json::from_slice::<VapidKeys>(&blob.bytes) {
                                            let mut state_guard = state.write().await;
                                            state_guard.vapid_keys = Some(keys);
                                            Printout::new(
                                                2,
                                                NOTIFICATIONS_PROCESS_ID.clone(),
                                                "notifications: loaded existing VAPID keys from state".to_string(),
                                            )
                                            .send(send_to_terminal)
                                            .await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    break;
                } else {
                    // Not our response, put it back for main loop to handle
                    km.send(send_to_loop).await;
                }
            }
        }
    }

    // Load subscriptions
    let request_id = rand::random::<u64>();

    let km = KernelMessage::builder()
        .id(request_id)
        .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
        .target((our_node, ProcessId::new(Some("state"), "distro", "sys")))
        .message(Message::Request(Request {
            inherit: false,
            expects_response: Some(5),
            body: serde_json::to_vec(&StateAction::GetState(ProcessId::new(
                Some("notifications-subscriptions"),
                "distro",
                "sys",
            )))
            .unwrap(),
            metadata: None,
            capabilities: vec![],
        }))
        .build()
        .unwrap();

    km.send(send_to_state).await;

    // Wait for response with timeout
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                // Timeout reached, no saved subscriptions
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: no saved subscriptions found in state".to_string(),
                )
                .send(send_to_terminal)
                .await;
                break;
            }
            Some(km) = recv_notifications.recv() => {
                // Check if this is our response
                if km.id == request_id {
                    if let Message::Response((response, _context)) = km.message {
                        // Check if we got the state successfully
                        if let Ok(state_response) = serde_json::from_slice::<lib::core::StateResponse>(&response.body) {
                            match state_response {
                                lib::core::StateResponse::GetState => {
                                    // We got the state, deserialize the subscriptions from context
                                    if let Some(blob) = km.lazy_load_blob {
                                        if let Ok(subscriptions) = serde_json::from_slice::<Vec<PushSubscription>>(&blob.bytes) {
                                            let mut state_guard = state.write().await;
                                            state_guard.subscriptions = subscriptions;
                                            Printout::new(
                                                2,
                                                NOTIFICATIONS_PROCESS_ID.clone(),
                                                format!("notifications: loaded {} existing subscriptions from state", state_guard.subscriptions.len()),
                                            )
                                            .send(send_to_terminal)
                                            .await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    break;
                } else {
                    // Not our response, put it back for main loop to handle
                    km.send(send_to_loop).await;
                }
            }
        }
    }
}

async fn handle_request(
    our_node: &str,
    km: KernelMessage,
    send_to_loop: &MessageSender,
    send_to_terminal: &PrintSender,
    send_to_state: &MessageSender,
    state: &Arc<RwLock<NotificationsState>>,
) -> Result<(), NotificationsError> {
    let KernelMessage {
        id,
        source,
        rsvp,
        message,
        ..
    } = km;

    let Message::Request(Request {
        expects_response,
        body,
        ..
    }) = message
    else {
        return Err(NotificationsError::BadRequest {
            error: "not a request".into(),
        });
    };

    let action: NotificationsAction =
        serde_json::from_slice(&body).map_err(|e| NotificationsError::BadJson {
            error: format!("parse into NotificationsAction failed: {:?}", e),
        })?;

    let response = match action {
        NotificationsAction::InitializeKeys => {
            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                "notifications: InitializeKeys action received".to_string(),
            )
            .send(send_to_terminal)
            .await;
            let keys = VapidKeys::generate()?;

            // Save keys to state
            save_keys_to_state(our_node, send_to_state, &keys).await?;

            // Update our state
            let mut state_guard = state.write().await;
            state_guard.vapid_keys = Some(keys);

            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                "notifications: Keys initialized successfully".to_string(),
            )
            .send(send_to_terminal)
            .await;
            NotificationsResponse::KeysInitialized
        }
        NotificationsAction::GetPublicKey => {
            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                format!("notifications: GetPublicKey action received from {:?}", source),
            )
            .send(send_to_terminal)
            .await;
            let state_guard = state.read().await;
            match &state_guard.vapid_keys {
                Some(keys) => {
                    NotificationsResponse::PublicKey(keys.public_key.clone())
                }
                None => {
                    Printout::new(
                        2,
                        NOTIFICATIONS_PROCESS_ID.clone(),
                        "notifications: no keys found, generating new ones".to_string(),
                    )
                    .send(send_to_terminal)
                    .await;
                    // Try to initialize keys
                    drop(state_guard);
                    let keys = VapidKeys::generate()?;
                    Printout::new(
                        2,
                        NOTIFICATIONS_PROCESS_ID.clone(),
                        format!("notifications: generated new keys, public key: {}", keys.public_key),
                    )
                    .send(send_to_terminal)
                    .await;
                    save_keys_to_state(our_node, send_to_state, &keys).await?;

                    let mut state_guard = state.write().await;
                    let public_key = keys.public_key.clone();
                    state_guard.vapid_keys = Some(keys);

                    NotificationsResponse::PublicKey(public_key)
                }
            }
        }
        NotificationsAction::SendNotification {
            title,
            body,
            icon,
            data,
        } => {
            // Add notification to queue
            let mut state_guard = state.write().await;

            // Check if we have keys and subscriptions
            if state_guard.vapid_keys.is_none() {
                return Err(NotificationsError::KeysNotInitialized);
            }

            if state_guard.subscriptions.is_empty() {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: No subscriptions available to send notification".to_string(),
                )
                .send(send_to_terminal)
                .await;
                return Ok(());
            }

            // Create queued notification
            let queued_notification = QueuedNotification {
                title,
                body,
                icon,
                data,
            };

            // Add to queue
            state_guard.notification_queue.push_back(queued_notification);
            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                format!("notifications: Added notification to queue, queue size: {}", state_guard.notification_queue.len()),
            )
            .send(send_to_terminal)
            .await;

            // Check if we need to start the queue processor
            if state_guard.queue_processor_handle.is_none() || state_guard.queue_processor_handle.as_ref().unwrap().is_finished() {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: Starting queue processor".to_string(),
                )
                .send(send_to_terminal)
                .await;

                // Clone what we need for the async task
                let state_clone = state.clone();
                let send_to_terminal_clone = send_to_terminal.clone();

                // Start the queue processor
                let handle = tokio::spawn(async move {
                    process_notification_queue(
                        &send_to_terminal_clone,
                        &state_clone,
                    )
                    .await;
                });

                state_guard.queue_processor_handle = Some(handle);
            }

            NotificationsResponse::NotificationSent
        }
        NotificationsAction::AddSubscription { mut subscription } => {
            let mut state_guard = state.write().await;

            // Set created_at timestamp if not provided (for backward compatibility)
            if subscription.created_at == 0 {
                subscription.created_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
            }

            // Check if subscription already exists (by endpoint)
            if !state_guard
                .subscriptions
                .iter()
                .any(|s| s.endpoint == subscription.endpoint)
            {
                state_guard.subscriptions.push(subscription.clone());
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    format!("notifications: Added subscription, total: {}", state_guard.subscriptions.len()),
                )
                .send(send_to_terminal)
                .await;

                // Save subscriptions to state
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions)
                    .await?;
            } else {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: Subscription already exists, updating it".to_string(),
                )
                .send(send_to_terminal)
                .await;
                // Update existing subscription
                if let Some(existing) = state_guard
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.endpoint == subscription.endpoint)
                {
                    *existing = subscription;
                    save_subscriptions_to_state(
                        our_node,
                        send_to_state,
                        &state_guard.subscriptions,
                    )
                    .await?;
                }
            }

            // Clean up old subscriptions (older than 1 month)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let one_month_ms = 30 * 24 * 60 * 60 * 1000; // 30 days in milliseconds

            let initial_count = state_guard.subscriptions.len();
            let mut removed_subscriptions = Vec::new();
            state_guard.subscriptions.retain(|s| {
                let age = now.saturating_sub(s.created_at);
                if age > one_month_ms {
                    removed_subscriptions.push((age, s.endpoint.clone()));
                    false
                } else {
                    true
                }
            });

            // Log removed subscriptions
            for (age, endpoint) in removed_subscriptions {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    format!("notifications: Removing old subscription ({}ms old): {}", age, endpoint),
                )
                .send(send_to_terminal)
                .await;
            }

            if state_guard.subscriptions.len() < initial_count {
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions)
                    .await?;
            }

            NotificationsResponse::SubscriptionAdded
        }
        NotificationsAction::RemoveSubscription { endpoint } => {
            let mut state_guard = state.write().await;
            let initial_len = state_guard.subscriptions.len();
            state_guard.subscriptions.retain(|s| s.endpoint != endpoint);

            if state_guard.subscriptions.len() < initial_len {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    format!("notifications: Removed subscription, remaining: {}", state_guard.subscriptions.len()),
                )
                .send(send_to_terminal)
                .await;
                // Save updated subscriptions to state
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions)
                    .await?;
                NotificationsResponse::SubscriptionRemoved
            } else {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: Subscription not found to remove".to_string(),
                )
                .send(send_to_terminal)
                .await;
                NotificationsResponse::SubscriptionRemoved
            }
        }
        NotificationsAction::ClearSubscriptions => {
            let mut state_guard = state.write().await;
            state_guard.subscriptions.clear();
            Printout::new(
                2,
                NOTIFICATIONS_PROCESS_ID.clone(),
                "notifications: Cleared all subscriptions".to_string(),
            )
            .send(send_to_terminal)
            .await;

            // Save empty subscriptions to state
            save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions)
                .await?;

            NotificationsResponse::SubscriptionsCleared
        }
        NotificationsAction::GetSubscription { endpoint } => {
            let state_guard = state.read().await;
            let subscription = state_guard
                .subscriptions
                .iter()
                .find(|s| s.endpoint == endpoint)
                .cloned();

            NotificationsResponse::SubscriptionInfo(subscription)
        }
    };

    // Send response if expected
    if let Some(target) = rsvp.or_else(|| expects_response.map(|_| source)) {
        let response_bytes = serde_json::to_vec(&response).unwrap();

        KernelMessage::builder()
            .id(id)
            .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
            .target(target)
            .message(Message::Response((
                Response {
                    inherit: false,
                    body: response_bytes,
                    metadata: None,
                    capabilities: vec![],
                },
                None,
            )))
            .build()
            .unwrap()
            .send(send_to_loop)
            .await;

        // Response sent
    }

    Ok(())
}

async fn save_keys_to_state(
    our_node: &str,
    send_to_state: &MessageSender,
    keys: &VapidKeys,
) -> Result<(), NotificationsError> {
    let keys_bytes = serde_json::to_vec(keys).map_err(|e| NotificationsError::StateError {
        error: format!("Failed to serialize keys: {:?}", e),
    })?;

    KernelMessage::builder()
        .id(rand::random())
        .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
        .target((our_node, ProcessId::new(Some("state"), "distro", "sys")))
        .message(Message::Request(Request {
            inherit: false,
            expects_response: None, // Don't expect a response to avoid polluting the main loop
            body: serde_json::to_vec(&StateAction::SetState(NOTIFICATIONS_PROCESS_ID.clone()))
                .unwrap(),
            metadata: None,
            capabilities: vec![],
        }))
        .lazy_load_blob(Some(LazyLoadBlob {
            mime: Some("application/octet-stream".into()),
            bytes: keys_bytes,
        }))
        .build()
        .unwrap()
        .send(send_to_state)
        .await;

    Ok(())
}

async fn save_subscriptions_to_state(
    our_node: &str,
    send_to_state: &MessageSender,
    subscriptions: &[PushSubscription],
) -> Result<(), NotificationsError> {
    let subscriptions_bytes =
        serde_json::to_vec(subscriptions).map_err(|e| NotificationsError::StateError {
            error: format!("Failed to serialize subscriptions: {:?}", e),
        })?;

    KernelMessage::builder()
        .id(rand::random())
        .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
        .target((our_node, ProcessId::new(Some("state"), "distro", "sys")))
        .message(Message::Request(Request {
            inherit: false,
            expects_response: None, // Don't expect a response to avoid polluting the main loop
            body: serde_json::to_vec(&StateAction::SetState(ProcessId::new(
                Some("notifications-subscriptions"),
                "distro",
                "sys",
            )))
            .unwrap(),
            metadata: None,
            capabilities: vec![],
        }))
        .lazy_load_blob(Some(LazyLoadBlob {
            mime: Some("application/octet-stream".into()),
            bytes: subscriptions_bytes,
        }))
        .build()
        .unwrap()
        .send(send_to_state)
        .await;

    Ok(())
}

async fn process_notification_queue(
    send_to_terminal: &PrintSender,
    state: &Arc<RwLock<NotificationsState>>,
) {
    loop {
        // Check if we should process the next notification
        let should_process = {
            let state_guard = state.read().await;

            // Check if enough time has passed since last push
            match state_guard.last_push_timestamp {
                None => true, // No previous push, can send immediately
                Some(last_timestamp) => {
                    let elapsed = tokio::time::Instant::now().duration_since(last_timestamp);
                    elapsed >= tokio::time::Duration::from_secs(5)
                }
            }
        };

        if should_process {
            // Process one notification from the queue
            let notification = {
                let mut state_guard = state.write().await;
                state_guard.notification_queue.pop_front()
            };

            if let Some(notification) = notification {
                Printout::new(
                    2,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    "notifications: Processing notification from queue".to_string(),
                )
                .send(send_to_terminal)
                .await;

                // Send the notification
                if let Err(e) = send_notification_to_all(
                    send_to_terminal,
                    state,
                    notification,
                ).await {
                    Printout::new(
                        0,
                        NOTIFICATIONS_PROCESS_ID.clone(),
                        format!("notifications: Error sending notification: {:?}", e),
                    )
                    .send(send_to_terminal)
                    .await;
                }

                // Update timestamp and check if we should exit
                let mut state_guard = state.write().await;
                state_guard.last_push_timestamp = Some(tokio::time::Instant::now());

                // Check if queue is now empty and clean up if so
                if state_guard.notification_queue.is_empty() {
                    Printout::new(
                        2,
                        NOTIFICATIONS_PROCESS_ID.clone(),
                        "notifications: Queue now empty, exiting processor".to_string(),
                    )
                    .send(send_to_terminal)
                    .await;
                    state_guard.queue_processor_handle = None;
                    return;
                } else {
                    Printout::new(
                        2,
                        NOTIFICATIONS_PROCESS_ID.clone(),
                        format!("notifications: {} more notifications in queue, waiting 5 seconds", state_guard.notification_queue.len()),
                    )
                    .send(send_to_terminal)
                    .await;
                }
            }
        } else {
            // Wait until 5 seconds have passed since last push
            let wait_duration = {
                let state_guard = state.read().await;
                match state_guard.last_push_timestamp {
                    Some(last_timestamp) => {
                        let elapsed = tokio::time::Instant::now().duration_since(last_timestamp);
                        if elapsed < tokio::time::Duration::from_secs(5) {
                            tokio::time::Duration::from_secs(5) - elapsed
                        } else {
                            tokio::time::Duration::from_secs(0)
                        }
                    }
                    None => tokio::time::Duration::from_secs(0),
                }
            };

            if wait_duration > tokio::time::Duration::from_secs(0) {
                tokio::time::sleep(wait_duration).await;
            }
        }
    }
}

async fn send_notification_to_all(
    send_to_terminal: &PrintSender,
    state: &Arc<RwLock<NotificationsState>>,
    notification: QueuedNotification,
) -> Result<(), NotificationsError> {
    let state_guard = state.read().await;

    let keys = state_guard
        .vapid_keys
        .as_ref()
        .ok_or(NotificationsError::KeysNotInitialized)?;

    if state_guard.subscriptions.is_empty() {
        Printout::new(
            2,
            NOTIFICATIONS_PROCESS_ID.clone(),
            "notifications: No subscriptions available to send notification".to_string(),
        )
        .send(send_to_terminal)
        .await;
        return Ok(());
    }

    // Build the notification payload
    let payload = serde_json::json!({
        "title": notification.title,
        "body": notification.body,
        "icon": notification.icon,
        "data": notification.data,
    });

    Printout::new(
        2,
        NOTIFICATIONS_PROCESS_ID.clone(),
        format!("notifications: Sending notification to {} devices", state_guard.subscriptions.len()),
    )
    .send(send_to_terminal)
    .await;

    // Send to all subscriptions
    let mut send_errors = Vec::new();
    let mut send_count = 0;

    for subscription in &state_guard.subscriptions {
        // Create subscription info for web-push
        let subscription_info = SubscriptionInfo::new(
            &subscription.endpoint,
            &subscription.keys.p256dh,
            &subscription.keys.auth,
        );

        // Convert raw private key bytes to PEM format for web-push
        let private_key_bytes = URL_SAFE_NO_PAD.decode(&keys.private_key).map_err(|e| {
            NotificationsError::WebPushError {
                error: format!("Failed to decode private key: {:?}", e),
            }
        })?;

        // Convert Vec to fixed-size array
        let private_key_array: [u8; 32] =
            private_key_bytes
                .try_into()
                .map_err(|_| NotificationsError::WebPushError {
                    error: "Invalid private key length".to_string(),
                })?;

        // Create PEM from raw bytes using p256
        use p256::ecdsa::SigningKey;
        use p256::pkcs8::EncodePrivateKey;

        let signing_key =
            SigningKey::from_bytes(&private_key_array.into()).map_err(|e| {
                NotificationsError::WebPushError {
                    error: format!("Failed to create signing key: {:?}", e),
                }
            })?;

        let pem_content = signing_key
            .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
            .map_err(|e| NotificationsError::WebPushError {
                error: format!("Failed to convert to PEM: {:?}", e),
            })?
            .to_string();

        // Create VAPID signature from PEM
        let mut sig_builder =
            VapidSignatureBuilder::from_pem(pem_content.as_bytes(), &subscription_info)
                .map_err(|e| NotificationsError::WebPushError {
                    error: format!("Failed to create VAPID signature: {:?}", e),
                })?;

        // Add required subject claim for VAPID
        sig_builder.add_claim("sub", "mailto:admin@hyperware.ai");

        let sig_builder =
            sig_builder
                .build()
                .map_err(|e| NotificationsError::WebPushError {
                    error: format!("Failed to build VAPID signature: {:?}", e),
                })?;

        // Build the web push message
        let mut message_builder = WebPushMessageBuilder::new(&subscription_info);
        let payload_str = payload.to_string();
        message_builder.set_payload(ContentEncoding::Aes128Gcm, payload_str.as_bytes());
        message_builder.set_vapid_signature(sig_builder);

        let message =
            message_builder
                .build()
                .map_err(|e| NotificationsError::WebPushError {
                    error: format!("Failed to build message: {:?}", e),
                })?;

        // Send the notification using IsahcWebPushClient
        let client =
            IsahcWebPushClient::new().map_err(|e| NotificationsError::WebPushError {
                error: format!("Failed to create web push client: {:?}", e),
            })?;

        match client.send(message).await {
            Ok(_) => {
                send_count += 1;
            }
            Err(e) => {
                Printout::new(
                    0,
                    NOTIFICATIONS_PROCESS_ID.clone(),
                    format!("notifications: Failed to send to {}: {:?}", subscription.endpoint, e),
                )
                .send(send_to_terminal)
                .await;
                send_errors.push(format!("Failed to send to endpoint: {:?}", e));
            }
        }
    }

    Printout::new(
        2,
        NOTIFICATIONS_PROCESS_ID.clone(),
        format!("notifications: Sent to {}/{} devices", send_count, state_guard.subscriptions.len()),
    )
    .send(send_to_terminal)
    .await;

    Ok(())
}
