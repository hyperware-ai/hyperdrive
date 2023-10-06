cargo_component_bindings::generate!();

use std::collections::HashMap;

use redb::ReadableTable;
use serde::{Deserialize, Serialize};

use bindings::component::uq_process::types::*;
use bindings::{get_payload, Guest, print_to_terminal, receive, send_and_await_response, send_response};

mod kernel_types;
use kernel_types as kt;
mod process_lib;

struct Component;

const PREFIX: &str = "key_value-";
const TABLE: redb::TableDefinition<&[u8], &[u8]> = redb::TableDefinition::new("process");

fn get_payload_wrapped() -> Option<(Option<String>, Vec<u8>)> {
   match get_payload() {
       None => None,
       Some(Payload { mime, bytes }) => Some((mime, bytes)),
   }
}

fn send_and_await_response_wrapped(
    target_node: String,
    target_process: Result<u64, String>,
    request_ipc: Option<String>,
    request_metadata: Option<String>,
    payload: Option<(Option<String>, Vec<u8>)>,
    timeout: u64,
) -> (
    (String, Result<u64, String>),
    (Option<String>, Option<String>),
) {
    let payload = match payload {
        None => None,
        Some((mime, bytes)) => Some(Payload { mime, bytes }),
    };
    let (
        Address { node, process },
        Message::Response((Response { ipc, metadata }, _)),
    ) = send_and_await_response(
        &Address {
            node: target_node,
            process: match target_process {
                Ok(id) => ProcessId::Id(id),
                Err(name) => ProcessId::Name(name),
            },
        },
        &Request {
            inherit: false,
            expects_response: Some(timeout),
            ipc: request_ipc,
            metadata: request_metadata,
        },
        match payload {
            None => None,
            Some(ref p) => Some(p),
        },
    ).unwrap() else {
        panic!("");
    };
    (
        (
            node,
            match process {
                ProcessId::Id(id) => Ok(id),
                ProcessId::Name(name) => Err(name),
            },
        ),
        (ipc, metadata)
    )
}

fn handle_message (
    our: &Address,
    db: &mut Option<redb::Database>,
) -> anyhow::Result<()> {
    let (source, message) = receive().unwrap();
    // let (source, message) = receive()?;

    if our.node != source.node {
        return Err(anyhow::anyhow!(
            "rejecting foreign Message from {:?}",
            source,
        ));
    }

    match message {
        Message::Response(_) => { unimplemented!() },
        Message::Request(Request { inherit: _ , expects_response: _, ipc, metadata: _ }) => {
            match process_lib::parse_message_ipc(ipc.clone())? {
                kt::KeyValueMessage::New { drive: kv_drive } => {
                    let vfs_drive = format!("{}{}", PREFIX, kv_drive);
                    match db {
                        Some(_) => {
                            return Err(anyhow::anyhow!("cannot send New more than once"));
                        },
                        None => {
                            *db = Some(redb::Database::create(
                                format!(
                                    "/{}.redb",
                                    kv_drive,
                                ),
                                vfs_drive,
                                get_payload_wrapped,
                                send_and_await_response_wrapped,
                            )?);
                        },
                    }
                },
                kt::KeyValueMessage::Write { drive: _, ref key } => {
                    let Some(db) = db else {
                        return Err(anyhow::anyhow!("cannot send New more than once"));
                    };

                    let Payload { mime: _, ref bytes } = get_payload().ok_or(anyhow::anyhow!("couldnt get bytes for Write"))?;

                    let write_txn = db.begin_write()?;
                    {
                        let mut table = write_txn.open_table(TABLE)?;
                        table.insert(&key[..], &bytes[..])?;
                    }
                    write_txn.commit()?;

                    send_response(
                        &Response {
                            ipc,
                            metadata: None,
                        },
                        None,
                    );
                },
                kt::KeyValueMessage::Read { drive: _, ref key } => {
                    let Some(db) = db else {
                        return Err(anyhow::anyhow!("cannot send New more than once"));
                    };

                    let read_txn = db.begin_read()?;

                    let table = read_txn.open_table(TABLE)?;

                    match table.get(&key[..])? {
                        None => {
                            send_response(
                                &Response {
                                    ipc,
                                    metadata: None,
                                },
                                None,
                            );
                        },
                        Some(v) => {
                            send_response(
                                &Response {
                                    ipc,
                                    metadata: None,
                                },
                                Some(&Payload {
                                    mime: None,
                                    bytes: v.value().to_vec(),
                                }),
                            );
                        },
                    };
                },
            }

            Ok(())
        },
    }
}

impl Guest for Component {
    fn init(our: Address) {
        print_to_terminal(1, "key_value: begin");

        let mut db: Option<redb::Database> = None;

        loop {
            match handle_message(&our, &mut db) {
                Ok(()) => {},
                Err(e) => {
                    //  TODO: should we send an error on failure?
                    print_to_terminal(0, format!(
                        "key_value_worker: error: {:?}",
                        e,
                    ).as_str());
                },
            };
        }
    }
}
