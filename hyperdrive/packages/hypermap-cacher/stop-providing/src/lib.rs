use crate::hyperware::process::hypermap_cacher::{CacherRequest, CacherResponse};
use hyperware_process_lib::{call_init, println, Address, Request, Response};

wit_bindgen::generate!({
    path: "target/wit",
    world: "process-v1",
    generate_unused_types: false,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(our: Address) {
    println!("Disabling hypermap-cacher provider mode...");

    let response = Request::new()
        .target((&our.node, "hypermap-cacher", "hypermap-cacher", "sys"))
        .body(CacherRequest::StopProviding)
        .send_and_await_response(5);

    match response {
        Ok(Ok(message)) => match message.body().try_into() {
            Ok(CacherResponse::StopProviding(Ok(msg))) => {
                println!("✓ {}", msg);
            }
            Ok(CacherResponse::StopProviding(Err(err))) => {
                println!("✗ Failed to disable provider mode: {}", err);
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
