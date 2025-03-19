use crate::hyperware::process::sign;
use anyhow::{anyhow, Result};
use hyperware_process_lib::logging::{error, init_logging, Level};
use hyperware_process_lib::net::{NetAction, NetResponse};
use hyperware_process_lib::{await_message, call_init, Address, Message, Request, Response};

wit_bindgen::generate!({
    path: "target/wit",
    world: "sign-sys-v0",
    generate_unused_types: true,
    additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(initialize);
fn initialize(our: Address) {
    // this seems to require calling vfs:distro:sys
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    loop {
        match await_message() {
            Err(send_error) => error!("got SendError: {send_error}"),
            Ok(ref message) => {
                match message {
                    Message::Request { body, .. } => match handle_request(&our, &body) {
                        Ok(_) => continue,
                        Err(request_error) => {
                            error!("error handling sign request: \n{request_error}")
                        }
                    },
                    _ => error!("Response received at sign process"), // we are awaiting all requests no resposes should be received
                }
            }
        }
    }
}

fn handle_request(our: &Address, request_bytes: &[u8]) -> Result<()> {
    match request_bytes.try_into()? {
        sign::Request::Sign(bytes) => handle_sign(bytes),
        sign::Request::Verify(req) => handle_verify(our, req),
    }
}

fn handle_sign(bytes: Vec<u8>) -> Result<()> {
    let body = rmp_serde::to_vec(&NetAction::Sign)?;
    let res = Request::to(("our", "net", "distro", "sys"))
        .blob_bytes(bytes.clone())
        .body(body)
        .send_and_await_response(10)??;
    let Ok(NetResponse::Signed) = rmp_serde::from_slice::<NetResponse>(res.body()) else {
        return Err(anyhow!("signature failed"));
    };
    let signature = res.blob();
    match signature {
        None => Err(anyhow!("no blob")),
        Some(b) => {
            let sign_response = sign::SignResponse {
                message: bytes,
                signature: b.bytes().to_vec(),
            };
            let sign_response_bytes = serde_json::to_vec(&sign_response)?;
            Response::new().body(sign_response_bytes).send()?;
            Ok(())
        }
    }
}
fn handle_verify(our: &Address, req: sign::VerifyRequest) -> Result<()> {
    let process = our.to_owned().process;
    let from = Address::new(req.node, process);
    let body = rmp_serde::to_vec(&NetAction::Verify {
        from,
        signature: req.signature,
    })?;
    let req_bytes = rmp_serde::to_vec(&body)?;
    let res = Request::to(("our", "net", "distro", "sys"))
        .blob_bytes(req_bytes)
        .body(body)
        .send_and_await_response(10)??;
    let resp = rmp_serde::from_slice::<NetResponse>(res.body())?;
    match resp {
        NetResponse::Verified(is_good) => {
            Response::new().body(serde_json::to_vec(&is_good)?).send()?;
            Ok(())
        }
        _ => Err(anyhow!("weird response")),
    }
}
