//! set-nodes:hypermap-cacher:sys
//! terminal script for setting the nodes list for hypermap-cacher bootstrap.
//!
//! Usage:
//!     set-nodes:hypermap-cacher:sys [node1] [node2] [node3] ...
//!
//! Arguments:
//!     [node1] [node2] ...  Space-separated list of node names to use for bootstrapping
//!
//! Example:
//!     set-nodes:hypermap-cacher:sys alice.os bob.os charlie.os

use crate::hyperware::process::hypermap_cacher::{CacherRequest, CacherResponse};
use hyperware_process_lib::{await_next_message_body, call_init, println, Address, Request};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "hypermap-cacher-sys-v1",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(_our: Address) {
    let Ok(body) = await_next_message_body() else {
        println!("set-nodes: failed to get args!");
        return;
    };

    let args = String::from_utf8(body).unwrap_or_default();
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.is_empty() {
        println!("set-nodes: no arguments provided. Please specify node names.");
        println!("example: set-nodes alice.os bob.os charlie.os");
        return;
    }

    let nodes: Vec<String> = parts.iter().map(|s| s.to_string()).collect();

    println!("Setting hypermap-cacher nodes to: {:?}", nodes);

    let response = Request::to(("our", "hypermap-cacher", "hypermap-cacher", "sys"))
        .body(CacherRequest::SetNodes(nodes))
        .send_and_await_response(5);

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(CacherResponse::SetNodes(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(CacherResponse::SetNodes(Err(err))) => {
                println!("✗ Failed to set nodes: {}", err);
            }
            _ => {
                println!("✗ Unexpected response from hypermap-cacher");
            }
        },
        Ok(Err(err)) => {
            println!("✗ Request failed: {:?}", err);
        }
        Err(err) => {
            println!("✗ Communication error: {:?}", err);
        }
    }
}
