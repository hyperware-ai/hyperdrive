use crate::types::*;
use futures::stream::SplitSink;
use hmac::{Hmac, Mac};
use jwt::{Error, VerifyWithKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use warp::http::{header::HeaderName, header::HeaderValue, HeaderMap};
use warp::ws::WebSocket;

pub type SharedWriteStream = Arc<Mutex<SplitSink<WebSocket, warp::ws::Message>>>;
pub type WebSockets = Arc<Mutex<HashMap<String, HashMap<String, HashMap<u64, SharedWriteStream>>>>>;
pub type WebSocketProxies = Arc<Mutex<HashMap<String, HashSet<String>>>>;

pub struct BoundPath {
    pub app: ProcessId,
    pub authenticated: bool,
    pub local_only: bool,
    pub original_path: String,
}

#[derive(Serialize, Deserialize)]
pub struct RpcMessage {
    pub node: Option<String>,
    pub process: String,
    pub inherit: Option<bool>,
    pub expects_response: Option<u64>,
    pub ipc: Option<String>,
    pub metadata: Option<String>,
    pub context: Option<String>,
    pub mime: Option<String>,
    pub data: Option<String>,
}

pub fn parse_auth_token(auth_token: String, jwt_secret: Vec<u8>) -> Result<String, Error> {
    let secret: Hmac<Sha256> = match Hmac::new_from_slice(jwt_secret.as_slice()) {
        Ok(secret) => secret,
        Err(_) => {
            return Ok("Error recovering jwt secret".to_string());
        }
    };

    let claims: Result<JwtClaims, Error> = auth_token.verify_with_key(&secret);

    match claims {
        Ok(data) => Ok(data.username),
        Err(err) => Err(err),
    }
}

pub fn auth_cookie_valid(our_node: String, cookie: &str, jwt_secret: Vec<u8>) -> bool {
    let cookie_parts: Vec<&str> = cookie.split("; ").collect();
    let mut auth_token = None;

    for cookie_part in cookie_parts {
        let cookie_part_parts: Vec<&str> = cookie_part.split('=').collect();
        if cookie_part_parts.len() == 2
            && cookie_part_parts[0] == format!("uqbar-auth_{}", our_node)
        {
            auth_token = Some(cookie_part_parts[1].to_string());
            break;
        }
    }

    let auth_token = match auth_token {
        Some(token) if !token.is_empty() => token,
        _ => return false,
    };

    let secret = match Hmac::<Sha256>::new_from_slice(&jwt_secret) {
        Ok(secret) => secret,
        Err(_) => return false,
    };

    let claims: Result<JwtClaims, _> = auth_token.verify_with_key(&secret);

    match claims {
        Ok(data) => data.username == our_node,
        Err(_) => false,
    }
}

pub fn remove_process_id(path: &str) -> String {
    // Split the string into parts separated by '/'
    let mut parts = path.splitn(3, '/');
    // Skip the first two parts (before and after the first '/')
    let remaining_path = parts.nth(2).unwrap_or("");
    // If the result is empty, return "/"
    if remaining_path.is_empty() {
        return "/".to_string();
    }
    // Otherwise, return the result with a leading "/"
    format!("/{}", remaining_path)
}

pub fn normalize_path(path: &str) -> String {
    let mut normalized = path.to_string();
    if normalized != "/" && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

pub async fn handle_incoming_ws(
    parsed_msg: WebSocketClientMessage,
    our: String,
    jwt_secret_bytes: Vec<u8>,
    websockets: WebSockets,
    send_to_loop: MessageSender,
    print_tx: PrintSender,
    write_stream: SharedWriteStream,
    ws_id: u64,
) {
    let cloned_parsed_msg = parsed_msg.clone();
    match parsed_msg {
        WebSocketClientMessage::WsRegister(WsRegister {
            ws_auth_token,
            auth_token: _,
            channel_id,
        }) => {
            let _ = print_tx
                .send(Printout {
                    verbosity: 1,
                    content: format!("REGISTER: {} {}", ws_auth_token, channel_id),
                })
                .await;
            // Get node from auth token
            if let Ok(node) = parse_auth_token(ws_auth_token, jwt_secret_bytes.clone()) {
                let _ = print_tx
                    .send(Printout {
                        verbosity: 1,
                        content: format!("NODE: {}", node),
                    })
                    .await;
                handle_ws_register(
                    node,
                    cloned_parsed_msg,
                    channel_id.clone(),
                    our.clone(),
                    websockets.clone(),
                    send_to_loop.clone(),
                    print_tx.clone(),
                    write_stream.clone(),
                    ws_id,
                )
                .await;
            } else {
                let _ = print_tx
                    .send(Printout {
                        verbosity: 1,
                        content: "Auth token parsing failed for WsRegister".to_string(),
                    })
                    .await;
            }
        }
        // Forward to target's http_server with the auth_token
        WebSocketClientMessage::WsMessage(WsMessage {
            ws_auth_token,
            auth_token: _,
            target,
            json,
            ..
        }) => {
            let _ = print_tx
                .send(Printout {
                    verbosity: 1,
                    content: format!("ACTION: {}", target.node.clone()),
                })
                .await;
            // TODO: restrict sending actions to ourself and nodes for which we are proxying
            // TODO: use the channel_id
            if let Ok(node) = parse_auth_token(ws_auth_token, jwt_secret_bytes.clone()) {
                if node == target.node {
                    if target.node == our {
                        handle_ws_message(
                            target.clone(),
                            json.clone(),
                            our.clone(),
                            send_to_loop.clone(),
                            print_tx.clone(),
                        )
                        .await;
                    } else {
                        proxy_ws_message(
                            node,
                            cloned_parsed_msg,
                            our.clone(),
                            send_to_loop.clone(),
                            print_tx.clone(),
                        )
                        .await;
                    }
                }
            }
        }
        // Forward to target's http_server with the auth_token
        WebSocketClientMessage::EncryptedWsMessage(EncryptedWsMessage {
            ws_auth_token,
            auth_token: _,
            channel_id,
            target,
            encrypted,
            nonce,
        }) => {
            let _ = print_tx
                .send(Printout {
                    verbosity: 1,
                    content: format!("ENCRYPTED ACTION: {}", target.node.clone()),
                })
                .await;
            if let Ok(node) = parse_auth_token(ws_auth_token, jwt_secret_bytes.clone()) {
                if node == target.node {
                    if target.node == our {
                        handle_encrypted_ws_message(
                            target.clone(),
                            our.clone(),
                            channel_id.clone(),
                            encrypted.clone(),
                            nonce.clone(),
                            send_to_loop.clone(),
                            print_tx.clone(),
                        )
                        .await;
                    } else {
                        proxy_ws_message(
                            node,
                            cloned_parsed_msg,
                            our.clone(),
                            send_to_loop.clone(),
                            print_tx.clone(),
                        )
                        .await;
                    }
                }
            }
        }
    }
}

pub fn serialize_headers(headers: &HeaderMap) -> HashMap<String, String> {
    let mut hashmap = HashMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.to_string();
        let value_str = value.to_str().unwrap_or("").to_string();
        hashmap.insert(key_str, value_str);
    }
    hashmap
}

pub fn deserialize_headers(hashmap: HashMap<String, String>) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    for (key, value) in hashmap {
        let key_bytes = key.as_bytes();
        let key_name = HeaderName::from_bytes(key_bytes).unwrap();
        let value_header = HeaderValue::from_str(&value).unwrap();
        header_map.insert(key_name, value_header);
    }
    header_map
}

pub async fn is_port_available(bind_addr: &str) -> bool {
    TcpListener::bind(bind_addr).await.is_ok()
}

pub fn binary_encoded_string_to_bytes(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

pub async fn handle_ws_register(
    node: String,
    parsed_msg: WebSocketClientMessage,
    channel_id: String,
    our: String,
    websockets: WebSockets,
    send_to_loop: MessageSender,
    print_tx: PrintSender,
    write_stream: SharedWriteStream,
    ws_id: u64,
) {
    // let _ = print_tx.send(Printout { verbosity: 1, content: format!("1.2 {}", node) }).await;
    // TODO: restrict registration to ourself and nodes for which we are proxying
    let mut ws_map = websockets.lock().await;
    let node_map = ws_map.entry(node.clone()).or_insert(HashMap::new());
    let id_map = node_map.entry(channel_id.clone()).or_insert(HashMap::new());
    id_map.insert(ws_id, write_stream.clone());

    // Send a message to the target node to add to let it know we are proxying
    if node != our {
        let id: u64 = rand::random();
        let message = KernelMessage {
            id,
            source: Address {
                node: our.clone(),
                process: HTTP_SERVER_PROCESS_ID.clone(),
            },
            target: Address {
                node: node.clone(),
                process: HTTP_SERVER_PROCESS_ID.clone(),
            },
            rsvp: None,
            message: Message::Request(Request {
                inherit: false,
                expects_response: None,
                ipc: serde_json::json!(parsed_msg).to_string().into_bytes(),
                metadata: None,
            }),
            payload: Some(Payload {
                mime: Some("application/octet-stream".to_string()),
                bytes: vec![],
            }),
            signed_capabilities: None,
        };

        send_to_loop.send(message).await.unwrap();
        let _ = print_tx
            .send(Printout {
                verbosity: 1,
                content: "WEBSOCKET CHANNEL FORWARDED!".to_string(),
            })
            .await;
    }

    let _ = print_tx
        .send(Printout {
            verbosity: 1,
            content: "WEBSOCKET CHANNEL REGISTERED!".to_string(),
        })
        .await;
}

pub async fn handle_ws_message(
    target: Address,
    json: Option<serde_json::Value>,
    our: String,
    send_to_loop: MessageSender,
    _print_tx: PrintSender,
) {
    let id: u64 = rand::random();
    let message = KernelMessage {
        id,
        source: Address {
            node: our.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        target: target.clone(),
        rsvp: None,
        message: Message::Request(Request {
            inherit: false,
            expects_response: None,
            ipc: vec![],
            metadata: None,
        }),
        payload: Some(Payload {
            mime: Some("application/octet-stream".to_string()),
            bytes: json.unwrap_or_default().to_string().as_bytes().to_vec(),
        }),
        signed_capabilities: None,
    };

    send_to_loop.send(message).await.unwrap();
}

pub async fn handle_encrypted_ws_message(
    target: Address,
    our: String,
    channel_id: String,
    encrypted: String,
    nonce: String,
    send_to_loop: MessageSender,
    _print_tx: PrintSender,
) {
    let encrypted_bytes = binary_encoded_string_to_bytes(&encrypted);
    let nonce_bytes = binary_encoded_string_to_bytes(&nonce);

    let mut encrypted_data = encrypted_bytes;
    encrypted_data.extend(nonce_bytes);

    let id: u64 = rand::random();

    // Send a message to the encryptor
    let message = KernelMessage {
        id,
        source: Address {
            node: our.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        target: Address {
            node: target.node.clone(),
            process: ENCRYPTOR_PROCESS_ID.clone(),
        },
        rsvp: None,
        message: Message::Request(Request {
            inherit: false,
            expects_response: None,
            ipc: serde_json::json!({
                "DecryptAndForwardAction": {
                    "channel_id": channel_id.clone(),
                    "forward_to": target.clone(),
                    "json": {
                        "forwarded_from": {
                            "node": our.clone(),
                            "process": "http_server:sys:uqbar",
                        }
                    },
                }
            })
            .to_string()
            .into_bytes(),
            metadata: None,
        }),
        payload: Some(Payload {
            mime: Some("application/octet-stream".to_string()),
            bytes: encrypted_data,
        }),
        signed_capabilities: None,
    };

    send_to_loop.send(message).await.unwrap();
}

pub async fn proxy_ws_message(
    node: String,
    parsed_msg: WebSocketClientMessage,
    our: String,
    send_to_loop: MessageSender,
    _print_tx: PrintSender,
) {
    let id: u64 = rand::random();
    let message = KernelMessage {
        id,
        source: Address {
            node: our.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        target: Address {
            node,
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        rsvp: None,
        message: Message::Request(Request {
            inherit: false,
            expects_response: None,
            ipc: serde_json::json!(parsed_msg).to_string().into_bytes(),
            metadata: None,
        }),
        payload: Some(Payload {
            mime: Some("application/octet-stream".to_string()),
            bytes: vec![],
        }),
        signed_capabilities: None,
    };

    send_to_loop.send(message).await.unwrap();
}

pub async fn add_ws_proxy(ws_proxies: WebSocketProxies, channel_id: String, source_node: String) {
    let mut locked_proxies = ws_proxies.lock().await;
    if let Some(proxy_nodes) = locked_proxies.get_mut(&channel_id) {
        if !proxy_nodes.contains(&source_node) {
            proxy_nodes.insert(source_node);
        }
    } else {
        let mut proxy_nodes = HashSet::new();
        proxy_nodes.insert(source_node);
        locked_proxies.insert(channel_id, proxy_nodes);
    }
}

pub async fn send_ws_disconnect(
    node: String,
    our: String,
    channel_id: String,
    send_to_loop: MessageSender,
    _print_tx: PrintSender,
) {
    let id: u64 = rand::random();
    let message = KernelMessage {
        id,
        source: Address {
            node: our.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        target: Address {
            node: node.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        rsvp: None,
        message: Message::Request(Request {
            inherit: false,
            expects_response: None,
            ipc: serde_json::json!({
                "WsProxyDisconnect": {
                    "channel_id": channel_id.clone(),
                }
            })
            .to_string()
            .into_bytes(),
            metadata: None,
        }),
        payload: Some(Payload {
            mime: Some("application/octet-stream".to_string()),
            bytes: vec![],
        }),
        signed_capabilities: None,
    };

    send_to_loop.send(message).await.unwrap();
}

pub fn make_error_message(
    our_name: String,
    id: u64,
    target: Address,
    error: HttpServerError,
) -> KernelMessage {
    KernelMessage {
        id,
        source: Address {
            node: our_name.clone(),
            process: HTTP_SERVER_PROCESS_ID.clone(),
        },
        target,
        rsvp: None,
        message: Message::Response((
            Response {
                inherit: false,
                ipc: serde_json::to_vec(&error).unwrap(),
                metadata: None,
            },
            None,
        )),
        payload: None,
        signed_capabilities: None,
    }
}
