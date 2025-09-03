use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use lib::types::core::{
    KernelMessage, LazyLoadBlob, Message, MessageReceiver, MessageSender, PrintSender, Printout,
    ProcessId, Request, Response, NOTIFICATIONS_PROCESS_ID,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use web_push::WebPushClient;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

// Import our types from lib
use lib::core::StateAction;
use lib::notifications::{NotificationsAction, NotificationsError, NotificationsResponse, PushSubscription, SubscriptionKeys};

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
        use p256::{ecdsa::SigningKey, PublicKey};

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

        println!("notifications: Generated public key: {}", public_key);
        println!(
            "notifications: Public key length: {} bytes",
            public_key_bytes.len()
        );

        Ok(VapidKeys {
            public_key,
            private_key,
        })
    }
}

pub struct NotificationsState {
    vapid_keys: Option<VapidKeys>,
    subscriptions: Vec<PushSubscription>,
}

pub async fn notifications(
    our_node: Arc<String>,
    send_to_loop: MessageSender,
    send_to_terminal: PrintSender,
    mut recv_notifications: MessageReceiver,
    send_to_state: MessageSender,
) -> Result<(), anyhow::Error> {
    println!("notifications: starting notifications module");

    let state = Arc::new(RwLock::new(NotificationsState {
        vapid_keys: None,
        subscriptions: Vec::new(),
    }));

    // Try to load existing keys from state
    println!("notifications: loading keys from state");
    load_keys_from_state(
        &our_node,
        &mut recv_notifications,
        &send_to_state,
        &send_to_loop,
        &state,
    )
    .await;
    println!("notifications: finished loading keys from state");

    while let Some(km) = recv_notifications.recv().await {
        if *our_node != km.source.node {
            Printout::new(
                1,
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
                println!("notifications: error handling request: {:?}", e);
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
                println!("notifications: no saved keys found in state, will generate on first use");
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
                                            println!("notifications: loaded existing VAPID keys from state");
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
            body: serde_json::to_vec(&StateAction::GetState(ProcessId::new(Some("notifications-subscriptions"), "distro", "sys")))
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
                println!("notifications: no saved subscriptions found in state");
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
                                            println!("notifications: loaded {} existing subscriptions from state", state_guard.subscriptions.len());
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
            println!("notifications: InitializeKeys action received");
            let keys = VapidKeys::generate()?;

            // Save keys to state
            save_keys_to_state(our_node, send_to_state, &keys).await?;

            // Update our state
            let mut state_guard = state.write().await;
            state_guard.vapid_keys = Some(keys);

            println!("notifications: Keys initialized successfully");
            NotificationsResponse::KeysInitialized
        }
        NotificationsAction::GetPublicKey => {
            println!(
                "notifications: GetPublicKey action received from {:?}",
                source
            );
            let state_guard = state.read().await;
            match &state_guard.vapid_keys {
                Some(keys) => {
                    println!(
                        "notifications: returning existing public key: {}",
                        keys.public_key
                    );
                    NotificationsResponse::PublicKey(keys.public_key.clone())
                }
                None => {
                    println!("notifications: no keys found, generating new ones");
                    // Try to initialize keys
                    drop(state_guard);
                    let keys = VapidKeys::generate()?;
                    println!(
                        "notifications: generated new keys, public key: {}",
                        keys.public_key
                    );
                    save_keys_to_state(our_node, send_to_state, &keys).await?;

                    let mut state_guard = state.write().await;
                    let public_key = keys.public_key.clone();
                    state_guard.vapid_keys = Some(keys);

                    println!("notifications: returning new public key: {}", public_key);
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
            let state_guard = state.read().await;
            let keys = state_guard
                .vapid_keys
                .as_ref()
                .ok_or(NotificationsError::KeysNotInitialized)?;

            if state_guard.subscriptions.is_empty() {
                println!("notifications: No subscriptions available to send notification");
                return Ok(());
            }

            // Build the notification payload
            let payload = serde_json::json!({
                "title": title,
                "body": body,
                "icon": icon,
                "data": data,
            });

            println!("notifications: Sending notification to {} devices", state_guard.subscriptions.len());

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

                let signing_key = SigningKey::from_bytes(&private_key_array.into()).map_err(|e| {
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
                        .map_err(|e| {
                            NotificationsError::WebPushError {
                                error: format!("Failed to create VAPID signature: {:?}", e),
                            }
                        })?;

                // Add required subject claim for VAPID
                sig_builder.add_claim("sub", "mailto:admin@hyperware.ai");

                let sig_builder = sig_builder.build().map_err(|e| {
                    NotificationsError::WebPushError {
                        error: format!("Failed to build VAPID signature: {:?}", e),
                    }
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
                        println!("notifications: Failed to send to {}: {:?}", subscription.endpoint, e);
                        send_errors.push(format!("Failed to send to endpoint: {:?}", e));
                    }
                }
            }

            println!("notifications: Sent to {}/{} devices", send_count, state_guard.subscriptions.len());

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
            if !state_guard.subscriptions.iter().any(|s| s.endpoint == subscription.endpoint) {
                state_guard.subscriptions.push(subscription.clone());
                println!("notifications: Added subscription, total: {}", state_guard.subscriptions.len());

                // Save subscriptions to state
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions).await?;
            } else {
                println!("notifications: Subscription already exists, updating it");
                // Update existing subscription
                if let Some(existing) = state_guard.subscriptions.iter_mut().find(|s| s.endpoint == subscription.endpoint) {
                    *existing = subscription;
                    save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions).await?;
                }
            }

            // Clean up old subscriptions (older than 1 month)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let one_month_ms = 30 * 24 * 60 * 60 * 1000; // 30 days in milliseconds

            let initial_count = state_guard.subscriptions.len();
            state_guard.subscriptions.retain(|s| {
                let age = now.saturating_sub(s.created_at);
                if age > one_month_ms {
                    println!("notifications: Removing old subscription ({}ms old): {}", age, s.endpoint);
                    false
                } else {
                    true
                }
            });

            if state_guard.subscriptions.len() < initial_count {
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions).await?;
            }

            NotificationsResponse::SubscriptionAdded
        }
        NotificationsAction::RemoveSubscription { endpoint } => {
            let mut state_guard = state.write().await;
            let initial_len = state_guard.subscriptions.len();
            state_guard.subscriptions.retain(|s| s.endpoint != endpoint);

            if state_guard.subscriptions.len() < initial_len {
                println!("notifications: Removed subscription, remaining: {}", state_guard.subscriptions.len());
                // Save updated subscriptions to state
                save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions).await?;
                NotificationsResponse::SubscriptionRemoved
            } else {
                println!("notifications: Subscription not found to remove");
                NotificationsResponse::SubscriptionRemoved
            }
        }
        NotificationsAction::ClearSubscriptions => {
            let mut state_guard = state.write().await;
            state_guard.subscriptions.clear();
            println!("notifications: Cleared all subscriptions");

            // Save empty subscriptions to state
            save_subscriptions_to_state(our_node, send_to_state, &state_guard.subscriptions).await?;

            NotificationsResponse::SubscriptionsCleared
        }
        NotificationsAction::GetSubscription { endpoint } => {
            let state_guard = state.read().await;
            let subscription = state_guard.subscriptions.iter()
                .find(|s| s.endpoint == endpoint)
                .cloned();

            if let Some(ref sub) = subscription {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let age_ms = now.saturating_sub(sub.created_at);
                println!("notifications: Found subscription for endpoint, age: {}ms", age_ms);
            } else {
                println!("notifications: No subscription found for endpoint: {}", endpoint);
            }

            NotificationsResponse::SubscriptionInfo(subscription)
        }
    };

    // Send response if expected
    if let Some(target) = rsvp.or_else(|| expects_response.map(|_| source)) {
        println!(
            "notifications: sending response {:?} to {:?}",
            response, target
        );
        let response_bytes = serde_json::to_vec(&response).unwrap();
        println!(
            "notifications: response serialized to {} bytes",
            response_bytes.len()
        );

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

        println!("notifications: response sent");
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
            expects_response: None,  // Don't expect a response to avoid polluting the main loop
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
    let subscriptions_bytes = serde_json::to_vec(subscriptions).map_err(|e| NotificationsError::StateError {
        error: format!("Failed to serialize subscriptions: {:?}", e),
    })?;

    KernelMessage::builder()
        .id(rand::random())
        .source((our_node, NOTIFICATIONS_PROCESS_ID.clone()))
        .target((our_node, ProcessId::new(Some("state"), "distro", "sys")))
        .message(Message::Request(Request {
            inherit: false,
            expects_response: None,  // Don't expect a response to avoid polluting the main loop
            body: serde_json::to_vec(&StateAction::SetState(ProcessId::new(Some("notifications-subscriptions"), "distro", "sys")))
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
