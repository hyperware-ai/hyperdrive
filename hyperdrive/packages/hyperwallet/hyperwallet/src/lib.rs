pub mod config;
pub mod operations;
pub mod permissions;
pub mod state;
pub mod bundler;
mod handlers;

use config::*;
use handlers::{HttpHandler, MessageHandler, ProcessHandler, TerminalHandler};
use hyperware_process_lib::homepage::add_to_homepage;
use hyperware_process_lib::http::server::{HttpBindingConfig, HttpServer};
use hyperware_process_lib::logging::{error, info, init_logging, Level};
use hyperware_process_lib::{await_message, call_init, Address, Message};
use state::HyperwalletState;

// Generate WIT bindings
wit_bindgen::generate!({
    path: "../target/wit",
    world: "process-v1",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(our: Address) {
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    info!("Initializing {} v{} for: {}", SERVICE_NAME, SERVICE_VERSION, our.node);

    // Initialize state
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
    
    // Set up handlers
    let http_handler = HttpHandler::new();
    let process_handler = ProcessHandler::new();
    let terminal_handler = TerminalHandler::new();

    info!("{} Service initialized successfully!", SERVICE_NAME);
    info!("Entering main message loop...");
    
    // Main message loop
    loop {
        if let Err(e) = handle_message(
            &our,
            &mut state,
            &mut http_server,
            &http_handler,
            &process_handler,
            &terminal_handler,
        ) {
            error!("Error in main loop: {:?}", e);
            break;
        }
    }
    info!("Exited main message loop.");
}

/// Handle incoming messages
fn handle_message(
    our: &Address,
    state: &mut HyperwalletState,
    _http_server: &mut HttpServer,
    http_handler: &HttpHandler,
    process_handler: &ProcessHandler,
    terminal_handler: &TerminalHandler,
) -> anyhow::Result<()> {
    let message = await_message()?;

    match message {
        Message::Request { source, body, .. } => {
            route_request(
                our,
                &source,
                body,
                state,
                http_handler,
                process_handler,
                terminal_handler,
            )
        }
        Message::Response {
            source,
            body,
            context,
            ..
        } => handle_response(our, &source, body, context, state),
    }
}

/// Route requests to appropriate handlers
fn route_request(
    _our: &Address,
    source: &Address,
    body: Vec<u8>,
    state: &mut HyperwalletState,
    http_handler: &HttpHandler,
    process_handler: &ProcessHandler,
    terminal_handler: &TerminalHandler,
) -> anyhow::Result<()> {
    let process = source.process.to_string();
    let pkg = source.package_id().to_string();

    match process.as_str() {
        "http-server:distro:sys" => http_handler.handle(source, body, state),
        _ if pkg == "terminal:sys" => terminal_handler.handle(source, body, state),
        _ => process_handler.handle(source, body, state),
    }
}

/// Handle responses (currently minimal implementation)
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

/// Initialize HTTP server with configured endpoints
fn init_http() -> anyhow::Result<HttpServer> {
    let mut http_server = HttpServer::new(5);
    let http_config = HttpBindingConfig::default().authenticated(HTTP_BIND_AUTHENTICATED);

    // Serve UI
    add_to_homepage(SERVICE_NAME, Some(ICON), Some("/"), None);
    http_server.serve_ui("ui", vec!["/"], http_config.clone())?;
            
    // API endpoints - convert to String
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
