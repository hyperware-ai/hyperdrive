use crate::eth::*;
use alloy_pubsub::RawSubscription;
use alloy_rpc_types::pubsub::SubscriptionResult;
use anyhow::Result;
use std::collections::HashMap;

/// cleans itself up when the subscription is closed or fails.
pub async fn create_new_subscription(
    our: String,
    km_id: u64,
    target: Address,
    rsvp: Option<Address>,
    send_to_loop: MessageSender,
    sub_id: u64,
    eth_action: EthAction,
    providers: Providers,
    active_subscriptions: ActiveSubscriptions,
    response_channels: ResponseChannels,
    print_tx: PrintSender,
) {
    verbose_print(&print_tx, "eth: creating new subscription").await;
    match build_subscription(
        &our,
        km_id,
        &target,
        &send_to_loop,
        &eth_action,
        &providers,
        &response_channels,
        &print_tx,
    )
    .await
    {
        Ok(maybe_raw_sub) => {
            // send a response to the target that the subscription was successful
            kernel_message(
                &our,
                km_id,
                target.clone(),
                rsvp.clone(),
                false,
                None,
                EthResponse::Ok,
                &send_to_loop,
            )
            .await;
            let mut subs = active_subscriptions
                .entry(target.clone())
                .or_insert(HashMap::new());
            let active_subscriptions = active_subscriptions.clone();
            match maybe_raw_sub {
                Ok(rx) => {
                    subs.insert(
                        sub_id,
                        // this is a local sub, as in, we connect to the rpc endpt
                        ActiveSub::Local(tokio::spawn(async move {
                            // await the subscription error and kill it if so
                            if let Err(e) = maintain_local_subscription(
                                &our,
                                sub_id,
                                rx,
                                &target,
                                &rsvp,
                                &send_to_loop,
                            )
                            .await
                            {
                                verbose_print(
                                    &print_tx,
                                    "eth: closed local subscription due to error",
                                )
                                .await;
                                kernel_message(
                                    &our,
                                    rand::random(),
                                    target.clone(),
                                    rsvp,
                                    true,
                                    None,
                                    EthSubResult::Err(e),
                                    &send_to_loop,
                                )
                                .await;
                                active_subscriptions.entry(target).and_modify(|sub_map| {
                                    sub_map.remove(&km_id);
                                });
                            }
                        })),
                    );
                }
                Err((provider_node, remote_sub_id)) => {
                    // this is a remote sub, given by a relay node
                    let (sender, rx) = tokio::sync::mpsc::channel(10);
                    let keepalive_km_id = rand::random();
                    let (keepalive_err_sender, keepalive_err_receiver) =
                        tokio::sync::mpsc::channel(1);
                    response_channels.insert(keepalive_km_id, keepalive_err_sender);
                    subs.insert(
                        remote_sub_id,
                        ActiveSub::Remote {
                            provider_node: provider_node.clone(),
                            handle: tokio::spawn(async move {
                                if let Err(e) = maintain_remote_subscription(
                                    &our,
                                    &provider_node,
                                    remote_sub_id,
                                    sub_id,
                                    keepalive_km_id,
                                    rx,
                                    keepalive_err_receiver,
                                    &target,
                                    &send_to_loop,
                                )
                                .await
                                {
                                    verbose_print(
                                        &print_tx,
                                        "eth: closed subscription with provider node due to error",
                                    )
                                    .await;
                                    kernel_message(
                                        &our,
                                        rand::random(),
                                        target.clone(),
                                        None,
                                        true,
                                        None,
                                        EthSubResult::Err(e),
                                        &send_to_loop,
                                    )
                                    .await;
                                    active_subscriptions.entry(target).and_modify(|sub_map| {
                                        sub_map.remove(&sub_id);
                                    });
                                    response_channels.remove(&keepalive_km_id);
                                }
                            }),
                            sender,
                        },
                    );
                }
            }
        }
        Err(e) => {
            error_message(&our, km_id, target.clone(), e, &send_to_loop).await;
        }
    }
}

/// terrible abuse of result in return type, yes, sorry
async fn build_subscription(
    our: &str,
    km_id: u64,
    target: &Address,
    send_to_loop: &MessageSender,
    eth_action: &EthAction,
    providers: &Providers,
    response_channels: &ResponseChannels,
    print_tx: &PrintSender,
) -> Result<Result<RawSubscription, (String, u64)>, EthError> {
    let EthAction::SubscribeLogs {
        chain_id,
        kind,
        params,
        ..
    } = eth_action
    else {
        return Err(EthError::PermissionDenied); // will never hit
    };
    let Some(mut aps) = providers.get_mut(&chain_id) else {
        return Err(EthError::NoRpcForChain);
    };
    // first, try any url providers we have for this chain,
    // then if we have none or they all fail, go to node providers.
    // finally, if no provider works, return an error.
    for url_provider in &mut aps.urls {
        let pubsub = match &url_provider.pubsub {
            Some(pubsub) => pubsub,
            None => {
                if let Ok(()) = activate_url_provider(url_provider).await {
                    verbose_print(print_tx, "eth: activated a url provider").await;
                    url_provider.pubsub.as_ref().unwrap()
                } else {
                    continue;
                }
            }
        };
        let kind = serde_json::to_value(&kind).unwrap();
        let params = serde_json::to_value(&params).unwrap();
        if let Ok(id) = pubsub
            .inner()
            .prepare("eth_subscribe", [kind, params])
            .await
        {
            let rx = pubsub.inner().get_raw_subscription(id).await;
            return Ok(Ok(rx));
        }
        // this provider failed and needs to be reset
        url_provider.pubsub = None;
    }
    // now we need a response channel
    let (sender, mut response_receiver) = tokio::sync::mpsc::channel(1);
    response_channels.insert(km_id, sender);
    // we need to create our own unique sub id because in the remote provider node,
    // all subs will be identified under our process address.
    let remote_sub_id = rand::random();
    for node_provider in &mut aps.nodes {
        match forward_to_node_provider(
            &our,
            km_id,
            Some(target.clone()),
            node_provider,
            EthAction::SubscribeLogs {
                sub_id: remote_sub_id,
                chain_id: chain_id.clone(),
                kind: kind.clone(),
                params: params.clone(),
            },
            &send_to_loop,
            &mut response_receiver,
        )
        .await
        {
            EthResponse::Ok => {
                kernel_message(
                    &our,
                    km_id,
                    target.clone(),
                    None,
                    false,
                    None,
                    EthResponse::Ok,
                    &send_to_loop,
                )
                .await;
                response_channels.remove(&km_id);
                return Ok(Err((node_provider.name.clone(), remote_sub_id)));
            }
            EthResponse::Response { .. } => {
                // the response to a SubscribeLogs request must be an 'ok'
                node_provider.usable = false;
            }
            EthResponse::Err(e) => {
                if e == EthError::RpcMalformedResponse {
                    node_provider.usable = false;
                }
            }
        }
    }
    return Err(EthError::NoRpcForChain);
}

async fn maintain_local_subscription(
    our: &str,
    sub_id: u64,
    mut rx: RawSubscription,
    target: &Address,
    rsvp: &Option<Address>,
    send_to_loop: &MessageSender,
) -> Result<(), EthSubError> {
    while let Ok(value) = rx.recv().await {
        let result: SubscriptionResult =
            serde_json::from_str(value.get()).map_err(|e| EthSubError {
                id: sub_id,
                error: e.to_string(),
            })?;
        kernel_message(
            our,
            rand::random(),
            target.clone(),
            rsvp.clone(),
            true,
            None,
            EthSubResult::Ok(EthSub { id: sub_id, result }),
            &send_to_loop,
        )
        .await;
    }
    Err(EthSubError {
        id: sub_id,
        error: "subscription closed unexpectedly".to_string(),
    })
}

/// handle the subscription updates from a remote provider,
/// and also perform keepalive checks on that provider.
/// current keepalive is 30s, this can be adjusted as desired
async fn maintain_remote_subscription(
    our: &str,
    provider_node: &str,
    remote_sub_id: u64,
    sub_id: u64,
    keepalive_km_id: u64,
    mut rx: tokio::sync::mpsc::Receiver<EthSubResult>,
    mut net_error_rx: ProcessMessageReceiver,
    target: &Address,
    send_to_loop: &MessageSender,
) -> Result<(), EthSubError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    loop {
        tokio::select! {
            incoming = rx.recv() => {
                match incoming {
                    Some(EthSubResult::Ok(upd)) => {
                        kernel_message(
                            &our,
                            rand::random(),
                            target.clone(),
                            None,
                            true,
                            None,
                            EthSubResult::Ok(EthSub {
                                id: sub_id,
                                result: upd.result,
                            }),
                            &send_to_loop,
                        )
                        .await;
                    }
                    Some(EthSubResult::Err(e)) => {
                        return Err(EthSubError {
                            id: sub_id,
                            error: e.error,
                        });
                    }
                    None => {
                        return Err(EthSubError {
                            id: sub_id,
                            error: "subscription closed unexpectedly".to_string(),
                        });

                    }
                }
            }
            _ = interval.tick() => {
                // perform keepalive
                kernel_message(
                    &our,
                    keepalive_km_id,
                    Address { node: provider_node.to_string(), process: ETH_PROCESS_ID.clone() },
                    None,
                    true,
                    Some(30),
                    IncomingReq::SubKeepalive(remote_sub_id),
                    &send_to_loop,
                ).await;
            }
            incoming = net_error_rx.recv() => {
                if let Some(Err(_net_error)) = incoming {
                    return Err(EthSubError {
                        id: sub_id,
                        error: "subscription node-provider failed keepalive".to_string(),
                    });
                }
            }
        }
    }
}
