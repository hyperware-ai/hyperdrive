use alloy_providers::provider::Provider;
use alloy_pubsub::PubSubFrontend;
use alloy_rpc_client::ClientBuilder;
use alloy_transport_ws::WsConnect;
use anyhow::Result;
use dashmap::DashMap;
use lib::types::core::*;
use lib::types::eth::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinHandle;
use url::Url;

mod subscription;

/// meta-type for all incoming requests we need to handle
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum IncomingReq {
    EthAction(EthAction),
    EthConfigAction(EthConfigAction),
    EthSubResult(EthSubResult),
    SubKeepalive(u64),
}

/// mapping of chain id to ordered lists of providers
type Providers = Arc<DashMap<u64, ActiveProviders>>;

#[derive(Debug)]
struct ActiveProviders {
    pub urls: Vec<UrlProvider>,
    pub nodes: Vec<NodeProvider>,
}

#[derive(Debug)]
struct UrlProvider {
    pub trusted: bool,
    pub url: String,
    pub pubsub: Option<Provider<PubSubFrontend>>,
}

#[derive(Debug)]
struct NodeProvider {
    pub trusted: bool,
    /// semi-temporary flag to mark if this provider is currently usable
    /// future updates will make this more dynamic
    pub usable: bool,
    pub name: String,
}

impl ActiveProviders {
    fn add_provider_config(&mut self, new: ProviderConfig) {
        match new.provider {
            NodeOrRpcUrl::Node {
                kns_update,
                use_as_provider,
            } => {
                self.nodes.push(NodeProvider {
                    trusted: new.trusted,
                    usable: use_as_provider,
                    name: kns_update.name,
                });
            }
            NodeOrRpcUrl::RpcUrl(url) => {
                self.urls.push(UrlProvider {
                    trusted: new.trusted,
                    url,
                    pubsub: None,
                });
            }
        }
    }

    fn remove_provider(&mut self, remove: &str) {
        self.urls.retain(|x| x.url != remove);
        self.nodes.retain(|x| x.name != remove);
    }
}

/// existing subscriptions held by local OR remote processes
type ActiveSubscriptions = Arc<DashMap<Address, HashMap<u64, ActiveSub>>>;

type ResponseChannels = Arc<DashMap<u64, ProcessMessageSender>>;

#[derive(Debug)]
enum ActiveSub {
    Local(JoinHandle<()>),
    Remote {
        provider_node: String,
        handle: JoinHandle<()>,
        sender: tokio::sync::mpsc::Sender<EthSubResult>,
    },
}

impl ActiveSub {
    async fn close(&self, sub_id: u64, state: &ModuleState) {
        match self {
            ActiveSub::Local(handle) => {
                handle.abort();
            }
            ActiveSub::Remote {
                provider_node,
                handle,
                ..
            } => {
                // tell provider node we don't need their services anymore
                kernel_message(
                    &state.our,
                    rand::random(),
                    Address {
                        node: provider_node.clone(),
                        process: ETH_PROCESS_ID.clone(),
                    },
                    None,
                    true,
                    None,
                    EthAction::UnsubscribeLogs(sub_id),
                    &state.send_to_loop,
                )
                .await;
                handle.abort();
            }
        }
    }
}

struct ModuleState {
    /// the name of this node
    our: Arc<String>,
    /// the home directory path
    home_directory_path: String,
    /// the access settings for this provider
    access_settings: AccessSettings,
    /// the set of providers we have available for all chains
    providers: Providers,
    /// the set of active subscriptions we are currently maintaining
    active_subscriptions: ActiveSubscriptions,
    /// the set of response channels we have open for outstanding request tasks
    response_channels: ResponseChannels,
    /// our sender for kernel event loop
    send_to_loop: MessageSender,
    /// our sender for terminal prints
    print_tx: PrintSender,
}

/// The ETH provider runtime process is responsible for connecting to one or more ETH RPC providers
/// and using them to service indexing requests from other apps. This is the runtime entry point
/// for the entire module.
pub async fn provider(
    our: String,
    home_directory_path: String,
    configs: SavedConfigs,
    send_to_loop: MessageSender,
    mut recv_in_client: MessageReceiver,
    mut net_error_recv: NetworkErrorReceiver,
    caps_oracle: CapMessageSender,
    print_tx: PrintSender,
) -> Result<()> {
    // load access settings if they've been saved
    let access_settings: AccessSettings =
        match tokio::fs::read_to_string(format!("{}/.eth_access_settings", home_directory_path))
            .await
        {
            Ok(contents) => serde_json::from_str(&contents).unwrap(),
            Err(_) => {
                let access_settings = AccessSettings {
                    public: false,
                    allow: HashSet::new(),
                    deny: HashSet::new(),
                };
                let _ = tokio::fs::write(
                    format!("{}/.eth_access_settings", home_directory_path),
                    serde_json::to_string(&access_settings).unwrap(),
                )
                .await;
                access_settings
            }
        };
    verbose_print(
        &print_tx,
        &format!("eth: access settings loaded: {access_settings:?}"),
    )
    .await;

    let mut state = ModuleState {
        our: Arc::new(our),
        home_directory_path,
        access_settings,
        providers: Arc::new(DashMap::new()),
        active_subscriptions: Arc::new(DashMap::new()),
        response_channels: Arc::new(DashMap::new()),
        send_to_loop,
        print_tx,
    };

    // convert saved configs into data structure that we will use to route queries
    for entry in configs {
        let mut ap = state
            .providers
            .entry(entry.chain_id)
            .or_insert(ActiveProviders {
                urls: vec![],
                nodes: vec![],
            });
        ap.add_provider_config(entry);
    }

    verbose_print(&state.print_tx, "eth: provider initialized").await;

    loop {
        tokio::select! {
            Some(wrapped_error) = net_error_recv.recv() => {
                handle_network_error(
                    wrapped_error,
                    &state.active_subscriptions,
                    &state.response_channels,
                    &state.print_tx
                ).await;
            }
            Some(km) = recv_in_client.recv() => {
                let km_id = km.id;
                let response_target = km.rsvp.as_ref().unwrap_or(&km.source).clone();
                if let Err(e) = handle_message(
                    &mut state,
                    km,
                    &caps_oracle,
                )
                .await
                {
                    error_message(
                        &state.our,
                        km_id,
                        response_target,
                        e,
                        &state.send_to_loop
                    ).await;
                };
            }
        }
    }
}

async fn handle_network_error(
    wrapped_error: WrappedSendError,
    active_subscriptions: &ActiveSubscriptions,
    response_channels: &ResponseChannels,
    print_tx: &PrintSender,
) {
    verbose_print(&print_tx, "eth: got network error").await;
    // if we hold active subscriptions for the remote node that this error refers to,
    // close them here -- they will need to resubscribe
    // TODO is this necessary?
    if let Some((_who, sub_map)) = active_subscriptions.remove(&wrapped_error.error.target) {
        for (_sub_id, sub) in sub_map.iter() {
            if let ActiveSub::Local(handle) = sub {
                verbose_print(
                    &print_tx,
                    "eth: closing local sub in response to network error",
                )
                .await;
                handle.abort();
            }
        }
    }
    // we got an error from a remote node provider --
    // forward it to response channel if it exists
    if let Some(chan) = response_channels.get(&wrapped_error.id) {
        // can't close channel here, as response may be an error
        // and fulfill_request may wish to try other providers.
        verbose_print(&print_tx, "eth: sent network error to response channel").await;
        let _ = chan.send(Err(wrapped_error)).await;
    }
}

/// handle incoming requests, namely [`EthAction`] and [`EthConfigAction`].
/// also handle responses that are passthroughs from remote provider nodes.
async fn handle_message(
    state: &mut ModuleState,
    km: KernelMessage,
    caps_oracle: &CapMessageSender,
) -> Result<(), EthError> {
    match &km.message {
        Message::Response(_) => {
            // map response to the correct channel
            if let Some(chan) = state.response_channels.get(&km.id) {
                // can't close channel here, as response may be an error
                // and fulfill_request may wish to try other providers.
                let _ = chan.send(Ok(km)).await;
            } else {
                verbose_print(
                    &state.print_tx,
                    "eth: got response but no matching channel found",
                )
                .await;
            }
        }
        Message::Request(req) => {
            let timeout = req.expects_response.unwrap_or(60);
            let Ok(req) = serde_json::from_slice::<IncomingReq>(&req.body) else {
                return Err(EthError::MalformedRequest);
            };
            match req {
                IncomingReq::EthAction(eth_action) => {
                    return handle_eth_action(state, km, timeout, eth_action).await;
                }
                IncomingReq::EthConfigAction(eth_config_action) => {
                    kernel_message(
                        &state.our.clone(),
                        km.id,
                        km.rsvp.as_ref().unwrap_or(&km.source).clone(),
                        None,
                        false,
                        None,
                        handle_eth_config_action(state, caps_oracle, &km, eth_config_action).await,
                        &state.send_to_loop,
                    )
                    .await;
                }
                IncomingReq::EthSubResult(eth_sub_result) => {
                    // forward this to rsvp, if we have the sub id in our active subs
                    let Some(rsvp) = km.rsvp else {
                        return Ok(()); // no rsvp, no need to forward
                    };
                    let sub_id = match eth_sub_result {
                        Ok(EthSub { id, .. }) => id,
                        Err(EthSubError { id, .. }) => id,
                    };
                    if let Some(sub_map) = state.active_subscriptions.get(&rsvp) {
                        if let Some(ActiveSub::Remote {
                            provider_node,
                            sender,
                            ..
                        }) = sub_map.get(&sub_id)
                        {
                            if provider_node == &km.source.node {
                                if let Ok(()) = sender.send(eth_sub_result).await {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    // tell the remote provider that we don't have this sub
                    // so they can stop sending us updates
                    verbose_print(
                        &state.print_tx,
                        "eth: got eth_sub_result but no matching sub found",
                    )
                    .await;
                    kernel_message(
                        &state.our.clone(),
                        km.id,
                        km.source.clone(),
                        None,
                        true,
                        None,
                        EthAction::UnsubscribeLogs(sub_id),
                        &state.send_to_loop,
                    )
                    .await;
                }
                IncomingReq::SubKeepalive(sub_id) => {
                    // source expects that we have a local sub for them with this id
                    // if we do, no action required, otherwise, throw them an error.
                    if let Some(sub_map) = state.active_subscriptions.get(&km.source) {
                        if sub_map.contains_key(&sub_id) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_eth_action(
    state: &mut ModuleState,
    km: KernelMessage,
    timeout: u64,
    eth_action: EthAction,
) -> Result<(), EthError> {
    // check our access settings if the request is from a remote node
    if km.source.node != *state.our {
        if state.access_settings.deny.contains(&km.source.node) {
            verbose_print(
                &state.print_tx,
                "eth: got eth_action from unauthorized remote source",
            )
            .await;
            return Err(EthError::PermissionDenied);
        }
        if !state.access_settings.public {
            if !state.access_settings.allow.contains(&km.source.node) {
                verbose_print(
                    &state.print_tx,
                    "eth: got eth_action from unauthorized remote source",
                )
                .await;
                return Err(EthError::PermissionDenied);
            }
        }
    }

    verbose_print(
        &state.print_tx,
        &format!("eth: handling eth_action {eth_action:?}"),
    )
    .await;

    // for each incoming action, we need to assign a provider from our map
    // based on the chain id. once we assign a provider, we can use it for
    // this request. if the provider is not usable, cycle through options
    // before returning an error.
    match eth_action {
        EthAction::SubscribeLogs { sub_id, .. } => {
            tokio::spawn(subscription::create_new_subscription(
                state.our.to_string(),
                km.id,
                km.source.clone(),
                km.rsvp,
                state.send_to_loop.clone(),
                sub_id,
                eth_action,
                state.providers.clone(),
                state.active_subscriptions.clone(),
                state.response_channels.clone(),
                state.print_tx.clone(),
            ));
        }
        EthAction::UnsubscribeLogs(sub_id) => {
            let mut sub_map = state
                .active_subscriptions
                .entry(km.source)
                .or_insert(HashMap::new());
            if let Some(sub) = sub_map.remove(&sub_id) {
                sub.close(sub_id, state).await;
            }
        }
        EthAction::Request { .. } => {
            let (sender, receiver) = tokio::sync::mpsc::channel(1);
            state.response_channels.insert(km.id, sender);
            let our = state.our.to_string();
            let send_to_loop = state.send_to_loop.clone();
            let providers = state.providers.clone();
            let response_channels = state.response_channels.clone();
            let print_tx = state.print_tx.clone();
            tokio::spawn(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout),
                    fulfill_request(
                        &our,
                        km.id,
                        &send_to_loop,
                        eth_action,
                        providers,
                        receiver,
                        &print_tx,
                    ),
                )
                .await
                {
                    Ok(response) => {
                        kernel_message(
                            &our,
                            km.id,
                            km.rsvp.unwrap_or(km.source),
                            None,
                            false,
                            None,
                            response,
                            &send_to_loop,
                        )
                        .await;
                    }
                    Err(_) => {
                        // task timeout
                        error_message(&our, km.id, km.source, EthError::RpcTimeout, &send_to_loop)
                            .await;
                    }
                }
                response_channels.remove(&km.id);
            });
        }
    }
    Ok(())
}

async fn fulfill_request(
    our: &str,
    km_id: u64,
    send_to_loop: &MessageSender,
    eth_action: EthAction,
    providers: Providers,
    mut remote_request_receiver: ProcessMessageReceiver,
    print_tx: &PrintSender,
) -> EthResponse {
    let EthAction::Request {
        chain_id,
        ref method,
        ref params,
    } = eth_action
    else {
        return EthResponse::Err(EthError::PermissionDenied); // will never hit
    };
    let Some(method) = to_static_str(&method) else {
        return EthResponse::Err(EthError::InvalidMethod(method.to_string()));
    };
    let Some(mut aps) = providers.get_mut(&chain_id) else {
        return EthResponse::Err(EthError::NoRpcForChain);
    };
    // first, try any url providers we have for this chain,
    // then if we have none or they all fail, go to node provider.
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
        let Ok(value) = pubsub.inner().prepare(method, params.clone()).await else {
            // this provider failed and needs to be reset
            url_provider.pubsub = None;
            continue;
        };
        return EthResponse::Response { value };
    }
    for node_provider in &mut aps.nodes {
        let response = forward_to_node_provider(
            our,
            km_id,
            None,
            node_provider,
            eth_action.clone(),
            send_to_loop,
            &mut remote_request_receiver,
        )
        .await;
        if let EthResponse::Err(e) = response {
            if e == EthError::RpcMalformedResponse {
                node_provider.usable = false;
            }
        } else {
            return response;
        }
    }
    EthResponse::Err(EthError::NoRpcForChain)
}

/// take an EthAction and send it to a node provider, then await a response.
async fn forward_to_node_provider(
    our: &str,
    km_id: u64,
    rsvp: Option<Address>,
    node_provider: &NodeProvider,
    eth_action: EthAction,
    send_to_loop: &MessageSender,
    receiver: &mut ProcessMessageReceiver,
) -> EthResponse {
    if !node_provider.usable || node_provider.name == our {
        return EthResponse::Err(EthError::PermissionDenied);
    }
    kernel_message(
        our,
        km_id,
        Address {
            node: node_provider.name.clone(),
            process: ETH_PROCESS_ID.clone(),
        },
        rsvp,
        true,
        Some(60), // TODO
        eth_action.clone(),
        &send_to_loop,
    )
    .await;
    let Ok(Some(Ok(response_km))) =
        tokio::time::timeout(std::time::Duration::from_secs(30), receiver.recv()).await
    else {
        return EthResponse::Err(EthError::RpcTimeout);
    };
    let Message::Response((resp, _context)) = response_km.message else {
        // if we hit this, they spoofed a request with same id, ignore and possibly punish
        return EthResponse::Err(EthError::RpcMalformedResponse);
    };
    let Ok(eth_response) = serde_json::from_slice::<EthResponse>(&resp.body) else {
        // if we hit this, they sent a malformed response, ignore and possibly punish
        return EthResponse::Err(EthError::RpcMalformedResponse);
    };
    eth_response
}

async fn handle_eth_config_action(
    state: &mut ModuleState,
    caps_oracle: &CapMessageSender,
    km: &KernelMessage,
    eth_config_action: EthConfigAction,
) -> EthConfigResponse {
    if km.source.node != *state.our {
        verbose_print(
            &state.print_tx,
            "eth: got eth_config_action from unauthorized remote source",
        )
        .await;
        return EthConfigResponse::PermissionDenied;
    }

    // check capabilities to ensure the sender is allowed to make this request
    if !check_for_root_cap(&state.our, &km.source.process, caps_oracle).await {
        verbose_print(
            &state.print_tx,
            "eth: got eth_config_action from unauthorized local source",
        )
        .await;
        return EthConfigResponse::PermissionDenied;
    }

    verbose_print(
        &state.print_tx,
        &format!("eth: handling eth_config_action {eth_config_action:?}"),
    )
    .await;

    let mut save_providers = false;

    // modify our providers and access settings based on config action
    match eth_config_action {
        EthConfigAction::AddProvider(provider) => {
            let mut aps = state
                .providers
                .entry(provider.chain_id)
                .or_insert(ActiveProviders {
                    urls: vec![],
                    nodes: vec![],
                });
            aps.add_provider_config(provider);
            save_providers = true;
        }
        EthConfigAction::RemoveProvider((chain_id, remove)) => {
            if let Some(mut aps) = state.providers.get_mut(&chain_id) {
                aps.remove_provider(&remove);
                save_providers = true;
            }
        }
        EthConfigAction::SetPublic => {
            state.access_settings.public = true;
        }
        EthConfigAction::SetPrivate => {
            state.access_settings.public = false;
        }
        EthConfigAction::AllowNode(node) => {
            state.access_settings.allow.insert(node);
        }
        EthConfigAction::UnallowNode(node) => {
            state.access_settings.allow.remove(&node);
        }
        EthConfigAction::DenyNode(node) => {
            state.access_settings.deny.insert(node);
        }
        EthConfigAction::UndenyNode(node) => {
            state.access_settings.deny.remove(&node);
        }
        EthConfigAction::SetProviders(new_providers) => {
            let new_map = DashMap::new();
            for entry in new_providers {
                let mut aps = new_map.entry(entry.chain_id).or_insert(ActiveProviders {
                    urls: vec![],
                    nodes: vec![],
                });
                aps.add_provider_config(entry);
            }
            state.providers = Arc::new(new_map);
            save_providers = true;
        }
        EthConfigAction::GetProviders => {
            return EthConfigResponse::Providers(providers_to_saved_configs(&state.providers));
        }
        EthConfigAction::GetAccessSettings => {
            return EthConfigResponse::AccessSettings(state.access_settings.clone());
        }
        EthConfigAction::GetState => {
            return EthConfigResponse::State {
                active_subscriptions: state
                    .active_subscriptions
                    .iter()
                    .map(|e| {
                        (
                            e.key().clone(),
                            e.value()
                                .iter()
                                .map(|(id, sub)| {
                                    (
                                        *id,
                                        match sub {
                                            ActiveSub::Local(_) => None,
                                            ActiveSub::Remote { provider_node, .. } => {
                                                Some(provider_node.clone())
                                            }
                                        },
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
                outstanding_requests: state.response_channels.iter().map(|e| *e.key()).collect(),
            };
        }
    }
    // save providers and access settings to disk
    let _ = tokio::fs::write(
        format!("{}/.eth_access_settings", state.home_directory_path),
        serde_json::to_string(&state.access_settings).unwrap(),
    )
    .await;
    verbose_print(&state.print_tx, "eth: saved new access settings").await;
    if save_providers {
        let _ = tokio::fs::write(
            format!("{}/.eth_providers", state.home_directory_path),
            serde_json::to_string(&providers_to_saved_configs(&state.providers)).unwrap(),
        )
        .await;
        verbose_print(&state.print_tx, "eth: saved new provider settings").await;
    }
    EthConfigResponse::Ok
}

async fn activate_url_provider(provider: &mut UrlProvider) -> Result<()> {
    match Url::parse(&provider.url)?.scheme() {
        "ws" | "wss" => {
            let connector = WsConnect {
                url: provider.url.to_string(),
                auth: None,
            };
            let client = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                ClientBuilder::default().ws(connector),
            )
            .await??;
            provider.pubsub = Some(Provider::new_with_client(client));
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Only `ws://` or `wss://` providers are supported."
        )),
    }
}

fn providers_to_saved_configs(providers: &Providers) -> SavedConfigs {
    providers
        .iter()
        .map(|entry| {
            entry
                .urls
                .iter()
                .map(|url_provider| ProviderConfig {
                    chain_id: *entry.key(),
                    provider: NodeOrRpcUrl::RpcUrl(url_provider.url.clone()),
                    trusted: url_provider.trusted,
                })
                .chain(entry.nodes.iter().map(|node_provider| ProviderConfig {
                    chain_id: *entry.key(),
                    provider: NodeOrRpcUrl::Node {
                        kns_update: KnsUpdate {
                            name: node_provider.name.clone(),
                            owner: "".to_string(),
                            node: "".to_string(),
                            public_key: "".to_string(),
                            ip: "".to_string(),
                            port: 0,
                            routers: vec![],
                        },
                        use_as_provider: node_provider.usable,
                    },
                    trusted: node_provider.trusted,
                }))
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect()
}

async fn check_for_root_cap(
    our: &str,
    process: &ProcessId,
    caps_oracle: &CapMessageSender,
) -> bool {
    let (send_cap_bool, recv_cap_bool) = tokio::sync::oneshot::channel();
    caps_oracle
        .send(CapMessage::Has {
            on: process.clone(),
            cap: Capability {
                issuer: Address {
                    node: our.to_string(),
                    process: ETH_PROCESS_ID.clone(),
                },
                params: serde_json::to_string(&serde_json::json!({
                    "root": true,
                }))
                .unwrap(),
            },
            responder: send_cap_bool,
        })
        .await
        .expect("eth: capability oracle died!");
    recv_cap_bool.await.unwrap_or(false)
}

async fn verbose_print(print_tx: &PrintSender, content: &str) {
    let _ = print_tx
        .send(Printout {
            verbosity: 2,
            content: content.to_string(),
        })
        .await;
}

async fn error_message(
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

async fn kernel_message<T: Serialize>(
    our: &str,
    km_id: u64,
    target: Address,
    rsvp: Option<Address>,
    req: bool,
    timeout: Option<u64>,
    body: T,
    send_to_loop: &MessageSender,
) {
    let _ = send_to_loop
        .send(KernelMessage {
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
        })
        .await;
}
