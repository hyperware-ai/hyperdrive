use anyhow::{anyhow, Result};
use hyperware_process_lib::net::{NetAction, NetResponse};
use hyperware_process_lib::{await_message, call_init, Address, Message, Request, Response};
use serde::{Deserialize, Serialize};

wit_bindgen::generate!({
    path: "target/wit",
    world: "sign-sys-v0",
    generate_unused_types: true,
    additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

#[derive(Debug, Serialize, Deserialize)]
enum SignerRequest {
    Sign(SignRequest),
    Verify { from: Address, data: SignResponse },
}
#[derive(Debug, Serialize, Deserialize)]
struct SignRequest {
    pub site: String,
    pub time: u64,
    pub nonce: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct SignResponse {
    pub body: SignRequest,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

call_init!(initialize);
fn initialize(our: Address) {
    loop {
        let msg = await_message();
        match msg {
            Err(_send_error) => {
                // ignore send errors, local-only process
                continue;
            }
            Ok(Message::Request { body, .. }) => handle_request(&our, &body).unwrap_or_default(),
            _ => continue, // ignore responses
        }
    }
}

fn handle_request(our: &Address, request_bytes: &[u8]) -> Result<()> {
    let req = serde_json::from_slice::<SignerRequest>(request_bytes)?;
    match req {
        SignerRequest::Sign(r) => handle_sign(our, r, request_bytes),
        SignerRequest::Verify { from, data } => handle_verify(from, data),
    }
}

fn handle_sign(our: &Address, req: SignRequest, request_bytes: &[u8]) -> Result<()> {
    let body = rmp_serde::to_vec(&NetAction::Sign)?;
    let res = Request::to(("our", "net", "distro", "sys"))
        .blob_bytes(request_bytes)
        .body(body)
        .send_and_await_response(10)??;
    let Ok(NetResponse::Signed) = rmp_serde::from_slice::<NetResponse>(res.body()) else {
        return Err(anyhow!("signature failed"));
    };
    let newblob = res.blob();
    let message = [our.to_string().as_bytes(), request_bytes].concat();
    match newblob {
        None => Err(anyhow!("no blob")),
        Some(b) => {
            let lr = SignResponse {
                body: req,
                message,
                signature: b.bytes().to_vec(),
            };
            let lrj = serde_json::to_vec(&lr)?;
            Response::new().body(lrj).send()?;
            Ok(())
        }
    }
}
fn handle_verify(from: Address, data: SignResponse) -> Result<()> {
    let signature = data.signature;
    let body = rmp_serde::to_vec(&NetAction::Verify { from, signature })?;
    let req_bytes = rmp_serde::to_vec(&data.body)?;
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
