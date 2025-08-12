pub mod config;
pub mod permissions;
pub mod state;

pub mod api;
pub mod core;
pub mod integrations;

use api::terminal_commands::{MessageHandler, TerminalHandler};
use config::*;
use hyperware_process_lib::homepage::add_to_homepage;
use hyperware_process_lib::http::server::{HttpBindingConfig, HttpServer};
use hyperware_process_lib::hyperwallet_client::types::HyperwalletMessage;
use hyperware_process_lib::logging::{error, info, init_logging, Level};
use hyperware_process_lib::{await_message, call_init, Address, Message, Response};
use permissions::PermissionValidator;
use state::HyperwalletState;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "hyperwallet-sys-v0",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(our: Address) {
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    info!(
        "Initializing {} v{} for: {}",
        SERVICE_NAME, SERVICE_VERSION, our.node
    );

    let mut state = HyperwalletState::initialize();
    let mut http_server = match init_http() {
        Ok(server) => {
            info!("Successfully initialized and bound HTTP server.");
            server
        }
        Err(e) => {
            error!("FATAL: Failed to initialize HTTP server: {:?}", e);
            return;
        }
    };

    let terminal_handler = TerminalHandler::new();
    let permission_validator = PermissionValidator::new();

    info!("{} Service initialized successfully!", SERVICE_NAME);
    info!("Entering main message loop...");

    loop {
        if let Err(e) = handle_message(
            &our,
            &mut state,
            &mut http_server,
            &terminal_handler,
            &permission_validator,
        ) {
            error!("Error in main loop: {:?}", e);
            break;
        }
    }
    info!("Exited main message loop.");
}

fn handle_message(
    our: &Address,
    state: &mut HyperwalletState,
    _http_server: &mut HttpServer,
    terminal_handler: &TerminalHandler,
    permission_validator: &PermissionValidator,
) -> anyhow::Result<()> {
    let message = await_message()?;

    match message {
        Message::Request { source, body, .. } => route_request(
            our,
            &source,
            body,
            state,
            terminal_handler,
            permission_validator,
        ),
        Message::Response {
            source,
            body,
            context,
            ..
        } => handle_response(our, &source, body, context, state),
    }
}

fn route_request(
    _our: &Address,
    source: &Address,
    body: Vec<u8>,
    state: &mut HyperwalletState,
    terminal_handler: &TerminalHandler,
    permission_validator: &PermissionValidator,
) -> anyhow::Result<()> {
    let process = source.process.to_string();
    let pkg = source.package_id().to_string();

    match process.as_str() {
        "http-server:distro:sys" => {
            let server_request: hyperware_process_lib::http::server::HttpServerRequest =
                serde_json::from_slice(&body)?;
            api::http_endpoints::handle_http_request(server_request, source, state)
        }
        _ if pkg == "terminal:sys" => terminal_handler.handle(source, body, state),
        _ => {
            // Gracefully handle unknown or unsupported operation variants without exiting main loop
            // Try to extract the request type for better error reporting
            let req_type = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("request").cloned())
                .and_then(|req| req.get("type").cloned())
                .and_then(|t| t.as_str().map(|s| s.to_string()));

            match serde_json::from_slice::<HyperwalletMessage>(&body) {
                Ok(message) => {
                    let response = permission_validator
                        .execute_with_permissions(message, source, state);

                    Response::new()
                        .body(serde_json::to_vec(&response)?)
                        .send()?;

                    Ok(())
                }
                Err(e) => {
                    // Map unknown variant parse errors to OperationNotSupported; otherwise invalid params
                    let error = if let Some(t) = req_type {
                        hyperware_process_lib::hyperwallet_client::types::OperationError::operation_not_supported(&t)
                    } else {
                        hyperware_process_lib::hyperwallet_client::types::OperationError::invalid_params(&format!(
                            "Invalid message format: {}",
                            e
                        ))
                    };

                    let response = hyperware_process_lib::hyperwallet_client::types::HyperwalletResponse::error(error);
                    Response::new()
                        .body(serde_json::to_vec(&response)?)
                        .send()?;
                    Ok(())
                }
            }
        }
    }
}

fn handle_response(
    _our: &Address,
    source: &Address,
    _body: Vec<u8>,
    _context: Option<Vec<u8>>,
    _state: &mut HyperwalletState,
) -> anyhow::Result<()> {
    info!("Received response from: {}", source);
    Ok(())
}

fn init_http() -> anyhow::Result<HttpServer> {
    let mut http_server = HttpServer::new(5);
    let http_config = HttpBindingConfig::default().authenticated(HTTP_BIND_AUTHENTICATED);

    add_to_homepage(SERVICE_NAME, None, Some("/"), None);
    http_server.serve_ui("ui", vec!["/"], http_config.clone())?;

    let endpoints = vec![
        "/api/operation".to_string(),
        "/api/status".to_string(),
        "/api/wallets".to_string(),
        "/api/permissions".to_string(),
    ];

    for endpoint in endpoints {
        http_server.bind_http_path(endpoint, http_config.clone())?;
    }

    Ok(http_server)
}
