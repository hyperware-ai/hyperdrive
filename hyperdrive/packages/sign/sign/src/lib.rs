use anyhow::{anyhow, Result};

use crate::hyperware::process::sign;
use hyperware_process_lib::logging::{error, init_logging, Level};
use hyperware_process_lib::net::{NetAction, NetResponse};
use hyperware_process_lib::{
    await_message, call_init, get_blob, our, Address, LazyLoadBlob, Message, Request, Response,
};

wit_bindgen::generate!({
    path: "target/wit",
    world: "sign-sys-v0",
    generate_unused_types: true,
    additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(initialize);
fn initialize(_our: Address) {
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    loop {
        match await_message() {
            Err(send_error) => error!("got SendError: {send_error}"),
            Ok(ref message) => {
                if let Err(request_error) = handle_message(message) {
                    error!("error handling sign request: \n{request_error}")
                }
            }
        }
    }
}

fn handle_message(message: &Message) -> Result<()> {
    if !message.is_request() {
        if message.source() == &Address::new("our", ("vfs", "distro", "sys")) {
            return Ok(());
        }
        return Err(anyhow!("Response received at sign process"));
    }

    let body = message.body();
    let source = message.source();
    match body.try_into()? {
        sign::Request::NetKeySign => handle_sign(source),
        sign::Request::NetKeyVerify(req) => handle_verify(source, req),
        sign::Request::NetKeyMakeMessage => handle_make_message(source),
    }
}

fn handle_sign(source: &Address) -> Result<()> {
    let body = rmp_serde::to_vec(&NetAction::Sign)?;

    let message = make_message(source, &get_blob().unwrap_or(LazyLoadBlob::default()));

    let res = Request::to(("our", "net", "distro", "sys"))
        .blob(LazyLoadBlob {
            mime: None,
            bytes: message,
        })
        .body(body)
        .send_and_await_response(10)??;

    let Ok(NetResponse::Signed) = rmp_serde::from_slice::<NetResponse>(res.body()) else {
        return Err(anyhow!("signature failed"));
    };
    let Some(signature) = res.blob() else {
        return Err(anyhow!("no blob"));
    };

    Response::new()
        .blob(signature)
        .body(sign::Response::NetKeySign)
        .send()?;
    Ok(())
}

fn handle_verify(source: &Address, req: sign::NetKeyVerifyRequest) -> Result<()> {
    let Some(blob) = get_blob() else {
        return Err(anyhow!("no blob"));
    };
    let message = make_message(source, &blob);
    let our_process = our().process;
    let from = Address::new("our", our_process);
    let body = rmp_serde::to_vec(&NetAction::Verify {
        from,
        signature: req.signature,
    })?;
    let res = Request::to(("our", "net", "distro", "sys"))
        .blob(LazyLoadBlob {
            mime: None,
            bytes: message,
        })
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

fn handle_make_message(source: &Address) -> Result<()> {
    let our_process = our().process;
    let from = Address::new("our", our_process);

    let message = make_message(source, &get_blob().unwrap_or(LazyLoadBlob::default()));
    let message = [from.to_string().as_bytes(), &message].concat();

    Response::new()
        .blob(LazyLoadBlob {
            mime: None,
            bytes: message,
        })
        .body(sign::Response::NetKeySign)
        .send()?;
    Ok(())
}

/// net:distro:sys prepends the message to sign with the sender of the request
///
/// since any sign requests passed through sign:sign:sys will look to net:distro:sys
///  like they come from sign:sign:sys, we additionally prepend the message with
///  source here
///
/// so final message to be signed looks like
///
/// [sign-address, source, blob.bytes].concat()
fn make_message(source: &Address, blob: &LazyLoadBlob) -> Vec<u8> {
    [source.to_string().as_bytes(), &blob.bytes].concat()
}
