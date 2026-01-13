use crate::{
    safe_update_message_status, ChatMessage, ChatState, MessageStatus, MessageType,
    WsClientMessage, WsServerMessage,
};
use hyperware_process_lib::{
    http::server::{HttpServerRequest, WsMessageType},
    Address, LazyLoadBlob, Request,
};
use serde_json;

impl ChatState {
    pub(crate) fn handle_client_message(&mut self, channel_id: u32, msg: WsClientMessage) {
        match msg {
            WsClientMessage::SendMessage {
                chat_id,
                content,
                reply_to,
            } => {
                if let Err(err) =
                    self.send_message_internal(&chat_id, content, reply_to, Some(channel_id))
                {
                    crate::log_debug!("Failed to send message via WS: {}", err);
                }
            }
            WsClientMessage::Ack { message_id } => {
                // Update message status
                for chat in self.chats.values_mut() {
                    if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                        message.status =
                            safe_update_message_status(&message.status, MessageStatus::Delivered);
                        break;
                    }
                }
            }
            WsClientMessage::MarkRead { chat_id } => {
                if let Some(chat) = self.chats.get_mut(&chat_id) {
                    chat.unread_count = 0;
                }
            }
            WsClientMessage::MarkGroupRead { group_id } => {
                self.group_unread.insert(group_id, 0);
            }
            WsClientMessage::UpdateStatus { status } => {
                // Track whether this connection is active (user viewing the page)
                if status == "active" {
                    self.active_connections.insert(channel_id);
                } else if status == "inactive" {
                    self.active_connections.remove(&channel_id);
                }

                if let Some(node) = self.ws_connections.get(&channel_id) {
                    let msg = WsServerMessage::StatusUpdate {
                        node: node.clone(),
                        status,
                    };
                    self.broadcast_ws_message(&msg);
                }
            }
            WsClientMessage::Heartbeat => {
                let msg = WsServerMessage::Heartbeat;
                self.push_ws_message(channel_id, &msg);
            }
            _ => {
                // Other message types not handled in node-to-node
            }
        }
    }

    pub(crate) fn handle_browser_message(&mut self, channel_id: u32, msg: WsClientMessage) {
        match msg {
            WsClientMessage::AuthWithKey { chat_key } => {
                if let Some(key_data) = self.chat_keys.get(&chat_key) {
                    if !key_data.is_revoked {
                        // Store connection
                        self.browser_connections
                            .insert(chat_key.clone(), channel_id);

                        // Get chat history
                        let history = self
                            .chats
                            .get(&key_data.chat_id)
                            .map(|chat| chat.messages.clone())
                            .unwrap_or_default();

                        let msg = WsServerMessage::AuthSuccess {
                            chat_id: key_data.chat_id.clone(),
                            history,
                        };
                        self.push_ws_message(channel_id, &msg);
                    } else {
                        let msg = WsServerMessage::AuthFailed {
                            reason: "Chat key has been revoked".to_string(),
                        };
                        self.push_ws_message(channel_id, &msg);
                    }
                } else {
                    let msg = WsServerMessage::AuthFailed {
                        reason: "Invalid chat key".to_string(),
                    };
                    self.push_ws_message(channel_id, &msg);
                }
            }
            WsClientMessage::BrowserMessage { content } => {
                // Find chat key for this connection
                if let Some((chat_key, _)) = self
                    .browser_connections
                    .iter()
                    .find(|(_, &ch)| ch == channel_id)
                {
                    if let Some(key_data) = self.chat_keys.get(chat_key).cloned() {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        let chat_id = key_data.chat_id.clone();
                        self.get_or_create_chat(
                            &chat_id,
                            timestamp,
                            Some(key_data.user_name.clone()),
                            None,
                        );

                        let mut message = ChatMessage {
                            id: format!("{}:{}", timestamp, rand::random::<u32>()),
                            sender: key_data.user_name.clone(),
                            content,
                            timestamp,
                            sequence: None,
                            status: MessageStatus::Sent,
                            reply_to: None,
                            reactions: Vec::new(),
                            message_type: MessageType::Text,
                            file_info: None,
                        };

                        self.assign_sequence_to_message(&chat_id, &mut message);

                        self.get_or_create_chat(
                            &chat_id,
                            timestamp,
                            Some(key_data.user_name.clone()),
                            None,
                        );
                        {
                            let chat = self.get_or_create_chat(
                                &chat_id,
                                timestamp,
                                Some(key_data.user_name.clone()),
                                None,
                            );
                            chat.messages.push(message.clone());
                            chat.last_activity = timestamp;
                            chat.unread_count += 1;
                        }
                        self.rebuild_chat_search(&chat_id);

                        // Send message to all participants
                        let msg = WsServerMessage::NewMessage(message);
                        self.push_ws_message(channel_id, &msg);
                    }
                }
            }
            WsClientMessage::Heartbeat => {
                let msg = WsServerMessage::Heartbeat;
                self.push_ws_message(channel_id, &msg);
            }
            _ => {}
        }
    }

    /// Push a message to a WebSocket channel. Returns true if successful, false if channel not found.
    pub(crate) fn push_ws_message(&self, channel_id: u32, message: &WsServerMessage) -> bool {
        let bytes = match serde_json::to_vec(message) {
            Ok(bytes) => bytes,
            Err(err) => {
                crate::log_debug!("Failed to serialize WS message: {:?}", err);
                return false;
            }
        };

        let request = Request::to(Address::new(
            "our",
            ("http-server", "distro", "sys"),
        ))
        .body(
            serde_json::to_vec(&HttpServerRequest::WebSocketPush {
                channel_id,
                message_type: WsMessageType::Text,
            })
            .unwrap(),
        )
        .blob(LazyLoadBlob {
            mime: Some("application/json".to_string()),
            bytes,
        });

        // Send and await response to detect stale channels
        match request.send_and_await_response(2) {
            Ok(Ok(response)) => {
                // Check if response body contains "WsChannelNotFound"
                if let Ok(body_str) = String::from_utf8(response.body().to_vec()) {
                    if body_str.contains("WsChannelNotFound") {
                        crate::log_debug!("[WS_DEBUG] Channel {} not found, marking for removal", channel_id);
                        return false;
                    }
                }
                true
            }
            Ok(Err(send_err)) => {
                crate::log_debug!("[WS_DEBUG] Send error for channel {}: {:?}", channel_id, send_err);
                false
            }
            Err(err) => {
                crate::log_debug!("[WS_DEBUG] Failed to push to channel {}: {:?}", channel_id, err);
                false
            }
        }
    }

    /// Broadcast a message to all WebSocket connections, removing stale channels.
    pub(crate) fn broadcast_ws_message(&mut self, message: &WsServerMessage) {
        let channels: Vec<u32> = self.ws_connections.keys().cloned().collect();
        crate::log_debug!("[WS_DEBUG] broadcast_ws_message: ws_connections has {} channels: {:?}", channels.len(), channels);

        let mut stale_channels = Vec::new();
        for channel_id in channels {
            if !self.push_ws_message(channel_id, message) {
                stale_channels.push(channel_id);
            }
        }

        // Clean up stale channels
        for channel_id in stale_channels {
            crate::log_debug!("[WS_DEBUG] Removing stale channel {} from ws_connections", channel_id);
            self.ws_connections.remove(&channel_id);
            self.browser_connections.retain(|_, &mut v| v != channel_id);
            self.active_connections.remove(&channel_id);
        }
    }
}
