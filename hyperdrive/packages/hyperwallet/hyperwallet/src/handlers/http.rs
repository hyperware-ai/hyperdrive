/// HTTP request handler

use super::{api, MessageHandler};
use crate::state::HyperwalletState;
use hyperware_process_lib::http::server::HttpServerRequest;
use hyperware_process_lib::http::Method;
use hyperware_process_lib::logging::info;
use hyperware_process_lib::{last_blob, Address, Response};

pub struct HttpHandler;

impl HttpHandler {
    pub fn new() -> Self {
        Self
    }

    fn handle_http_request(
        &self,
        server_request: HttpServerRequest,
        source: &Address,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        match server_request {
            HttpServerRequest::Http(incoming_request) => {
                let path = incoming_request.path()?.to_string();
                let method = incoming_request.method()?;

                info!("HTTP {:?} {}", method, path);

                let response = match (method.clone(), path.as_str()) {
                    (Method::POST, "/api/operation") => {
                        let blob = last_blob().unwrap_or_default();
                        let body = blob.bytes();
                        api::handle_operation_request(&body, source, state)
                    }
                    (Method::GET, "/api/status") => api::handle_status_request(state),
                    (Method::GET, "/api/wallets") => api::handle_wallets_request(state, source),
                    (Method::GET, "/api/permissions") => api::handle_permissions_request(state),
                    _ => api::handle_not_found(&path, &format!("{:?}", method)),
                };

                Response::new().body(response.as_bytes()).send()?;
            }
            HttpServerRequest::WebSocketOpen { .. } => {
                info!("WebSocket connection opened");
            }
            HttpServerRequest::WebSocketClose(_) => {
                info!("WebSocket connection closed");
            }
            HttpServerRequest::WebSocketPush { .. } => {
                info!("WebSocket message received");
            }
        }

        Ok(())
    }
}

impl MessageHandler for HttpHandler {
    fn handle(
        &self,
        source: &Address,
        body: Vec<u8>,
        state: &mut HyperwalletState,
    ) -> anyhow::Result<()> {
        let server_request: HttpServerRequest = serde_json::from_slice(&body)?;
        self.handle_http_request(server_request, source, state)
    }
}
