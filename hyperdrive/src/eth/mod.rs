use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::json_rpc::RpcError;
use anyhow::Result;
use dashmap::DashMap;
use indexmap::IndexMap;
use lib::types::core::*;
use lib::types::eth::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use utils::*;

mod subscription;
mod utils;

/// meta-type for all incoming requests we need to handle
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum IncomingReq {
    /// requests for an RPC action that can come from processes on this node or others
    EthAction(EthAction),
    /// requests that must come from this node to modify provider settings / fetch them
    EthConfigAction(EthConfigAction),
    /// subscription updates coming in from a remote provider
    EthSubResult(EthSubResult),
    /// a remote node who uses our provider keeping their subscription alive
    SubKeepalive(u64),
}

/// mapping of chain id to ordered lists of providers
type Providers = Arc<DashMap<u64, ActiveProviders>>;

#[derive(Debug)]
struct ActiveProviders {
    pub urls: Vec<UrlProvider>,
    pub nodes: Vec<NodeProvider>,
}

#[derive(Debug, Clone, Default)]
struct MethodFailures {
    /// Category 1: Simple methods that failed (retry until success)
    pub failed_methods: HashSet<String>,
    /// Category 2: eth_sendRawTransaction failures (clear after 60m)
    pub send_raw_tx_failed: Option<Instant>,
    /// Category 3: eth_getLogs max failed block range
    pub failed_logs_range: Option<u64>,
}

#[derive(Debug, Clone)]
struct UrlProvider {
    pub trusted: bool,
    pub url: String,
    /// a list, in case we build multiple providers for the same url
    pub pubsub: Vec<RootProvider<PubSubFrontend>>,
    pub auth: Option<Authorization>,
    /// whether this provider was online as of last check
    pub online: bool,
    /// last time we checked if offline provider is back online
    pub last_health_check: Option<Instant>,
    /// method-specific failures
    pub method_failures: MethodFailures,
}

#[derive(Debug, Clone)]
struct NodeProvider {
    /// NOT CURRENTLY USED
    pub trusted: bool,
    /// semi-temporary flag to mark if this provider is currently usable
    /// future updates will make this more dynamic
    pub usable: bool,
    /// the HNS update that describes this node provider
    /// kept so we can re-serialize to SavedConfigs
    pub hns_update: HnsUpdate,
    /// whether this provider was online as of last check
    pub online: bool,
    /// last time we checked if offline provider is back online
    pub last_health_check: Option<Instant>,
    /// method-specific failures
    pub method_failures: MethodFailures,
}

impl MethodFailures {
    /// Check if a method should be skipped for this provider
    fn should_skip_method(&self, method: &str, params: &serde_json::Value) -> bool {
        // Check Category 2: eth_sendRawTransaction with 60m timeout
        if method == "eth_sendRawTransaction" {
            if let Some(failed_at) = self.send_raw_tx_failed {
                if failed_at.elapsed() < Duration::from_secs(3600) {
                    return true;
                }
            }
        }

        // Check Category 3: eth_getLogs with block range
        if method == "eth_getLogs" {
            if let Some(block_range) = extract_block_range(params) {
                if let Some(failed_range) = self.failed_logs_range {
                    // Skip if current range is >= failed range
                    if block_range >= failed_range {
                        return true;
                    }
                }
            }
        }

        // Check Category 1: Simple methods
        self.failed_methods.contains(method)
    }

    /// Mark a method as failed
    fn mark_method_failed(&mut self, method: &str, params: &serde_json::Value) {
        match method {
            "eth_sendRawTransaction" => {
                // Category 2: Mark with timestamp
                self.send_raw_tx_failed = Some(Instant::now());
            }
            "eth_getLogs" => {
                // Category 3: Update minimum failed block range
                if let Some(block_range) = extract_block_range(params) {
                    // Update to the minimum of current and new failed range
                    self.failed_logs_range = Some(match self.failed_logs_range {
                        Some(existing) => existing.min(block_range),
                        None => block_range,
                    });
                }
            }
            _ => {
                // Category 1: Simple methods
                self.failed_methods.insert(method.to_string());
            }
        }
    }

    /// Clear a method failure (when it succeeds)
    fn clear_method_failure(&mut self, method: &str) {
        match method {
            "eth_sendRawTransaction" => {
                self.send_raw_tx_failed = None;
            }
            "eth_getLogs" => {
                // Clear the failed range when getLogs succeeds
                self.failed_logs_range = None;
            }
            _ => {
                self.failed_methods.remove(method);
            }
        }
    }
}

/// Extract block range from eth_getLogs params
fn extract_block_range(params: &serde_json::Value) -> Option<u64> {
    if let Some(arr) = params.as_array() {
        if let Some(obj) = arr.first().and_then(|v| v.as_object()) {
            let from_block = parse_block_number(obj.get("fromBlock")?)?;
            let to_block = parse_block_number(obj.get("toBlock")?)?;
            return Some(to_block.saturating_sub(from_block));
        }
    }
    None
}

/// Parse block number from various formats (hex string, number, "latest", etc.)
fn parse_block_number(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with("0x") {
                u64::from_str_radix(&s[2..], 16).ok()
            } else {
                match s.as_str() {
                    "latest" | "pending" => Some(u64::MAX),
                    "earliest" => Some(0),
                    _ => s.parse().ok(),
                }
            }
        }
        serde_json::Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

impl ActiveProviders {
    fn add_provider_config(&mut self, new: ProviderConfig) {
        match &new.provider {
            NodeOrRpcUrl::RpcUrl { url, auth } => {
                // Remove any existing URL provider with this URL
                self.urls
                    .retain(|existing_provider| existing_provider.url != *url);

                // Create and add new URL provider
                let url_provider = UrlProvider {
                    trusted: new.trusted,
                    url: url.clone(),
                    pubsub: vec![],
                    auth: auth.clone(),
                    online: true, // Default to online
                    last_health_check: None,
                    method_failures: MethodFailures::default(),
                };
                self.urls.insert(0, url_provider);
            }
            NodeOrRpcUrl::Node { hns_update, .. } => {
                // Remove any existing node provider with this node name
                self.nodes.retain(|existing_provider| {
                    existing_provider.hns_update.name != hns_update.name
                });

                // Create and add new node provider
                let node_provider = NodeProvider {
                    trusted: new.trusted,
                    usable: true, // Default to usable
                    hns_update: hns_update.clone(),
                    online: true, // Default to online
                    last_health_check: None,
                    method_failures: MethodFailures::default(),
                };
                self.nodes.insert(0, node_provider);
            }
        }
    }

    fn remove_provider(&mut self, remove: &str) -> bool {
        let urls_len_before = self.urls.len();
        let nodes_len_before = self.nodes.len();

        self.urls.retain(|x| x.url != remove);
        self.nodes.retain(|x| x.hns_update.name != remove);

        // Return true if anything was actually removed
        self.urls.len() < urls_len_before || self.nodes.len() < nodes_len_before
    }
}

/// existing subscriptions held by local OR remote processes
type ActiveSubscriptions = Arc<DashMap<Address, HashMap<u64, ActiveSub>>>;

type ResponseChannels = Arc<DashMap<u64, ProcessMessageSender>>;

#[derive(Debug)]
enum ActiveSub {
    Local((tokio::sync::mpsc::Sender<bool>, JoinHandle<()>)),
    Remote {
        provider_node: String,
        handle: JoinHandle<()>,
        sender: tokio::sync::mpsc::Sender<EthSubResult>,
    },
}

impl ActiveSub {
    async fn close(&self, sub_id: u64, state: &ModuleState) {
        match self {
            ActiveSub::Local((close_sender, _handle)) => {
                close_sender.send(true).await.unwrap();
                //handle.abort();
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
    home_directory_path: PathBuf,
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
    /// cache of ETH requests
    request_cache: RequestCache,
}

type RequestCache = Arc<Mutex<IndexMap<Vec<u8>, (EthResponse, Instant)>>>;

const DELAY_MS: u64 = 1_000;
const MAX_REQUEST_CACHE_LEN: usize = 500;

/// TODO replace with alloy abstraction
fn valid_method(method: &str) -> Option<&'static str> {
    match method {
        "eth_getBalance" => Some("eth_getBalance"),
        "eth_sendRawTransaction" => Some("eth_sendRawTransaction"),
        "eth_call" => Some("eth_call"),
        "eth_chainId" => Some("eth_chainId"),
        "eth_getTransactionReceipt" => Some("eth_getTransactionReceipt"),
        "eth_getTransactionCount" => Some("eth_getTransactionCount"),
        "eth_estimateGas" => Some("eth_estimateGas"),
        "eth_blockNumber" => Some("eth_blockNumber"),
        "eth_getBlockByHash" => Some("eth_getBlockByHash"),
        "eth_getBlockByNumber" => Some("eth_getBlockByNumber"),
        "eth_getTransactionByHash" => Some("eth_getTransactionByHash"),
        "eth_getCode" => Some("eth_getCode"),
        "eth_getStorageAt" => Some("eth_getStorageAt"),
        "eth_gasPrice" => Some("eth_gasPrice"),
        "eth_accounts" => Some("eth_accounts"),
        "eth_hashrate" => Some("eth_hashrate"),
        "eth_getLogs" => Some("eth_getLogs"),
        "eth_subscribe" => Some("eth_subscribe"),
        "eth_unsubscribe" => Some("eth_unsubscribe"),
        // "eth_mining" => Some("eth_mining"),
        // "net_version" => Some("net_version"),
        // "net_peerCount" => Some("net_peerCount"),
        // "net_listening" => Some("net_listening"),
        // "web3_clientVersion" => Some("web3_clientVersion"),
        // "web3_sha3" => Some("web3_sha3"),
        _ => None,
    }
}

/// The ETH provider runtime process is responsible for connecting to one or more ETH RPC providers
/// and using them to service indexing requests from other apps. This is the runtime entry point
/// for the entire module.
pub async fn provider(
    our: String,
    home_directory_path: PathBuf,
    configs: SavedConfigs,
    send_to_loop: MessageSender,
    mut recv_in_client: MessageReceiver,
    mut net_error_recv: NetworkErrorReceiver,
    caps_oracle: CapMessageSender,
    print_tx: PrintSender,
) -> Result<()> {
    // load access settings if they've been persisted to disk
    // this merely describes whether our provider is available to other nodes
    // and if so, which nodes are allowed to access it (public/whitelist/blacklist)
    let access_settings: AccessSettings =
        match tokio::fs::read_to_string(home_directory_path.join(".eth_access_settings")).await {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or(AccessSettings {
                public: false,
                allow: HashSet::new(),
                deny: HashSet::new(),
            }),
            Err(_) => AccessSettings {
                public: false,
                allow: HashSet::new(),
                deny: HashSet::new(),
            },
        };
    verbose_print(
        &print_tx,
        &format!("eth: access settings loaded: {access_settings:?}"),
    )
    .await;

    // initialize module state
    // fill out providers based on saved configs (possibly persisted, given to us)
    // this can be a mix of node providers and rpc providers
    let mut state = ModuleState {
        our: Arc::new(our),
        home_directory_path,
        access_settings,
        providers: Arc::new(DashMap::new()),
        active_subscriptions: Arc::new(DashMap::new()),
        response_channels: Arc::new(DashMap::new()),
        send_to_loop,
        print_tx,
        request_cache: Arc::new(Mutex::new(IndexMap::new())),
    };

    // convert saved configs into data structure that we will use to route queries
    for entry in configs.0.into_iter().rev() {
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

    // main loop: handle incoming network errors and incoming kernel messages
    loop {
        tokio::select! {
            Some(wrapped_error) = net_error_recv.recv() => {
                handle_network_error(
                    wrapped_error,
                    &state,
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

/// network errors only come from remote provider nodes we tried to access,
/// or from remote nodes that are using us as a provider.
///
/// if we tried to access them, we will have a response channel to send the error to.
/// if they are using us as a provider, close the subscription associated with the target.
async fn handle_network_error(wrapped_error: WrappedSendError, state: &ModuleState) {
    verbose_print(
        &state.print_tx,
        &format!(
            "eth: got network error from {}",
            &wrapped_error.error.target
        ),
    )
    .await;

    // close all subscriptions held by the process that we (possibly) tried to send an update to
    if let Some((_who, sub_map)) = state
        .active_subscriptions
        .remove(&wrapped_error.error.target)
    {
        for (sub_id, sub) in sub_map.iter() {
            verbose_print(
                &state.print_tx,
                &format!(
                    "eth: closed subscription {} in response to network error",
                    sub_id
                ),
            )
            .await;
            sub.close(*sub_id, state).await;
        }
    }

    // forward error to response channel if it exists
    if let Some(chan) = state.response_channels.get(&wrapped_error.id) {
        // don't close channel here, as channel holder will wish to try other providers.
        verbose_print(
            &state.print_tx,
            "eth: forwarded network error to response channel",
        )
        .await;
        let _ = chan.send(Err(wrapped_error)).await;
    }
}

/// handle incoming requests and responses.
/// requests must be one of types in [`IncomingReq`].
/// responses are passthroughs from remote provider nodes.
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
            Ok(())
        }
        Message::Request(req) => {
            let timeout = req.expects_response.unwrap_or(60);
            let Ok(req) = serde_json::from_slice::<IncomingReq>(&req.body) else {
                return Err(EthError::MalformedRequest);
            };
            match req {
                IncomingReq::EthAction(eth_action) => {
                    handle_eth_action(state, km, timeout, eth_action).await
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
                    Ok(())
                }
                IncomingReq::EthSubResult(eth_sub_result) => {
                    // forward this to rsvp, if we have the sub id in our active subs
                    let Some(rsvp) = km.rsvp else {
                        verbose_print(
                            &state.print_tx,
                            "eth: got eth_sub_result with no rsvp, ignoring",
                        )
                        .await;
                        return Ok(()); // no rsvp, no need to forward
                    };
                    let sub_id = match eth_sub_result {
                        Ok(EthSub { id, .. }) => id,
                        Err(EthSubError { id, .. }) => id,
                    };
                    if let Some(mut sub_map) = state.active_subscriptions.get_mut(&rsvp) {
                        if let Some(sub) = sub_map.get(&sub_id) {
                            if let ActiveSub::Remote {
                                provider_node,
                                sender,
                                ..
                            } = sub
                            {
                                if provider_node == &km.source.node {
                                    if let Ok(()) = sender.send(eth_sub_result).await {
                                        // successfully sent a subscription update from a
                                        // remote provider to one of our processes
                                        return Ok(());
                                    }
                                }
                                // failed to send subscription update to process,
                                // unsubscribe from provider and close
                                verbose_print(
                                    &state.print_tx,
                                    "eth: got eth_sub_result but provider node did not match or local sub was already closed",
                                )
                                .await;
                                sub.close(sub_id, state).await;
                                sub_map.remove(&sub_id);
                                return Ok(());
                            }
                        }
                    }
                    // tell the remote provider that we don't have this sub
                    // so they can stop sending us updates
                    verbose_print(
                        &state.print_tx,
                        &format!(
                            "eth: got eth_sub_result but no matching sub {} found, unsubscribing",
                            sub_id
                        ),
                    )
                    .await;
                    kernel_message(
                        &state.our,
                        km.id,
                        km.source,
                        None,
                        true,
                        None,
                        EthAction::UnsubscribeLogs(sub_id),
                        &state.send_to_loop,
                    )
                    .await;
                    Ok(())
                }
                IncomingReq::SubKeepalive(sub_id) => {
                    // source expects that we have a local sub for them with this id
                    // if we do, no action required, otherwise, throw them an error.
                    if let Some(sub_map) = state.active_subscriptions.get(&km.source) {
                        if sub_map.contains_key(&sub_id) {
                            return Ok(());
                        } else if sub_map.is_empty() {
                            drop(sub_map);
                            state.active_subscriptions.remove(&km.source);
                        }
                    }
                    verbose_print(
                        &state.print_tx,
                        &format!(
                            "eth: got sub_keepalive from {} but no matching sub found",
                            km.source
                        ),
                    )
                    .await;
                    // send a response with an EthSubError
                    kernel_message(
                        &state.our.clone(),
                        km.id,
                        km.source.clone(),
                        None,
                        false,
                        None,
                        EthSubResult::Err(EthSubError {
                            id: sub_id,
                            error: "Subscription not found".to_string(),
                        }),
                        &state.send_to_loop,
                    )
                    .await;
                    Ok(())
                }
            }
        }
    }
}

async fn handle_eth_action(
    state: &mut ModuleState,
    km: KernelMessage,
    timeout: u64,
    eth_action: EthAction,
) -> Result<(), EthError> {
    // check our access settings if the request is from a remote node
    if km.source.node != *state.our {
        if state.access_settings.deny.contains(&km.source.node)
            || (!state.access_settings.public
                && !state.access_settings.allow.contains(&km.source.node))
        {
            verbose_print(
                &state.print_tx,
                "eth: got eth_action from unauthorized remote source",
            )
            .await;
            return Err(EthError::PermissionDenied);
        }
    }

    verbose_print(
        &state.print_tx,
        &format!(
            "eth: handling {} from {}; active_subs len: {:?}",
            //"eth: handling {} from {}",
            match &eth_action {
                EthAction::SubscribeLogs { .. } => "subscribe",
                EthAction::UnsubscribeLogs(_) => "unsubscribe",
                EthAction::Request { .. } => "request",
            },
            km.source,
            state
                .active_subscriptions
                .iter()
                .map(|v| v.len())
                .collect::<Vec<_>>(),
        ),
    )
    .await;

    // for each incoming action, we need to assign a provider from our map
    // based on the chain id. once we assign a provider, we can use it for
    // this request. if the provider is not usable, cycle through options
    // before returning an error.
    match eth_action {
        EthAction::SubscribeLogs { sub_id, .. } => {
            subscription::create_new_subscription(
                state,
                km.id,
                km.source.clone(),
                km.rsvp,
                sub_id,
                eth_action,
            )
            .await;
        }
        EthAction::UnsubscribeLogs(sub_id) => {
            let Some(mut sub_map) = state.active_subscriptions.get_mut(&km.source) else {
                verbose_print(
                    &state.print_tx,
                    &format!(
                        "eth: got unsubscribe from {} but no subscription found",
                        km.source
                    ),
                )
                .await;
                error_message(
                    &state.our,
                    km.id,
                    km.source,
                    EthError::MalformedRequest,
                    &state.send_to_loop,
                )
                .await;
                return Ok(());
            };
            if let Some(sub) = sub_map.remove(&sub_id) {
                sub.close(sub_id, state).await;
                verbose_print(
                    &state.print_tx,
                    &format!("eth: closed subscription {} for {}", sub_id, km.source.node),
                )
                .await;
                kernel_message(
                    &state.our,
                    km.id,
                    km.rsvp.unwrap_or(km.source.clone()),
                    None,
                    false,
                    None,
                    EthResponse::Ok,
                    &state.send_to_loop,
                )
                .await;
            } else {
                verbose_print(
                    &state.print_tx,
                    &format!(
                        "eth: got unsubscribe from {} but no subscription {} found",
                        km.source, sub_id
                    ),
                )
                .await;
                error_message(
                    &state.our,
                    km.id,
                    km.source.clone(),
                    EthError::MalformedRequest,
                    &state.send_to_loop,
                )
                .await;
            }
            // if sub_map is now empty, remove the source from the active_subscriptions map
            if sub_map.is_empty() {
                drop(sub_map);
                state.active_subscriptions.remove(&km.source);
            }
        }
        EthAction::Request { .. } => {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
            state.response_channels.insert(km.id, sender);
            let our = state.our.to_string();
            let send_to_loop = state.send_to_loop.clone();
            let providers = state.providers.clone();
            let response_channels = state.response_channels.clone();
            let print_tx = state.print_tx.clone();
            let mut request_cache = Arc::clone(&state.request_cache);
            tokio::spawn(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout),
                    fulfill_request(
                        &our,
                        km.id,
                        &send_to_loop,
                        &eth_action,
                        &providers,
                        &mut receiver,
                        &response_channels,
                        &print_tx,
                        &mut request_cache,
                    ),
                )
                .await
                {
                    Ok(response) => {
                        if let EthResponse::Err(EthError::RpcError(_)) = response {
                            // try one more time after 1s delay in case RPC is rate limiting
                            std::thread::sleep(std::time::Duration::from_millis(DELAY_MS));
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(timeout),
                                fulfill_request(
                                    &our,
                                    km.id,
                                    &send_to_loop,
                                    &eth_action,
                                    &providers,
                                    &mut receiver,
                                    &response_channels,
                                    &print_tx,
                                    &mut request_cache,
                                ),
                            )
                            .await
                            {
                                Ok(response) => {
                                    kernel_message(
                                        &our,
                                        km.id,
                                        km.rsvp.clone().unwrap_or(km.source.clone()),
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
                                    error_message(
                                        &our,
                                        km.id,
                                        km.source.clone(),
                                        EthError::RpcTimeout,
                                        &send_to_loop,
                                    )
                                    .await;
                                }
                            }
                        } else {
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
    eth_action: &EthAction,
    providers: &Providers,
    remote_request_receiver: &mut ProcessMessageReceiver,
    response_channels: &ResponseChannels,
    print_tx: &PrintSender,
    request_cache: &mut RequestCache,
) -> EthResponse {
    let serialized_action = serde_json::to_vec(eth_action).unwrap();
    let EthAction::Request {
        ref chain_id,
        ref method,
        ref params,
    } = eth_action
    else {
        return EthResponse::Err(EthError::PermissionDenied); // will never hit
    };
    {
        let mut request_cache = request_cache.lock().await;
        if let Some((cache_hit, time_of_hit)) = request_cache.shift_remove(&serialized_action) {
            // refresh cache entry (it is most recently accessed) & return it
            if time_of_hit.elapsed() < Duration::from_millis(DELAY_MS) {
                request_cache.insert(serialized_action, (cache_hit.clone(), time_of_hit));
                return cache_hit;
            }
        }
    }
    let Some(method) = valid_method(&method) else {
        return EthResponse::Err(EthError::InvalidMethod(method.to_string()));
    };
    let urls = {
        // in code block to drop providers lock asap to avoid deadlock
        let Some(aps) = providers.get(&chain_id) else {
            return EthResponse::Err(EthError::NoRpcForChain);
        };
        aps.urls.clone()
    };

    // first, try any url providers we have for this chain,
    // then if we have none or they all fail, go to node providers.
    // finally, if no provider works, return an error.

    // Track all errors for comprehensive error reporting if all fail
    let mut all_errors = Vec::new();
    // Keep track of the last valid RPC error response (e.g., rate limits)
    let mut last_rpc_error: Option<serde_json::Value> = None;

    // Try URL providers, respecting their order but skipping offline ones
    for mut url_provider in urls.into_iter() {
        // Skip offline providers
        if !url_provider.online {
            verbose_print(
                print_tx,
                &format!("eth: skipping offline url provider {}", url_provider.url),
            )
            .await;
            continue;
        }

        // Check method-specific failures
        if url_provider
            .method_failures
            .should_skip_method(method, params)
        {
            verbose_print(
                print_tx,
                &format!(
                    "eth: skipping url provider {} due to previous {} failure",
                    url_provider.url, method
                ),
            )
            .await;
            continue;
        }
        let (pubsub, newly_activated) = match url_provider.pubsub.first() {
            Some(pubsub) => (pubsub, false),
            None => {
                if let Ok(()) = activate_url_provider(&mut url_provider).await {
                    verbose_print(
                        print_tx,
                        &format!("eth: activated url provider {}", url_provider.url),
                    )
                    .await;
                    (url_provider.pubsub.last().unwrap(), true)
                } else {
                    verbose_print(
                        print_tx,
                        &format!("eth: could not activate url provider {}", url_provider.url),
                    )
                    .await;
                    continue;
                }
            }
        };
        match pubsub.raw_request(method.into(), params).await {
            Ok(value) => {
                // Provider succeeded - clear any method failures and update pubsub if needed
                providers.entry(chain_id.clone()).and_modify(|aps| {
                    if let Some(provider) = aps.urls.iter_mut().find(|p| p.url == url_provider.url)
                    {
                        // Clear method failure since it succeeded
                        provider.method_failures.clear_method_failure(method);

                        if newly_activated {
                            provider.pubsub.push(url_provider.pubsub.pop().unwrap());
                        }
                    }
                });

                let response = EthResponse::Response(value);
                let mut request_cache = request_cache.lock().await;
                if request_cache.len() >= MAX_REQUEST_CACHE_LEN {
                    // drop 10% oldest cache entries
                    request_cache.drain(0..MAX_REQUEST_CACHE_LEN / 10);
                }
                request_cache.insert(serialized_action, (response.clone(), Instant::now()));
                return response;
            }
            Err(rpc_error) => {
                verbose_print(
                    print_tx,
                    &format!(
                        "eth: got error from url provider {}: {}",
                        url_provider.url, rpc_error
                    ),
                )
                .await;

                // Track the error
                all_errors.push((url_provider.url.clone(), format!("{:?}", rpc_error)));

                // Store RPC error responses for later if all providers fail
                let is_rpc_error_resp = if let RpcError::ErrorResp(err) = &rpc_error {
                    last_rpc_error =
                        Some(serde_json::to_value(err).unwrap_or_else(|_| serde_json::Value::Null));
                    true
                } else {
                    false
                };

                // Determine what to mark as failed
                if is_rpc_error_resp {
                    // Valid RPC error response - mark the specific method as failed
                    let mut should_spawn_retry = false;
                    providers.entry(chain_id.clone()).and_modify(|aps| {
                        let Some(provider) =
                            aps.urls.iter_mut().find(|p| p.url == url_provider.url)
                        else {
                            return;
                        };
                        // Check if this method wasn't already marked as failed
                        if !provider.method_failures.should_skip_method(method, params) {
                            provider.method_failures.mark_method_failed(method, params);
                            should_spawn_retry = true;
                        }
                    });

                    // Spawn method retry task if this is a new failure
                    if should_spawn_retry && method != "eth_sendRawTransaction" {
                        crate::eth::utils::spawn_method_retry_for_url_provider(
                            providers.clone(),
                            chain_id.clone(),
                            url_provider.url.clone(),
                            method.to_string(),
                            params.clone(),
                            print_tx.clone(),
                        );
                        verbose_print(
                            print_tx,
                            &format!(
                                "eth: spawned method retry for {} on {}",
                                method, url_provider.url
                            ),
                        )
                        .await;
                    }
                    // Continue to next provider without marking offline
                    continue;
                }

                // Transport/connection error - mark the provider as offline and spawn health check
                let mut spawn_health_check = false;
                providers.entry(chain_id.clone()).and_modify(|aps| {
                    let Some(index) = find_index(
                        &aps.urls.iter().map(|u| u.url.as_str()).collect(),
                        &url_provider.url,
                    ) else {
                        return ();
                    };
                    let mut url = aps.urls.remove(index);
                    url.pubsub = vec![];
                    url.online = false;
                    url.last_health_check = Some(Instant::now());

                    // Only spawn health check if not already running
                    if url.last_health_check.is_none()
                        || url.last_health_check.unwrap().elapsed() > Duration::from_secs(30)
                    {
                        spawn_health_check = true;
                    }

                    aps.urls.insert(index, url);
                });

                // Spawn health check task if needed
                if spawn_health_check {
                    use crate::eth::utils::spawn_health_check_for_url_provider;
                    spawn_health_check_for_url_provider(
                        providers.clone(),
                        chain_id.clone(),
                        url_provider.url.clone(),
                        print_tx.clone(),
                    );

                    verbose_print(
                        print_tx,
                        &format!(
                            "eth: spawned health check for offline provider {}",
                            url_provider.url
                        ),
                    )
                    .await;
                }
            }
        }
    }

    let nodes = {
        // in code block to drop providers lock asap to avoid deadlock
        let Some(aps) = providers.get(&chain_id) else {
            return EthResponse::Err(EthError::NoRpcForChain);
        };
        aps.nodes.clone()
    };
    for node_provider in &nodes {
        // Skip offline node providers
        if !node_provider.online || !node_provider.usable {
            verbose_print(
                print_tx,
                &format!(
                    "eth: skipping offline/unusable node provider {}",
                    node_provider.hns_update.name
                ),
            )
            .await;
            continue;
        }

        // Check method-specific failures
        if node_provider
            .method_failures
            .should_skip_method(method, params)
        {
            verbose_print(
                print_tx,
                &format!(
                    "eth: skipping node provider {} due to previous {} failure",
                    node_provider.hns_update.name, method
                ),
            )
            .await;
            continue;
        }

        verbose_print(
            print_tx,
            &format!(
                "eth: attempting to fulfill via {}",
                node_provider.hns_update.name
            ),
        )
        .await;
        let response = forward_to_node_provider(
            our,
            km_id,
            None,
            node_provider,
            eth_action.clone(),
            send_to_loop,
            remote_request_receiver,
        )
        .await;

        if let EthResponse::Err(e) = &response {
            // Track the error
            all_errors.push((node_provider.hns_update.name.clone(), format!("{:?}", e)));

            // Check if it's an RPC error (method failure) vs transport error
            let is_rpc_error = matches!(e, EthError::RpcError(_));

            if is_rpc_error {
                // Mark the specific method as failed
                let mut should_spawn_retry = false;
                providers.entry(chain_id.clone()).and_modify(|aps| {
                    let Some(provider) = aps
                        .nodes
                        .iter_mut()
                        .find(|p| p.hns_update.name == node_provider.hns_update.name)
                    else {
                        return;
                    };
                    // Check if this method wasn't already marked as failed
                    if !provider.method_failures.should_skip_method(method, params) {
                        provider.method_failures.mark_method_failed(method, params);
                        should_spawn_retry = true;
                    }
                });
                // Store the RPC error
                if let EthError::RpcError(err_value) = e {
                    last_rpc_error = Some(err_value.clone());
                }

                // Spawn method retry task if this is a new failure
                if should_spawn_retry && method != "eth_sendRawTransaction" {
                    use crate::eth::utils::spawn_method_retry_for_node_provider;
                    spawn_method_retry_for_node_provider(
                        our.to_string(),
                        providers.clone(),
                        chain_id.clone(),
                        node_provider.hns_update.name.clone(),
                        method.to_string(),
                        params.clone(),
                        send_to_loop.clone(),
                        response_channels.clone(),
                        print_tx.clone(),
                    );
                    verbose_print(
                        print_tx,
                        &format!(
                            "eth: spawned method retry for {} on node {}",
                            method, node_provider.hns_update.name
                        ),
                    )
                    .await;
                }
            } else {
                // Transport/timeout error - mark node as offline and spawn health check
                let mut spawn_health_check = false;
                providers.entry(chain_id.clone()).and_modify(|aps| {
                    let Some(provider) = aps
                        .nodes
                        .iter_mut()
                        .find(|p| p.hns_update.name == node_provider.hns_update.name)
                    else {
                        return;
                    };
                    provider.online = false;
                    provider.usable = false;

                    // Only spawn health check if not recently checked
                    if provider.last_health_check.is_none()
                        || provider.last_health_check.unwrap().elapsed() > Duration::from_secs(30)
                    {
                        spawn_health_check = true;
                        provider.last_health_check = Some(Instant::now());
                    }
                });

                // Spawn health check task if needed
                if spawn_health_check {
                    use crate::eth::utils::spawn_health_check_for_node_provider;
                    spawn_health_check_for_node_provider(
                        our.to_string(),
                        providers.clone(),
                        chain_id.clone(),
                        node_provider.hns_update.name.clone(),
                        send_to_loop.clone(),
                        response_channels.clone(),
                        print_tx.clone(),
                    );

                    verbose_print(
                        print_tx,
                        &format!(
                            "eth: spawned health check for offline node provider {}",
                            node_provider.hns_update.name
                        ),
                    )
                    .await;
                }
            }
            // Continue trying other providers instead of returning the error
            continue;
        } else {
            // Success! Clear method failure and return the response
            providers.entry(chain_id.clone()).and_modify(|aps| {
                if let Some(provider) = aps
                    .nodes
                    .iter_mut()
                    .find(|p| p.hns_update.name == node_provider.hns_update.name)
                {
                    provider.method_failures.clear_method_failure(method);
                }
            });
            return response;
        }
    }

    // All providers failed, return comprehensive error
    if all_errors.is_empty() {
        EthResponse::Err(EthError::NoRpcForChain)
    } else {
        verbose_print(
            print_tx,
            &format!(
                "eth: all providers failed for chain {}: {:?}",
                chain_id, all_errors
            ),
        )
        .await;

        // If we have a valid RPC error response from any provider, return that
        // This gives the user more specific information about why the request failed
        if let Some(rpc_error) = last_rpc_error {
            EthResponse::Err(EthError::RpcError(rpc_error))
        } else {
            EthResponse::Err(EthError::NoRpcForChain)
        }
    }
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
    if !node_provider.usable || node_provider.hns_update.name == our {
        return EthResponse::Err(EthError::PermissionDenied);
    }
    kernel_message(
        our,
        km_id,
        Address {
            node: node_provider.hns_update.name.clone(),
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
    if let Message::Response((resp, _context)) = response_km.message {
        if let Ok(eth_response) = serde_json::from_slice::<EthResponse>(&resp.body) {
            return eth_response;
        }
    }
    // if we hit this, they sent a malformed response, ignore and possibly punish
    EthResponse::Err(EthError::RpcMalformedResponse)
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

    let mut save_settings = false;
    let mut save_providers = false;
    let mut provider_not_found = false;

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
                if aps.remove_provider(&remove) {
                    save_providers = true;
                } else {
                    provider_not_found = true;
                }
            } else {
                provider_not_found = true;
            }
        }
        EthConfigAction::SetPublic => {
            state.access_settings.public = true;
            save_settings = true;
        }
        EthConfigAction::SetPrivate => {
            state.access_settings.public = false;
            save_settings = true;
        }
        EthConfigAction::AllowNode(node) => {
            state.access_settings.allow.insert(node);
            save_settings = true;
        }
        EthConfigAction::UnallowNode(node) => {
            state.access_settings.allow.remove(&node);
            save_settings = true;
        }
        EthConfigAction::DenyNode(node) => {
            state.access_settings.deny.insert(node);
            save_settings = true;
        }
        EthConfigAction::UndenyNode(node) => {
            state.access_settings.deny.remove(&node);
            save_settings = true;
        }
        EthConfigAction::SetProviders(new_providers) => {
            let new_map = DashMap::new();
            for entry in new_providers.0.into_iter().rev() {
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
    // save providers and/or access settings, depending on necessity, to disk
    if save_settings {
        if let Ok(()) = tokio::fs::write(
            state.home_directory_path.join(".eth_access_settings"),
            serde_json::to_string(&state.access_settings).unwrap(),
        )
        .await
        {
            verbose_print(&state.print_tx, "eth: saved new access settings").await;
        };
    }
    if save_providers {
        let saved_configs = providers_to_saved_configs(&state.providers);

        if let Ok(()) = tokio::fs::write(
            state.home_directory_path.join(".eth_providers"),
            serde_json::to_string(&saved_configs).unwrap(),
        )
            .await
        {
            verbose_print(&state.print_tx, "eth: saved new provider settings").await;

            /* TODO CLEANUP
            // Also update the base L2 providers in options config
            if let Err(e) = crate::options_config_utils::update_base_l2_providers_from_saved_configs(&saved_configs).await {
                verbose_print(&state.print_tx, &format!("eth: failed to update base L2 providers in options config: {}", e)).await;
            } else {
                verbose_print(&state.print_tx, "eth: updated base L2 providers in options config").await;
            }
            */
        };
    }
    if provider_not_found {
        EthConfigResponse::ProviderNotFound
    } else {
        EthConfigResponse::Ok
    }
}
