use std::str::FromStr;

use crate::hyperware::process::hypermap_cacher::CacherRequest;

use hyperware_process_lib::{
    await_message, call_init, http, hypermap,
    logging::{debug, error, info, init_logging, warn, Level},
    timer, vfs, Address, ProcessId, Response,
};

mod constants;
mod handlers;
mod state;
mod types;
mod utils;

use crate::handlers::{handle_request, http_handler};
use crate::types::State;

wit_bindgen::generate!({
    path: "target/wit",
    world: "hypermap-cacher-sys-v0",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

fn main_loop(
    our: &Address,
    state: &mut State,
    hypermap: &hypermap::Hypermap,
    server: &http::server::HttpServer,
) -> anyhow::Result<()> {
    info!("Hypermap Cacher main_loop started. Our address: {}", our);
    info!(
        "Monitoring Hypermap contract: {}",
        state.hypermap_address.to_string()
    );
    info!(
        "Chain ID: {}, Protocol Version: {}",
        state.chain_id, state.protocol_version
    );
    info!("Last cached block: {}", state.last_cached_block);

    // Always bootstrap on start to get latest state from other nodes or RPC
    if state.is_starting {
        match state.bootstrap_state(hypermap) {
            Ok(_) => info!("Bootstrap process completed successfully."),
            Err(e) => error!("Error during bootstrap process: {:?}", e),
        }
    }

    // Set up the main caching timer.
    info!(
        "Setting cache timer for {} seconds.",
        state.cache_interval_s
    );
    timer::set_timer(state.cache_interval_s * 1000, Some(b"cache_cycle".to_vec()));
    state.is_cache_timer_live = true;
    state.save();

    loop {
        let Ok(message) = await_message() else {
            warn!("Failed to get message, continuing loop.");
            continue;
        };
        let source = message.source();

        if message.is_request() {
            if source.process == ProcessId::from_str("http-server:distro:sys").unwrap() {
                // HTTP request from the system's HTTP server process
                let Ok(http::server::HttpServerRequest::Http(http_request)) =
                    server.parse_request(message.body())
                else {
                    error!("Failed to parse HTTP request from http-server:distro:sys");
                    // Potentially send an error response back if possible/expected
                    continue;
                };
                let (http_response, body) = http_handler(state, &http_request.path()?)?;
                Response::new()
                    .body(serde_json::to_vec(&http_response).unwrap())
                    .blob_bytes(body)
                    .send()?;
            } else {
                // Standard process-to-process request
                match serde_json::from_slice::<CacherRequest>(message.body()) {
                    Ok(request) => {
                        if let Err(e) = handle_request(our, &source, state, request) {
                            error!("Error handling request from {:?}: {:?}", source, e);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to deserialize CacherRequest from {:?}: {:?}",
                            source, e
                        );
                    }
                }
            }
        } else {
            // It's a Response or other kind of message
            if source.process == ProcessId::from_str("timer:distro:sys").unwrap() {
                if message.context() == Some(b"cache_cycle") {
                    info!("Cache timer triggered.");
                    state.is_cache_timer_live = false;
                    match state.cache_logs_and_update_manifest(hypermap) {
                        Ok(_) => info!("Periodic cache cycle complete."),
                        Err(e) => error!("Error during periodic cache cycle: {:?}", e),
                    }
                    // Reset the timer for the next cycle
                    if !state.is_cache_timer_live {
                        timer::set_timer(
                            state.cache_interval_s * 1000,
                            Some(b"cache_cycle".to_vec()),
                        );
                        state.is_cache_timer_live = true;
                        state.save();
                    }
                }
            } else {
                debug!(
                    "Received unhandled response or other message from {:?}.",
                    source
                );
            }
        }
    }
}

call_init!(init);
fn init(our: Address) {
    init_logging(Level::INFO, Level::DEBUG, None, None, None).unwrap();
    info!("Hypermap Cacher process starting...");

    let drive_path = vfs::create_drive(our.package_id(), "cache", None).unwrap();

    let bind_config = http::server::HttpBindingConfig::default().authenticated(false);
    let mut server = http::server::HttpServer::new(5);

    let hypermap_provider = hypermap::Hypermap::default(60);

    server
        .bind_http_path("/manifest", bind_config.clone())
        .expect("Failed to bind /manifest");
    server
        .bind_http_path("/manifest.json", bind_config.clone())
        .expect("Failed to bind /manifest.json");
    server
        .bind_http_path("/log-cache/*", bind_config.clone())
        .expect("Failed to bind /log-cache/*");
    server
        .bind_http_path("/status", bind_config.clone())
        .expect("Failed to bind /status");
    info!("Bound HTTP paths: /manifest, /log-cache/*, /status");

    let mut state = State::load(&drive_path);

    loop {
        match main_loop(&our, &mut state, &hypermap_provider, &server) {
            Ok(()) => {
                // main_loop should not exit with Ok in normal operation as it's an infinite loop.
                error!("main_loop exited unexpectedly with Ok. Restarting.");
            }
            Err(e) => {
                error!("main_loop exited with error: {:?}. Restarting.", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
        // Reload state in case of restart, or re-initialize if necessary.
        state = State::load(&drive_path);
    }
}
