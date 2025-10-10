use crate::eth::{Providers, ResponseChannels, UrlProvider};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use lib::types::core::*;
use lib::types::eth::*;
use serde::Serialize;
use std::time::{Duration, Instant};
use url::Url;

pub async fn activate_url_provider(provider: &mut UrlProvider) -> Result<()> {
    match Url::parse(&provider.url)?.scheme() {
        "ws" | "wss" => {
            let ws = WsConnect {
                url: provider.url.to_string(),
                auth: provider.auth.clone().map(|a| a.into()),
                config: None,
            };

            let client = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                ProviderBuilder::new().on_ws(ws),
            )
            .await??;
            provider.pubsub.push(client);
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Only `ws://` or `wss://` providers are supported."
        )),
    }
}

pub fn providers_to_saved_configs(providers: &Providers) -> SavedConfigs {
    SavedConfigs(
        providers
            .iter()
            .map(|entry| {
                entry
                    .urls
                    .iter()
                    .map(|url_provider| ProviderConfig {
                        chain_id: *entry.key(),
                        provider: NodeOrRpcUrl::RpcUrl {
                            url: url_provider.url.clone(),
                            auth: url_provider.auth.clone(),
                        },
                        trusted: url_provider.trusted,
                    })
                    .chain(entry.nodes.iter().map(|node_provider| ProviderConfig {
                        chain_id: *entry.key(),
                        provider: NodeOrRpcUrl::Node {
                            hns_update: node_provider.hns_update.clone(),
                            use_as_provider: node_provider.usable,
                        },
                        trusted: node_provider.trusted,
                    }))
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect(),
    )
}

pub async fn check_for_root_cap(
    our: &str,
    process: &ProcessId,
    caps_oracle: &CapMessageSender,
) -> bool {
    let (send_cap_bool, recv_cap_bool) = tokio::sync::oneshot::channel();
    caps_oracle
        .send(CapMessage::Has {
            on: process.clone(),
            cap: Capability::new((our, ETH_PROCESS_ID.clone()), "{\"root\":true}"),
            responder: send_cap_bool,
        })
        .await
        .expect("eth: capability oracle died!");
    recv_cap_bool.await.unwrap_or(false)
}

pub async fn verbose_print(print_tx: &PrintSender, content: &str) {
    let _ = print_tx
        .send(Printout::new(
            2,
            NET_PROCESS_ID.clone(),
            content.to_string(),
        ))
        .await;
}

pub async fn error_message(
    our: &str,
    km_id: u64,
    target: Address,
    error: EthError,
    send_to_loop: &MessageSender,
) {
    kernel_message(
        our,
        km_id,
        target,
        None,
        false,
        None,
        EthResponse::Err(error),
        send_to_loop,
    )
    .await
}

pub async fn kernel_message<T: Serialize>(
    our: &str,
    km_id: u64,
    target: Address,
    rsvp: Option<Address>,
    req: bool,
    timeout: Option<u64>,
    body: T,
    send_to_loop: &MessageSender,
) {
    let Err(e) = send_to_loop.try_send(KernelMessage {
        id: km_id,
        source: Address {
            node: our.to_string(),
            process: ETH_PROCESS_ID.clone(),
        },
        target,
        rsvp,
        message: if req {
            Message::Request(Request {
                inherit: false,
                expects_response: timeout,
                body: serde_json::to_vec(&body).unwrap(),
                metadata: None,
                capabilities: vec![],
            })
        } else {
            Message::Response((
                Response {
                    inherit: false,
                    body: serde_json::to_vec(&body).unwrap(),
                    metadata: None,
                    capabilities: vec![],
                },
                None,
            ))
        },
        lazy_load_blob: None,
    }) else {
        // not Err -> send successful; done here
        return;
    };
    // its an Err: handle
    match e {
        tokio::sync::mpsc::error::TrySendError::Closed(_) => {
            return;
        }
        tokio::sync::mpsc::error::TrySendError::Full(_) => {
            // TODO: implement backpressure
            panic!("(eth) kernel overloaded with messages: TODO: implement backpressure");
        }
    }
}

pub fn find_index(vec: &Vec<&str>, item: &str) -> Option<usize> {
    vec.iter().enumerate().find_map(
        |(index, value)| {
            if *value == item {
                Some(index)
            } else {
                None
            }
        },
    )
}

pub async fn set_node_unusable(
    providers: &Providers,
    chain_id: &u64,
    node_name: &str,
    print_tx: &PrintSender,
) -> bool {
    let mut is_replacement_successful = true;
    providers.entry(chain_id.clone()).and_modify(|aps| {
        let Some(index) = find_index(
            &aps.nodes
                .iter()
                .map(|n| n.hns_update.name.as_str())
                .collect(),
            &node_name,
        ) else {
            is_replacement_successful = false;
            return ();
        };
        let mut node = aps.nodes.remove(index);
        node.usable = false;
        aps.nodes.insert(index, node);
    });
    if !is_replacement_successful {
        verbose_print(
            print_tx,
            &format!("eth: unexpectedly couldn't find provider to be modified"),
        )
        .await;
    }
    is_replacement_successful
}

/// Check if an offline provider is back online by sending eth_blockNumber
pub async fn check_url_provider_health(provider: &mut UrlProvider) -> bool {
    // First try to activate the provider if not already activated
    if provider.pubsub.is_empty() {
        if let Err(_) = activate_url_provider(provider).await {
            return false;
        }
    }

    // Try to get the latest block number as a health check
    if let Some(pubsub) = provider.pubsub.first() {
        match tokio::time::timeout(Duration::from_secs(10), pubsub.get_block_number()).await {
            Ok(Ok(_)) => true,
            _ => {
                // Provider failed, clear the connection
                provider.pubsub.clear();
                false
            }
        }
    } else {
        false
    }
}

/// Spawn a health check task for an offline URL provider
pub fn spawn_health_check_for_url_provider(
    providers: Providers,
    chain_id: u64,
    url: String,
    print_tx: PrintSender,
) {
    tokio::spawn(async move {
        let mut backoff_mins = 1u64;

        // Double the backoff, max 60 minutes
        backoff_mins = (backoff_mins * 2).min(60);

        loop {
            // Wait for the backoff period
            tokio::time::sleep(Duration::from_secs(backoff_mins * 60)).await;

            // Try to check health
            let mut provider_online = false;

            if let Some(mut aps) = providers.get_mut(&chain_id) {
                if let Some(provider) = aps.urls.iter_mut().find(|p| p.url == url) {
                    provider.last_health_check = Some(Instant::now());
                    if check_url_provider_health(provider).await {
                        provider.online = true;
                        provider_online = true;
                        provider.last_health_check = Some(Instant::now());

                        verbose_print(&print_tx, &format!("eth: provider {} is back online", url))
                            .await;
                    }
                }
            }

            if provider_online {
                // Provider is back online, exit the health check loop
                break;
            }
        }
    });
}

/// Spawn a method-specific retry for URL provider
pub fn spawn_method_retry_for_url_provider(
    providers: Providers,
    chain_id: u64,
    url: String,
    method: String,
    params: serde_json::Value,
    print_tx: PrintSender,
) {
    tokio::spawn(async move {
        let mut backoff_mins = 1u64;

        // For eth_sendRawTransaction, just wait 60 minutes then clear
        if method == "eth_sendRawTransaction" {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            if let Some(mut aps) = providers.get_mut(&chain_id) {
                if let Some(provider) = aps.urls.iter_mut().find(|p| p.url == url) {
                    provider.method_failures.send_raw_tx_failed = None;
                    verbose_print(
                        &print_tx,
                        &format!("eth: cleared eth_sendRawTransaction failure for {}", url),
                    )
                    .await;
                }
            }
            return;
        }

        // For other methods, retry with exponential backoff
        loop {
            tokio::time::sleep(Duration::from_secs(backoff_mins * 60)).await;

            // Double the backoff, max 60 minutes
            backoff_mins = (backoff_mins * 2).min(60);

            // Try to activate and test the method
            let Some(mut aps) = providers.get_mut(&chain_id) else {
                continue;
            };

            let Some(provider) = aps.urls.iter_mut().find(|p| p.url == url) else {
                continue;
            };

            if provider.pubsub.is_empty() {
                let Ok(_) = activate_url_provider(provider).await else {
                    continue;
                };
            }

            let Some(pubsub) = provider.pubsub.first() else {
                continue;
            };

            // Try the previously-failing method
            let success = matches!(
                tokio::time::timeout(
                    Duration::from_secs(10),
                    pubsub.raw_request::<_, serde_json::Value>(
                        std::borrow::Cow::Owned(method.clone()),
                        &params
                    )
                )
                .await,
                Ok(Ok(_))
            );

            if success {
                // Clear the method failure
                provider.method_failures.clear_method_failure(&method);
                verbose_print(
                    &print_tx,
                    &format!("eth: {} now working again for {}", method, url),
                )
                .await;
                break;
            }
        }
    });
}

/// Spawn a method-specific retry for node provider
pub fn spawn_method_retry_for_node_provider(
    our: String,
    providers: Providers,
    chain_id: u64,
    node_name: String,
    method: String,
    params: serde_json::Value,
    send_to_loop: MessageSender,
    response_channels: ResponseChannels,
    print_tx: PrintSender,
) {
    tokio::spawn(async move {
        let mut backoff_mins = 1u64;

        // For eth_sendRawTransaction, just wait 60 minutes then clear
        if method == "eth_sendRawTransaction" {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            if let Some(mut aps) = providers.get_mut(&chain_id) {
                if let Some(provider) = aps
                    .nodes
                    .iter_mut()
                    .find(|p| p.hns_update.name == node_name)
                {
                    provider.method_failures.send_raw_tx_failed = None;
                    verbose_print(
                        &print_tx,
                        &format!(
                            "eth: cleared eth_sendRawTransaction failure for node {}",
                            node_name
                        ),
                    )
                    .await;
                }
            }
            return;
        }

        // For other methods, retry with exponential backoff
        loop {
            tokio::time::sleep(Duration::from_secs(backoff_mins * 60)).await;

            // Double the backoff, max 60 minutes
            backoff_mins = (backoff_mins * 2).min(60);

            // Try the method via the node
            let km_id = rand::random();
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);

            // Register our response channel
            response_channels.insert(km_id, sender);

            // Send the actual request
            kernel_message(
                &our,
                km_id,
                Address {
                    node: node_name.clone(),
                    process: ETH_PROCESS_ID.clone(),
                },
                None,
                true,
                Some(10),
                EthAction::Request {
                    chain_id: chain_id,
                    method: method.clone(),
                    params: params.clone(),
                },
                &send_to_loop,
            )
            .await;

            // Wait for response
            let success = match tokio::time::timeout(Duration::from_secs(10), receiver.recv()).await
            {
                Ok(Some(Ok(km))) => matches!(km.message, Message::Response(_)),
                _ => false,
            };

            // Clean up response channel
            response_channels.remove(&km_id);

            if success {
                // Clear the method failure
                if let Some(mut aps) = providers.get_mut(&chain_id) {
                    if let Some(provider) = aps
                        .nodes
                        .iter_mut()
                        .find(|p| p.hns_update.name == node_name)
                    {
                        provider.method_failures.clear_method_failure(&method);
                        verbose_print(
                            &print_tx,
                            &format!("eth: {} now working again for node {}", method, node_name),
                        )
                        .await;
                    }
                }
                break;
            }
        }
    });
}

/// Spawn a health check task for an offline node provider
pub fn spawn_health_check_for_node_provider(
    our: String,
    providers: Providers,
    chain_id: u64,
    node_name: String,
    send_to_loop: MessageSender,
    response_channels: ResponseChannels,
    print_tx: PrintSender,
) {
    tokio::spawn(async move {
        let mut backoff_mins = 1u64;

        loop {
            // Wait for the backoff period
            tokio::time::sleep(Duration::from_secs(backoff_mins * 60)).await;

            // Double the backoff, max 60 minutes
            backoff_mins = (backoff_mins * 2).min(60);

            // Try to send eth_blockNumber to check health
            let km_id = rand::random();
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);

            // Register our response channel
            response_channels.insert(km_id, sender);

            // Send eth_blockNumber request
            kernel_message(
                &our,
                km_id,
                Address {
                    node: node_name.clone(),
                    process: ETH_PROCESS_ID.clone(),
                },
                None,
                true,
                Some(10),
                EthAction::Request {
                    chain_id: chain_id,
                    method: "eth_blockNumber".to_string(),
                    params: serde_json::json!([]),
                },
                &send_to_loop,
            )
            .await;

            // Wait for response with timeout
            let provider_online =
                match tokio::time::timeout(Duration::from_secs(10), receiver.recv()).await {
                    Ok(Some(Ok(km))) => {
                        // Check if we got a successful response
                        matches!(km.message, Message::Response(_))
                    }
                    _ => false,
                };

            // Clean up response channel
            response_channels.remove(&km_id);

            if provider_online {
                // Mark the provider as online
                if let Some(mut aps) = providers.get_mut(&chain_id) {
                    if let Some(provider) = aps
                        .nodes
                        .iter_mut()
                        .find(|p| p.hns_update.name == node_name)
                    {
                        provider.online = true;
                        provider.usable = true;
                        provider.last_health_check = Some(Instant::now());

                        verbose_print(
                            &print_tx,
                            &format!("eth: node provider {} is back online", node_name),
                        )
                        .await;
                    }
                }
                // Provider is back online, exit the health check loop
                break;
            } else {
                // Provider is still offline, update last health check time
                if let Some(mut aps) = providers.get_mut(&chain_id) {
                    if let Some(provider) = aps
                        .nodes
                        .iter_mut()
                        .find(|p| p.hns_update.name == node_name)
                    {
                        provider.last_health_check = Some(Instant::now());
                    }
                }

                verbose_print(
                    &print_tx,
                    &format!(
                        "eth: health check failed for node provider {} (backoff: {} min)",
                        node_name, backoff_mins,
                    ),
                )
                .await;
            }
        }
    });
}
