use crate::hyperware::process::binding_cacher::{BindingCacherRequest, BindingCacherResponse};
use crate::hyperware::process::hypermap_cacher::{CacherRequest, CacherResponse};
use hyperware_process_lib::{call_init, println, Address, Request};

wit_bindgen::generate!({
    path: "../target/wit",
    world: "hypermap-cacher-sys-v1",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(_our: Address) {
    println!("Enabling hypermap-cacher provider mode...");

    let response = Request::to(("our", "hypermap-cacher", "hypermap-cacher", "sys"))
        .body(CacherRequest::StartProviding)
        .send_and_await_response(5);

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(CacherResponse::StartProviding(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(CacherResponse::StartProviding(Err(err))) => {
                println!("✗ Failed to enable hypermap-cacher provider mode: {}", err);
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

    println!("Enabling binding-cacher provider mode...");

    let response = Request::to(("our", "binding-cacher", "hypermap-cacher", "sys"))
        .body(BindingCacherRequest::StartProviding)
        .send_and_await_response(5);

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(BindingCacherResponse::StartProviding(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(BindingCacherResponse::StartProviding(Err(err))) => {
                println!("✗ Failed to enable binding-cacher provider mode: {}", err);
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
