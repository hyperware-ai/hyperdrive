//! reset:hypermap-cacher:sys
//! terminal script for resetting hypermap-cacher state and VFS.
//!
//! Usage:
//!     reset:hypermap-cacher:sys [node1] [node2] [node3] ...
//!
//! Arguments:
//!     [node1] [node2] ...  Optional space-separated list of node names to use for bootstrapping
//!                         If no arguments provided, uses default nodes
//!
//! Example:
//!     reset:hypermap-cacher:sys                      # Reset with default nodes
//!     reset:hypermap-cacher:sys alice.os bob.os      # Reset with custom nodes

use crate::hyperware::process::binding_cacher::{BindingCacherRequest, BindingCacherResponse};
use crate::hyperware::process::hypermap_cacher::{CacherRequest, CacherResponse};
use hyperware_process_lib::{await_next_message_body, call_init, println, Address, Request};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "hypermap-cacher-sys-v2",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(_our: Address) {
    let Ok(body) = await_next_message_body() else {
        println!("reset: failed to get args!");
        return;
    };

    let args = String::from_utf8(body).unwrap_or_default();
    let parts: Vec<&str> = args.split_whitespace().collect();

    let custom_nodes = if parts.is_empty() {
        println!("Resetting cachers with default nodes...");
        None
    } else {
        let nodes: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        println!("Resetting cachers with custom nodes: {:?}", nodes);
        Some(nodes)
    };
    let binding_custom_nodes = custom_nodes.clone();

    let response = Request::to(("our", "hypermap-cacher", "hypermap-cacher", "sys"))
        .body(CacherRequest::Reset(custom_nodes))
        .send_and_await_response(10); // Give it more time for reset operations

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(CacherResponse::Reset(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(CacherResponse::Reset(Err(err))) => {
                println!("✗ Failed to reset hypermap-cacher: {}", err);
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

    let response = Request::to(("our", "binding-cacher", "hypermap-cacher", "sys"))
        .body(BindingCacherRequest::Reset(binding_custom_nodes))
        .send_and_await_response(10); // Give it more time for reset operations

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(BindingCacherResponse::Reset(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(BindingCacherResponse::Reset(Err(err))) => {
                println!("✗ Failed to reset binding-cacher: {}", err);
            }
            _ => {
                println!("✗ Unexpected response from binding-cacher");
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
