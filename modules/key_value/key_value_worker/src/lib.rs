cargo_component_bindings::generate!();

use std::collections::HashMap;

use redb::ReadableTable;
use serde::{Deserialize, Serialize};

use bindings::component::uq_process::types::*;
use bindings::{get_payload, Guest, print_to_terminal, receive, send_and_await_response, send_response};

mod kernel_types;
use kernel_types as kt;
mod key_value_types;
use key_value_types as kv;
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
    target_process: String,
    target_package: String,
    target_publisher: String,
    request_ipc: Option<String>,
    request_metadata: Option<String>,
    payload: Option<(Option<String>, Vec<u8>)>,
    timeout: u64,
) -> (Option<String>, Option<String>) {
    let payload = match payload {
        None => None,
        Some((mime, bytes)) => Some(Payload { mime, bytes }),
    };
    let (
        _,
        Message::Response((Response { ipc, metadata }, _)),
    ) = send_and_await_response(
        &Address {
            node: target_node,
            process: kt::ProcessId::new(
                &target_process,
                &target_package,
                &target_publisher,
            ).en_wit(),
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
    (ipc, metadata)
}

fn handle_message (
    our: &Address,
    db_handle: &mut Option<redb::Database>,
) -> anyhow::Result<()> {
    let (source, message) = receive().unwrap();
    // let (source, message) = receive()?;

    if our.node != source.node {
        return Err(kv::KeyValueError::RejectForeign.into());
    }

    match message {
        Message::Response(_) => { unimplemented!() },
        Message::Request(Request { inherit: _ , expects_response: _, ipc, metadata: _ }) => {
            match process_lib::parse_message_ipc(ipc.clone())? {
                kv::KeyValueMessage::New { db } => {
                    let vfs_drive = format!("{}{}", PREFIX, db);
                    match db_handle {
                        Some(_) => {
                            return Err(kv::KeyValueError::DbAlreadyExists.into());
                        },
                        None => {
                            print_to_terminal(0, "key_value_worker: Create");
                            *db_handle = Some(redb::Database::create(
                                format!("/{}.redb", db),
                                our.node.clone(),
                                vfs_drive,
                                get_payload_wrapped,
                                send_and_await_response_wrapped,
                            )?);
                            print_to_terminal(0, "key_value_worker: Create done");
                        },
                    }
                },
                kv::KeyValueMessage::Write { ref key, .. } => {
                    let Some(db_handle) = db_handle else {
                        return Err(kv::KeyValueError::DbDoesNotExist.into());
                    };

                    let Payload { mime: _, ref bytes } = get_payload().ok_or(anyhow::anyhow!("couldnt get bytes for Write"))?;

                    let write_txn = db_handle.begin_write()?;
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
                kv::KeyValueMessage::Read { ref key, .. } => {
                    let Some(db_handle) = db_handle else {
                        return Err(kv::KeyValueError::DbDoesNotExist.into());
                    };

                    let read_txn = db_handle.begin_read()?;

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
                            let bytes = v.value().to_vec();
                            print_to_terminal(
                                1,
                                &format!(
                                    "key_value_worker: key, val: {:?}, {}",
                                    key,
                                    if bytes.len() < 100 { format!("{:?}", bytes) } else { "<elided>".into() },
                                ),
                            );
                            send_response(
                                &Response {
                                    ipc,
                                    metadata: None,
                                },
                                Some(&Payload {
                                    mime: None,
                                    bytes,
                                }),
                            );
                        },
                    };
                },
                kv::KeyValueMessage::Err { error } => {
                    return Err(error.into());
                }
            }

            Ok(())
        },
    }
}

impl Guest for Component {
    fn init(our: Address) {
        print_to_terminal(1, "key_value_worker: begin");

        let mut db_handle: Option<redb::Database> = None;

        loop {
            match handle_message(&our, &mut db_handle) {
                Ok(()) => {},
                Err(e) => {
                    print_to_terminal(0, format!(
                        "key_value_worker: error: {:?}",
                        e,
                    ).as_str());
                    if let Some(e) = e.downcast_ref::<kv::KeyValueError>() {
                        send_response(
                            &Response {
                                ipc: Some(serde_json::to_string(&e).unwrap()),
                                metadata: None,
                            },
                            None,
                        );
                    }
                    panic!("");
                },
            };
        }
    }
}
