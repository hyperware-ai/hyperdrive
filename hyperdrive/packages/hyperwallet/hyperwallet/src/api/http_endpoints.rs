/// HTTP API endpoints for the Hyperwallet web interface
///
/// This module provides REST API endpoints for external clients to interact
/// with the Hyperwallet service via HTTP requests.
use crate::state::HyperwalletState;
use hyperware_process_lib::http::server::HttpServerRequest;
use hyperware_process_lib::logging::info;
use hyperware_process_lib::Address;

pub fn handle_http_request(
    server_request: HttpServerRequest,
    source: &Address,
    _state: &mut HyperwalletState,
) -> anyhow::Result<()> {
    match server_request {
        HttpServerRequest::Http(incoming_request) => {
            info!(
                "HTTP request received: {:#?} from {:?}",
                incoming_request, source
            );
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
