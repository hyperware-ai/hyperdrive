use crate::kernel::process;
use crate::KERNEL_PROCESS_ID;
use crate::VFS_PROCESS_ID;
use anyhow::Result;

use lib::types::core::{self as t, STATE_PROCESS_ID};
pub use lib::wit;
pub use lib::wit::Host as StandardHost;

async fn print_debug(proc: &process::ProcessState, content: &str) {
    let _ = proc
        .send_to_terminal
        .send(t::Printout {
            verbosity: 2,
            content: format!("{}: {}", proc.metadata.our.process, content),
        })
        .await;
}

///
/// create the process API. this is where the functions that a process can use live.
///
#[async_trait::async_trait]
impl StandardHost for process::ProcessWasi {
    //
    // system utils:
    //

    /// Print a message to the runtime terminal. Add the name of the process to the
    /// beginning of the string, so user can verify source.
    async fn print_to_terminal(&mut self, verbosity: u8, content: String) -> Result<()> {
        self.process
            .send_to_terminal
            .send(t::Printout {
                verbosity,
                content: format!(
                    "{}:{}: {}",
                    self.process.metadata.our.process.package(),
                    self.process.metadata.our.process.publisher(),
                    content
                ),
            })
            .await
            .map_err(|e| anyhow::anyhow!("fatal: couldn't send to terminal: {e:?}"))
    }

    //
    // process management:
    //

    /// TODO critical: move to kernel logic to enable persistence of choice made here
    async fn set_on_exit(&mut self, on_exit: wit::OnExit) -> Result<()> {
        self.process.metadata.on_exit = t::OnExit::de_wit(on_exit);
        print_debug(&self.process, "set new on-exit behavior").await;
        Ok(())
    }

    async fn get_on_exit(&mut self) -> Result<wit::OnExit> {
        Ok(self.process.metadata.on_exit.en_wit())
    }

    /// create a message from the *kernel* to the filesystem,
    /// asking it to fetch the current state saved under this process
    async fn get_state(&mut self) -> Result<Option<Vec<u8>>> {
        let old_last_blob = self.process.last_blob.clone();
        let res = match process::send_and_await_response(
            self,
            Some(t::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.clone(),
            }),
            wit::Address {
                node: self.process.metadata.our.node.clone(),
                process: STATE_PROCESS_ID.en_wit(),
            },
            wit::Request {
                inherit: false,
                expects_response: Some(5),
                body: serde_json::to_vec(&t::StateAction::GetState(
                    self.process.metadata.our.process.clone(),
                ))
                .unwrap(),
                metadata: Some(self.process.metadata.our.process.to_string()),
                capabilities: vec![],
            },
            None,
        )
        .await
        {
            Ok(Ok(_resp)) => {
                // basically assuming filesystem responding properly here
                match &self.process.last_blob {
                    None => Ok(None),
                    Some(blob) => Ok(Some(blob.bytes.clone())),
                }
            }
            _ => Ok(None),
        };
        self.process.last_blob = old_last_blob;
        return res;
    }

    /// create a message from the *kernel* to the filesystem,
    /// asking it to replace the current state saved under
    /// this process with these bytes
    async fn set_state(&mut self, bytes: Vec<u8>) -> Result<()> {
        let old_last_blob = self.process.last_blob.clone();
        let res = match process::send_and_await_response(
            self,
            Some(t::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.clone(),
            }),
            wit::Address {
                node: self.process.metadata.our.node.clone(),
                process: STATE_PROCESS_ID.en_wit(),
            },
            wit::Request {
                inherit: false,
                expects_response: Some(5),
                body: serde_json::to_vec(&t::StateAction::SetState(
                    self.process.metadata.our.process.clone(),
                ))
                .unwrap(),
                metadata: Some(self.process.metadata.our.process.to_string()),
                capabilities: vec![],
            },
            Some(wit::LazyLoadBlob { mime: None, bytes }),
        )
        .await
        {
            Ok(Ok(_resp)) => {
                // basically assuming filesystem responding properly here
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "filesystem did not respond properly to SetState!!"
            )),
        };
        self.process.last_blob = old_last_blob;
        print_debug(&self.process, "persisted state").await;
        return res;
    }

    /// create a message from the *kernel* to the filesystem,
    /// asking it to delete the current state saved under this process
    async fn clear_state(&mut self) -> Result<()> {
        let old_last_blob = self.process.last_blob.clone();
        let res = match process::send_and_await_response(
            self,
            Some(t::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.clone(),
            }),
            wit::Address {
                node: self.process.metadata.our.node.clone(),
                process: STATE_PROCESS_ID.en_wit(),
            },
            wit::Request {
                inherit: false,
                expects_response: Some(5),
                body: serde_json::to_vec(&t::StateAction::DeleteState(
                    self.process.metadata.our.process.clone(),
                ))
                .unwrap(),
                metadata: None,
                capabilities: vec![],
            },
            None,
        )
        .await
        {
            Ok(Ok(_resp)) => {
                // basically assuming filesystem responding properly here
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "filesystem did not respond properly to ClearState!!"
            )),
        };
        self.process.last_blob = old_last_blob;
        print_debug(&self.process, "cleared persisted state").await;
        return res;
    }

    /// shortcut to spawn a new process. the child process will automatically
    /// be able to send messages to the parent process, and vice versa.
    /// the .wasm file for the process must already be in VFS.
    async fn spawn(
        &mut self,
        name: Option<String>,
        wasm_path: String, // must be located within package's drive
        on_exit: wit::OnExit,
        request_capabilities: Vec<wit::Capability>,
        grant_capabilities: Vec<wit::ProcessId>,
        public: bool,
    ) -> Result<Result<wit::ProcessId, wit::SpawnError>> {
        // save existing blob to restore later
        let old_last_blob = self.process.last_blob.clone();
        let vfs_address = wit::Address {
            node: self.process.metadata.our.node.clone(),
            process: VFS_PROCESS_ID.en_wit(),
        };
        let Ok(Ok((_, hash_response))) = process::send_and_await_response(
            self,
            None,
            vfs_address.clone(),
            wit::Request {
                inherit: false,
                expects_response: Some(5),
                body: serde_json::to_vec(&t::VfsRequest {
                    path: wasm_path.clone(),
                    action: t::VfsAction::Read,
                })
                .unwrap(),
                metadata: None,
                capabilities: vec![],
            },
            None,
        )
        .await
        else {
            println!("spawn: GetHash fail");
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };
        let wit::Message::Response((wit::Response { body, .. }, _)) = hash_response else {
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };
        let t::VfsResponse::Read = serde_json::from_slice(&body).unwrap() else {
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };
        let Some(t::LazyLoadBlob { mime: _, ref bytes }) = self.process.last_blob else {
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };

        let name = match name {
            Some(name) => name,
            None => rand::random::<u64>().to_string(),
        };
        let new_process_id = t::ProcessId::new(
            Some(&name),
            self.process.metadata.our.process.package(),
            self.process.metadata.our.process.publisher(),
        );
        // TODO I think we need to kill this process first in case it already exists
        let Ok(Ok((_, _response))) = process::send_and_await_response(
            self,
            Some(t::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.clone(),
            }),
            wit::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.en_wit(),
            },
            wit::Request {
                inherit: false,
                expects_response: Some(5), // TODO evaluate
                body: serde_json::to_vec(&t::KernelCommand::InitializeProcess {
                    id: new_process_id.clone(),
                    wasm_bytes_handle: wasm_path,
                    wit_version: Some(self.process.metadata.wit_version),
                    on_exit: t::OnExit::de_wit(on_exit),
                    initial_capabilities: request_capabilities
                        .iter()
                        .map(|cap| t::Capability {
                            issuer: t::Address::de_wit(cap.clone().issuer),
                            params: cap.clone().params,
                        })
                        .collect(),
                    public,
                })
                .unwrap(),
                metadata: None,
                capabilities: vec![],
            },
            Some(wit::LazyLoadBlob {
                mime: None,
                bytes: bytes.to_vec(),
            }),
        )
        .await
        else {
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NameTaken));
        };
        // insert messaging capabilities into requested processes
        for process in grant_capabilities {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.process
                .caps_oracle
                .send(t::CapMessage::Add {
                    on: t::ProcessId::de_wit(process),
                    caps: vec![t::Capability {
                        issuer: t::Address {
                            node: self.process.metadata.our.node.clone(),
                            process: new_process_id.clone(),
                        },
                        params: "\"messaging\"".into(),
                    }],
                    responder: tx,
                })
                .await
                .unwrap();
            let _ = rx.await.unwrap();
        }
        // finally, send the command to run the new process
        let Ok(Ok((_, response))) = process::send_and_await_response(
            self,
            Some(t::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.clone(),
            }),
            wit::Address {
                node: self.process.metadata.our.node.clone(),
                process: KERNEL_PROCESS_ID.en_wit(),
            },
            wit::Request {
                inherit: false,
                expects_response: Some(5), // TODO evaluate
                body: serde_json::to_vec(&t::KernelCommand::RunProcess(new_process_id.clone()))
                    .unwrap(),
                metadata: None,
                capabilities: vec![],
            },
            None,
        )
        .await
        else {
            // reset blob to what it was
            self.process.last_blob = old_last_blob;
            return Ok(Err(wit::SpawnError::NameTaken));
        };
        // reset blob to what it was
        self.process.last_blob = old_last_blob;
        let wit::Message::Response((wit::Response { body, .. }, _)) = response else {
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };
        let t::KernelResponse::StartedProcess = serde_json::from_slice(&body).unwrap() else {
            return Ok(Err(wit::SpawnError::NoFileAtPath));
        };
        // child processes are always able to Message parent
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.process
            .caps_oracle
            .send(t::CapMessage::Add {
                on: new_process_id.clone(),
                caps: vec![t::Capability {
                    issuer: self.process.metadata.our.clone(),
                    params: "\"messaging\"".into(),
                }],
                responder: tx,
            })
            .await
            .unwrap();
        let _ = rx.await.unwrap();

        // parent process is always able to Message child
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.process
            .caps_oracle
            .send(t::CapMessage::Add {
                on: self.process.metadata.our.process.clone(),
                caps: vec![t::Capability {
                    issuer: t::Address {
                        node: self.process.metadata.our.node.clone(),
                        process: new_process_id.clone(),
                    },
                    params: "\"messaging\"".into(),
                }],
                responder: tx,
            })
            .await
            .unwrap();
        let _ = rx.await.unwrap();
        print_debug(&self.process, "spawned a new process").await;
        Ok(Ok(new_process_id.en_wit().to_owned()))
    }

    //
    // capabilities management
    //

    async fn save_capabilities(&mut self, caps: Vec<wit::Capability>) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self
            .process
            .caps_oracle
            .send(t::CapMessage::Add {
                on: self.process.metadata.our.process.clone(),
                caps: caps
                    .iter()
                    .map(|cap| t::de_wit_capability(cap.clone()).0)
                    .collect(),
                responder: tx,
            })
            .await?;
        let _ = rx.await?;
        Ok(())
    }

    // TODO 0.6.0
    // async fn drop_capabilities(&mut self, caps: Vec<wit::Capability>) -> Result<()> {
    //     let (tx, rx) = tokio::sync::oneshot::channel();
    //     let _ = self
    //         .process
    //         .caps_oracle
    //         .send(t::CapMessage::Drop {
    //             on: self.process.metadata.our.process.clone(),
    //             caps: caps
    //                 .iter()
    //                 .map(|cap| t::de_wit_capability(cap.clone()).0)
    //                 .collect(),
    //             responder: tx,
    //         })
    //         .await?;
    //     let _ = rx.await?;
    //     Ok(())
    // }

    async fn our_capabilities(&mut self) -> Result<Vec<wit::Capability>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self
            .process
            .caps_oracle
            .send(t::CapMessage::GetAll {
                on: self.process.metadata.our.process.clone(),
                responder: tx,
            })
            .await?;
        let caps = rx.await?;
        Ok(caps
            .into_iter()
            .map(|cap| wit::Capability {
                issuer: t::Address::en_wit(&cap.0.issuer),
                params: cap.0.params,
            })
            .collect())
    }

    //
    // message I/O:
    //

    /// from a process: receive the next incoming message. will wait async until a message is received.
    /// the incoming message can be a Request or a Response, or an Error of the Network variety.
    async fn receive(
        &mut self,
    ) -> Result<Result<(wit::Address, wit::Message), (wit::SendError, Option<wit::Context>)>> {
        Ok(self.process.get_next_message_for_process().await)
    }

    /// from a process: grab the blob part of the current prompting message.
    /// if the prompting message did not have a blob, will return None.
    /// will also return None if there is no prompting message.
    async fn get_blob(&mut self) -> Result<Option<wit::LazyLoadBlob>> {
        Ok(t::en_wit_blob(self.process.last_blob.clone()))
    }

    async fn send_request(
        &mut self,
        target: wit::Address,
        request: wit::Request,
        context: Option<wit::Context>,
        blob: Option<wit::LazyLoadBlob>,
    ) -> Result<()> {
        let id = self
            .process
            .send_request(None, target, request, context, blob)
            .await;
        match id {
            Ok(_id) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn send_requests(
        &mut self,
        requests: Vec<(
            wit::Address,
            wit::Request,
            Option<wit::Context>,
            Option<wit::LazyLoadBlob>,
        )>,
    ) -> Result<()> {
        for request in requests {
            let id = self
                .process
                .send_request(None, request.0, request.1, request.2, request.3)
                .await;
            match id {
                Ok(_id) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn send_response(
        &mut self,
        response: wit::Response,
        blob: Option<wit::LazyLoadBlob>,
    ) -> Result<()> {
        self.process.send_response(response, blob).await;
        Ok(())
    }

    async fn send_and_await_response(
        &mut self,
        target: wit::Address,
        request: wit::Request,
        blob: Option<wit::LazyLoadBlob>,
    ) -> Result<Result<(wit::Address, wit::Message), wit::SendError>> {
        process::send_and_await_response(self, None, target, request, blob).await
    }
}
