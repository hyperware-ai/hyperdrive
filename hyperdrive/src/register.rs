use crate::{keygen, sol::*};
use crate::{HYPERMAP_ADDRESS, MULTICALL_ADDRESS};
//use crate::eth_config_utils::add_provider_to_config;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy::signers::Signature;
use alloy_primitives::{Address as EthAddress, Bytes, FixedBytes, U256};
use alloy_sol_types::{eip712_domain, SolCall, SolStruct};
use base64::{engine::general_purpose::STANDARD as base64_standard, Engine};
use lib::types::core::{
    BootInfo, Identity, ImportKeyfileInfo, InfoResponse, Keyfile, LoginInfo, NodeRouting,
};
use ring::{rand::SystemRandom, signature, signature::KeyPair};
use std::{
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
use warp::{
    http::{
        header::{HeaderMap, HeaderValue, SET_COOKIE},
        StatusCode,
    },
    Filter, Rejection, Reply,
};
#[cfg(feature = "simulation-mode")]
use {alloy_sol_macro::sol, alloy_sol_types::SolValue};

/// Default fallback RPC URLs for Base L2.
/// These are used when user-configured providers fail or return stale data.
const DEFAULT_RPC_URLS: &[&str] = &[
    "wss://base.llamarpc.com",
    "wss://base-rpc.publicnode.com",
    "wss://base.drpc.org",
    "wss://base.gateway.tenderly.co",
];

/// Check if a hypermap entry is empty (stale provider data).
/// Returns true if all fields are zero/empty.
fn is_hypermap_entry_empty(entry: &getReturn) -> bool {
    entry.tba == EthAddress::ZERO && entry.owner == EthAddress::ZERO && entry.data.is_empty()
}

type RegistrationSender = mpsc::Sender<(Identity, Keyfile, Vec<u8>, Vec<String>, Vec<String>)>;

/// Serve the registration page and receive POSTs and PUTs from it
pub async fn register(
    tx: RegistrationSender,
    kill_rx: oneshot::Receiver<bool>,
    ip: String,
    ws_networking: (Option<&tokio::net::TcpListener>, bool),
    tcp_networking: (Option<&tokio::net::TcpListener>, bool),
    http_port: u16,
    keyfile: Option<Vec<u8>>,
    eth_provider_config: lib::eth::SavedConfigs,
    detached: bool,
    initial_cache_sources: Option<Vec<String>>,
    initial_base_l2_providers: Option<Vec<String>>,
) {
    // Networking info is generated and passed to the UI, but not used until confirmed
    let (public_key, serialized_networking_keypair) = keygen::generate_networking_key();
    let net_keypair = Arc::new(serialized_networking_keypair.as_ref().to_vec());
    let tx = Arc::new(tx);

    let ws_port = match ws_networking.0 {
        Some(listener) => listener.local_addr().unwrap().port(),
        None => 0,
    };
    let ws_flag_used = ws_networking.1;
    let tcp_port = match tcp_networking.0 {
        Some(listener) => listener.local_addr().unwrap().port(),
        None => 0,
    };
    let tcp_flag_used = tcp_networking.1;

    let mut ports_map = std::collections::BTreeMap::new();
    if ws_port != 0 {
        ports_map.insert("ws".to_string(), ws_port);
    }
    if tcp_port != 0 {
        ports_map.insert("tcp".to_string(), tcp_port);
    }

    // This is a **temporary** identity, passed to the UI.
    // If it is confirmed through a /boot, then it will be used to replace the current identity.
    let our_temp_id = Arc::new(Identity {
        networking_key: format!("0x{}", public_key),
        name: "".to_string(),
        routing: NodeRouting::Both {
            ip: ip.clone(),
            ports: ports_map,
            routers: {
                // select 3 random routers from this list
                use rand::prelude::SliceRandom;
                let routers = (1..=12)
                    .map(|i| format!("default-router-{}.hypr", i))
                    .collect::<Vec<_>>()
                    .choose_multiple(&mut rand::thread_rng(), 3)
                    .cloned()
                    .collect::<Vec<_>>();
                routers
            },
        },
    });

    let providers = Arc::new(connect_to_providers(&eth_provider_config).await);

    let keyfile = warp::any().map(move || keyfile.clone());
    let our_temp_id = warp::any().map(move || our_temp_id.clone());
    let net_keypair = warp::any().map(move || net_keypair.clone());
    let tx = warp::any().map(move || tx.clone());
    let ip = warp::any().map(move || ip.clone());
    let ws_port = warp::any().map(move || (ws_port, ws_flag_used));
    let tcp_port = warp::any().map(move || (tcp_port, tcp_flag_used));

    #[cfg(unix)]
    let static_files =
        warp::path("assets").and(static_dir::static_dir!("src/register-ui/build/assets/"));
    #[cfg(target_os = "windows")]
    let static_files =
        warp::path("assets").and(static_dir::static_dir!("src\\register-ui\\build\\assets\\"));

    #[cfg(unix)]
    let react_app = warp::path::end()
        .or(warp::path("login"))
        .or(warp::path("commit-os-name"))
        .or(warp::path("mint-os-name"))
        .or(warp::path("claim-invite"))
        .or(warp::path("reset"))
        .or(warp::path("import-keyfile"))
        .or(warp::path("set-password"))
        .or(warp::path("custom-register"))
        .and(warp::get())
        .map(move |_| warp::reply::html(include_str!("register-ui/build/index.html").to_string()));
    #[cfg(target_os = "windows")]
    let react_app = warp::path::end()
        .or(warp::path("login"))
        .or(warp::path("commit-os-name"))
        .or(warp::path("mint-os-name"))
        .or(warp::path("claim-invite"))
        .or(warp::path("reset"))
        .or(warp::path("import-keyfile"))
        .or(warp::path("set-password"))
        .or(warp::path("custom-register"))
        .and(warp::get())
        .map(move |_| {
            warp::reply::html(include_str!("register-ui\\build\\index.html").to_string())
        });

    let boot_providers = providers.clone();
    let login_providers = providers.clone();
    let import_providers = providers.clone();
    let info_providers = providers.clone();

    let initial_cache_sources_arc = Arc::new(initial_cache_sources.clone());
    let initial_base_l2_providers_arc = Arc::new(initial_base_l2_providers.clone());

    let api = warp::path("info")
        .and(
            warp::get()
                .and(keyfile.clone())
                .and(warp::any().map(move || initial_cache_sources_arc.clone()))
                .and(warp::any().map(move || initial_base_l2_providers_arc.clone()))
                .and(warp::any().map(move || info_providers.clone()))
                .and_then(get_unencrypted_info),
        )
        .or(warp::path("current-chain")
            .and(warp::get())
            .map(move || warp::reply::json(&"0xa".to_string())))
        .or(warp::path("our").and(warp::get()).and(keyfile.clone()).map(
            move |keyfile: Option<Vec<u8>>| {
                if let Some(keyfile) = keyfile {
                    if let Ok((username, _, _, _, _, _)) = serde_json::from_slice::<(
                        String,
                        Vec<String>,
                        Vec<u8>,
                        Vec<u8>,
                        Vec<u8>,
                        Vec<u8>,
                    )>(&keyfile)
                    .or_else(|_| {
                        bincode::deserialize::<(
                            String,
                            Vec<String>,
                            Vec<u8>,
                            Vec<u8>,
                            Vec<u8>,
                            Vec<u8>,
                        )>(&keyfile)
                    }) {
                        return warp::reply::html(username);
                    }
                }
                warp::reply::html(String::new())
            },
        ))
        .or(warp::path("generate-networking-info").and(
            warp::post()
                .and(our_temp_id.clone())
                .and_then(generate_networking_info),
        ))
        .or(warp::path("boot").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(tx.clone())
                .and(our_temp_id.clone())
                .and(net_keypair.clone())
                .and_then(move |boot_info, tx, our_temp_id, net_keypair| {
                    let boot_providers = boot_providers.clone();
                    handle_boot(boot_info, tx, our_temp_id, net_keypair, boot_providers)
                }),
        ))
        .or(warp::path("import-keyfile").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(ip.clone())
                .and(ws_port.clone())
                .and(tcp_port.clone())
                .and(tx.clone())
                .and_then(move |boot_info, ip, ws_port, tcp_port, tx| {
                    let import_providers = import_providers.clone();
                    handle_import_keyfile(boot_info, ip, ws_port, tcp_port, tx, import_providers)
                }),
        ))
        .or(warp::path("login").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(ip)
                .and(ws_port.clone())
                .and(tcp_port.clone())
                .and(tx.clone())
                .and(keyfile.clone())
                .and_then(move |boot_info, ip, ws_port, tcp_port, tx, keyfile| {
                    let login_providers = login_providers.clone();
                    handle_login(
                        boot_info,
                        ip,
                        ws_port,
                        tcp_port,
                        tx,
                        keyfile,
                        login_providers,
                    )
                }),
        ));

    let mut headers = HeaderMap::new();
    headers.insert(
        "Cache-Control",
        HeaderValue::from_static("no-store, no-cache, must-revalidate, proxy-revalidate"),
    );

    let routes = static_files
        .or(react_app)
        .or(api)
        .with(warp::reply::with::headers(headers));

    if !detached {
        let _ = open::that(format!("http://localhost:{}/", http_port));
    }
    warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], http_port), async {
            kill_rx.await.ok();
        })
        .1
        .await;
}

/// Connect to as many providers as possible from the saved configuration and fallbacks.
/// Returns a Vec of connected providers (user-configured first, then fallbacks).
/// Panics if unable to connect to any provider.
pub async fn connect_to_providers(
    eth_provider_config: &lib::eth::SavedConfigs,
) -> Vec<RootProvider<PubSubFrontend>> {
    let saved_configs = &eth_provider_config.0;
    let mut providers = Vec::new();

    // Try each configured provider first
    for (_index, provider_config) in saved_configs.iter().enumerate() {
        match &provider_config.provider {
            lib::eth::NodeOrRpcUrl::RpcUrl { url, auth } => {
                let ws_connect = WsConnect {
                    url: url.clone(),
                    auth: auth.clone().map(|a| a.into()),
                    config: None,
                };

                match ProviderBuilder::new().on_ws(ws_connect).await {
                    Ok(client) => {
                        println!("Connected to configured provider: {url}\r");
                        providers.push(client);
                    }
                    Err(_) => {
                        println!("Failed to connect to configured provider: {url}\r");
                    }
                }
            }
            lib::eth::NodeOrRpcUrl::Node { .. } => {
                // Node providers are not supported in registration, skip to next
            }
        }
    }

    // Also connect to default fallback providers
    for rpc_url in DEFAULT_RPC_URLS.iter() {
        let ws_connect = WsConnect {
            url: rpc_url.to_string(),
            auth: None,
            config: None,
        };

        match ProviderBuilder::new().on_ws(ws_connect).await {
            Ok(client) => {
                println!("Connected to fallback provider: {rpc_url}\r");
                providers.push(client);
            }
            Err(_) => {
                println!("Failed to connect to fallback provider: {rpc_url}\r");
            }
        }
    }

    if providers.is_empty() {
        panic!(
            "Error: runtime could not connect to any configured or fallback Base ETH RPC providers\n\
            This is necessary in order to verify node identity onchain.\n\
            Please make sure you are using a valid WebSockets URL if using \
            the --rpc or --rpc-config flag, and you are connected to the internet."
        );
    }

    println!(
        "Connected to {} provider(s) for registration\r",
        providers.len()
    );
    providers
}

async fn get_unencrypted_info(
    keyfile: Option<Vec<u8>>,
    initial_cache_sources: Arc<Option<Vec<String>>>,
    initial_base_l2_providers: Arc<Option<Vec<String>>>,
    provider: Arc<RootProvider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    let (name, allowed_routers, status_code) = match keyfile {
        Some(encoded_keyfile) => match keygen::get_username_and_routers(&encoded_keyfile) {
            Ok(k) => (Some(k.0), Some(k.1), StatusCode::OK),
            Err(_) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&"keyfile deserialization went wrong".to_string()),
                    StatusCode::UNAUTHORIZED,
                )
                .into_response())
            }
        },
        None => (None, None, StatusCode::NOT_FOUND),
    };

    // Use the shared helper function to detect IP
    let detected_ip_address = detect_ipv4_address().await;

    // Determine networking configuration and HNS IP from chain
    let (uses_direct_networking, hns_ip_address) = if let Some(ref routers) = allowed_routers {
        if routers.is_empty() {
            // This is a direct node - query the chain for its IP
            if let Some(ref node_name) = name {
                match query_hns_ip(node_name, &provider).await {
                    Ok(ip) => (true, Some(ip)),
                    Err(e) => {
                        println!("Failed to query HNS IP for {}: {}\r", node_name, e);
                        (true, None)
                    }
                }
            } else {
                (true, None)
            }
        } else {
            // This is an indirect node
            (false, None)
        }
    } else {
        (false, None)
    };

    let response = InfoResponse {
        name,
        allowed_routers,
        initial_cache_sources: initial_cache_sources.as_ref().clone().unwrap_or_default(),
        initial_base_l2_providers: initial_base_l2_providers
            .as_ref()
            .clone()
            .unwrap_or_default(),
        uses_direct_networking,
        hns_ip_address,
        detected_ip_address,
    };

    return Ok(warp::reply::with_status(warp::reply::json(&response), status_code).into_response());
}

/// Query the HNS IP address for a given node name from the chain
async fn query_hns_ip(
    node_name: &str,
    provider: &RootProvider<PubSubFrontend>,
) -> anyhow::Result<String> {
    let hypermap = EthAddress::from_str(HYPERMAP_ADDRESS)?;
    let ip_hash = FixedBytes::<32>::from_slice(&keygen::namehash(&format!("~ip.{}", node_name)));

    let get_call = getCall { namehash: ip_hash }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(get_call));
    let tx = TransactionRequest::default().to(hypermap).input(tx_input);

    let result = provider.call(&tx).await?;
    let ip_data = getCall::abi_decode_returns(&result, false)?;

    let ip = keygen::bytes_to_ip(&ip_data.data)?;
    Ok(ip.to_string())
}

async fn generate_networking_info(our_temp_id: Arc<Identity>) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(our_temp_id.as_ref()))
}

async fn detect_ipv4_address() -> String {
    #[cfg(feature = "simulation-mode")]
    {
        return std::net::Ipv4Addr::LOCALHOST.to_string();
    }

    #[cfg(not(feature = "simulation-mode"))]
    {
        // Helper function to parse IP from JSON response
        async fn try_hyperware_endpoint(url: &str) -> Option<String> {
            match tokio::time::timeout(std::time::Duration::from_secs(3), reqwest::get(url)).await {
                Ok(Ok(response)) => {
                    if let Ok(json) = response.json::<serde_json::Value>().await {
                        if let Some(ip) = json.get("ip").and_then(|v| v.as_str()) {
                            // Validate it's a proper IPv4 address
                            if let Ok(parsed_ip) = ip.parse::<std::net::Ipv4Addr>() {
                                println!("Detected IPv4 address from {}: {}\r", url, parsed_ip);
                                return Some(parsed_ip.to_string());
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    println!("Failed to fetch IP from {}: {}\r", url, e);
                }
                Err(_) => {
                    println!("Timeout fetching IP from {}\r", url);
                }
            }
            None
        }

        // Try ip-address-2.hyperware.ai first
        if let Some(ip) = try_hyperware_endpoint("https://ip-address-2.hyperware.ai/").await {
            return ip;
        }

        // Try ip-address-1.hyperware.ai as fallback
        if let Some(ip) = try_hyperware_endpoint("https://ip-address-1.hyperware.ai/").await {
            return ip;
        }

        // Final fallback to public_ip crate
        println!("Falling back to public_ip crate for IP detection\r");
        match tokio::time::timeout(std::time::Duration::from_secs(5), public_ip::addr_v4()).await {
            Ok(Some(ip)) => {
                println!("Detected IPv4 address from public_ip crate: {}\r", ip);
                ip.to_string()
            }
            _ => {
                println!("Failed to find public IPv4 address for /info endpoint.\r");
                std::net::Ipv4Addr::LOCALHOST.to_string()
            }
        }
    }
}

async fn handle_boot(
    info: BootInfo,
    sender: Arc<RegistrationSender>,
    our: Arc<Identity>,
    networking_keypair: Arc<Vec<u8>>,
    providers: Arc<Vec<RootProvider<PubSubFrontend>>>,
) -> Result<impl Reply, Rejection> {
    let hypermap = EthAddress::from_str(HYPERMAP_ADDRESS).unwrap();
    let mut our = our.as_ref().clone();

    our.name = info.username.clone();

    // Check if direct IP address is provided (Some = direct, None = routers)
    let is_direct = info.direct.is_some();

    if is_direct {
        let ip_address = info.direct.as_ref().unwrap();
        our.both_to_direct(ip_address);
    } else {
        // Set custom routers if provided
        if let Some(custom_routers) = info.custom_routers.clone() {
            our.routing = NodeRouting::Routers(custom_routers);
        } else {
            our.both_to_routers(); // Use defaults
        }
    }

    let cache_source_vector = if let Some(custom_cache_sources) = &info.custom_cache_sources {
        println!(
            "Custom cache sources specified: {:?}\r",
            custom_cache_sources
        );
        custom_cache_sources.clone()
    } else {
        println!("No custom cache sources specified\r");
        Vec::new()
    };

    let base_l2_access_source_vector =
        if let Some(custom_base_l2_providers) = &info.custom_base_l2_access_providers {
            println!(
                "Custom Base L2 access providers specified: {:?}\r",
                custom_base_l2_providers
            );
            custom_base_l2_providers.clone()
        } else {
            println!("No custom Base L2 access providers specified\r");
            Vec::new()
        };

    let jwt_seed = SystemRandom::new();
    let mut jwt_secret = [0u8, 32];
    ring::rand::SecureRandom::fill(&jwt_seed, &mut jwt_secret).unwrap();

    // verifying owner + signature, get registrar contract, call auth()
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    if info.timestamp < now + 120 {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Timestamp is outdated.".to_string()),
            StatusCode::UNAUTHORIZED,
        )
        .into_response());
    }
    let Ok(password_hash) = FixedBytes::<32>::from_str(&info.password_hash) else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Invalid password hash".to_string()),
            StatusCode::UNAUTHORIZED,
        )
        .into_response());
    };

    let namehash = FixedBytes::<32>::from_slice(&keygen::namehash(&our.name));

    let get_call = getCall { namehash }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(get_call));

    let tx = TransactionRequest::default().to(hypermap).input(tx_input);

    // this call can fail if the indexer has not caught up to the transaction
    // that just got confirmed on our frontend. for this reason, we retry
    // the call a few times before giving up.
    //
    // Additionally, if a provider returns Address::ZERO for the owner,
    // it likely means that provider has stale/out-of-date data. In this case,
    // we try all other providers before giving up on this attempt.
    let mut attempts = 0;

    while attempts < 5 {
        // Try each provider in turn
        for (provider_index, current_provider) in providers.iter().enumerate() {
            match current_provider.call(&tx).await {
                Ok(get) => {
                    let Ok(node_info) = getCall::abi_decode_returns(&get, false) else {
                        return Ok(warp::reply::with_status(
                            warp::reply::json(
                                &"Failed to decode hypermap entry from return bytes".to_string(),
                            ),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                        .into_response());
                    };
                    // If all fields are zero/empty, the provider likely has stale data.
                    // Try the next provider.
                    if is_hypermap_entry_empty(&node_info) {
                        println!(
                            "Provider {} returned empty hypermap entry, trying next provider...\r",
                            provider_index
                        );
                        continue;
                    }

                    let owner = node_info.owner;

                    let chain_id: u64 = 8453; // base

                    let domain = eip712_domain! {
                        name: "Hypermap",
                        version: "1",
                        chain_id: chain_id,
                        verifying_contract: hypermap,
                    };

                let boot = Boot {
                    username: our.name.clone(),
                    password_hash,
                    timestamp: U256::from(info.timestamp),
                    direct: is_direct, // Convert Option<String> to bool
                    reset: info.reset,
                    chain_id: U256::from(chain_id),
                };

                    let hash = boot.eip712_signing_hash(&domain);
                    let sig = Signature::from_str(&info.signature).map_err(|_| warp::reject())?;

                    let recovered_address = sig
                        .recover_address_from_prehash(&hash)
                        .map_err(|_| warp::reject())?;

                    if recovered_address != owner {
                        println!("recovered_address: {}\r", recovered_address);
                        println!("owner: {}\r", owner);
                        // Try next provider
                        continue;
                    }

                    let decoded_keyfile = Keyfile {
                        username: our.name.clone(),
                        routers: our.routers().unwrap_or(&vec![]).clone(),
                        networking_keypair: signature::Ed25519KeyPair::from_pkcs8(
                            networking_keypair.as_ref(),
                        )
                        .unwrap(),
                        jwt_secret_bytes: jwt_secret.to_vec(),
                        file_key: keygen::generate_file_key(),
                    };

                    let encoded_keyfile = keygen::encode_keyfile(
                        info.password_hash,
                        decoded_keyfile.username.clone(),
                        decoded_keyfile.routers.clone(),
                        &networking_keypair,
                        &decoded_keyfile.jwt_secret_bytes,
                        &decoded_keyfile.file_key,
                    );

                    return success_response(
                        sender,
                        our,
                        decoded_keyfile,
                        encoded_keyfile,
                        cache_source_vector,
                        base_l2_access_source_vector,
                    )
                    .await;
                }
                Err(_) => {
                    // Try next provider
                    continue;
                }
            }
        }

        // All providers exhausted for this attempt, sleep and retry
        println!("All providers failed or returned invalid data, retrying...\r");
        attempts += 1;
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    return Ok(warp::reply::with_status(
        warp::reply::json(&"Recovered address does not match owner".to_string()),
        StatusCode::UNAUTHORIZED,
    )
    .into_response());
}

async fn handle_import_keyfile(
    info: ImportKeyfileInfo,
    ip: String,
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
    sender: Arc<RegistrationSender>,
    providers: Arc<Vec<RootProvider<PubSubFrontend>>>,
) -> Result<impl Reply, Rejection> {
    println!("received base64 keyfile: {}\r", info.keyfile);
    // if keyfile was not present in node and is present from user upload
    let encoded_keyfile = match base64_standard.decode(info.keyfile) {
        Ok(k) => k,
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Keyfile not valid base64".to_string()),
                StatusCode::BAD_REQUEST,
            )
            .into_response())
        }
    };

    println!(
        "received keyfile: {}\r",
        String::from_utf8_lossy(&encoded_keyfile)
    );
    let (decoded_keyfile, mut our) =
        match keygen::decode_keyfile(&encoded_keyfile, &info.password_hash) {
            Ok(k) => {
                let our = Identity {
                    name: k.username.clone(),
                    networking_key: format!(
                        "0x{}",
                        hex::encode(k.networking_keypair.public_key().as_ref())
                    ),
                    routing: if k.routers.is_empty() {
                        NodeRouting::Direct {
                            ip,
                            ports: std::collections::BTreeMap::new(),
                        }
                    } else {
                        NodeRouting::Routers(k.routers.clone())
                    },
                };

                (k, our)
            }
            Err(_) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&"Incorrect password!".to_string()),
                    StatusCode::UNAUTHORIZED,
                )
                .into_response())
            }
        };

    if let Err(e) = assign_routing(
        &mut our,
        &providers,
        ws_networking_port,
        tcp_networking_port,
    )
    .await
    {
        return Ok(warp::reply::with_status(
            warp::reply::json(&e.to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response());
    }
    success_response(
        sender,
        our,
        decoded_keyfile,
        encoded_keyfile,
        Vec::new(),
        Vec::new(),
    )
    .await
}

async fn handle_login(
    info: LoginInfo,
    ip: String,
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
    sender: Arc<RegistrationSender>,
    encoded_keyfile: Option<Vec<u8>>,
    providers: Arc<Vec<RootProvider<PubSubFrontend>>>,
) -> Result<impl Reply, Rejection> {
    if encoded_keyfile.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Keyfile not present".to_string()),
            StatusCode::NOT_FOUND,
        )
        .into_response());
    }
    let encoded_keyfile = encoded_keyfile.unwrap();

    let (decoded_keyfile, mut our) =
        match keygen::decode_keyfile(&encoded_keyfile, &info.password_hash) {
            Ok(k) => {
                let our = Identity {
                    name: k.username.clone(),
                    networking_key: format!(
                        "0x{}",
                        hex::encode(k.networking_keypair.public_key().as_ref())
                    ),
                    routing: if k.routers.is_empty() {
                        NodeRouting::Direct {
                            ip,
                            ports: std::collections::BTreeMap::new(),
                        }
                    } else {
                        NodeRouting::Routers(k.routers.clone())
                    },
                };

                (k, our)
            }
            Err(_) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&"Incorrect password!".to_string()),
                    StatusCode::UNAUTHORIZED,
                )
                .into_response())
            }
        };

    // Process cache sources and Base L2 access providers just like handle_boot
    let cache_source_vector = if let Some(custom_cache_sources) = &info.custom_cache_sources {
        println!(
            "Custom cache sources specified: {:?}\r",
            custom_cache_sources
        );
        custom_cache_sources.clone()
    } else {
        println!("No custom cache sources specified\r");
        Vec::new()
    };

    let base_l2_access_source_vector =
        if let Some(custom_base_l2_providers) = &info.custom_base_l2_access_providers {
            println!(
                "Custom Base L2 access providers specified: {:?}\r",
                custom_base_l2_providers
            );
            custom_base_l2_providers.clone()
        } else {
            println!("No custom Base L2 access providers specified\r");
            Vec::new()
        };

    if let Err(e) = assign_routing(
        &mut our,
        &providers,
        ws_networking_port,
        tcp_networking_port,
    )
    .await
    {
        return Ok(warp::reply::with_status(
            warp::reply::json(&e.to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response());
    }
    success_response(
        sender,
        our,
        decoded_keyfile,
        encoded_keyfile,
        cache_source_vector,
        base_l2_access_source_vector,
    )
    .await
}

pub async fn assign_routing(
    our: &mut Identity,
    providers: &[RootProvider<PubSubFrontend>],
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
) -> anyhow::Result<()> {
    let multicall = EthAddress::from_str(MULTICALL_ADDRESS)?;
    let hypermap = EthAddress::from_str(HYPERMAP_ADDRESS)?;

    let netkey_hash =
        FixedBytes::<32>::from_slice(&keygen::namehash(&format!("~net-key.{}", our.name)));
    let ws_hash =
        FixedBytes::<32>::from_slice(&keygen::namehash(&format!("~ws-port.{}", our.name)));
    let tcp_hash =
        FixedBytes::<32>::from_slice(&keygen::namehash(&format!("~tcp-port.{}", our.name)));
    let ip_hash = FixedBytes::<32>::from_slice(&keygen::namehash(&format!("~ip.{}", our.name)));

    let multicalls = vec![
        Call {
            target: hypermap,
            callData: Bytes::from(
                getCall {
                    namehash: netkey_hash,
                }
                .abi_encode(),
            ),
        },
        Call {
            target: hypermap,
            callData: Bytes::from(getCall { namehash: ws_hash }.abi_encode()),
        },
        Call {
            target: hypermap,
            callData: Bytes::from(getCall { namehash: tcp_hash }.abi_encode()),
        },
        Call {
            target: hypermap,
            callData: Bytes::from(getCall { namehash: ip_hash }.abi_encode()),
        },
    ];

    let multicall_call = aggregateCall { calls: multicalls }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(multicall_call));
    let tx = TransactionRequest::default().to(multicall).input(tx_input);

    let mut last_error = None;

    for (provider_index, provider) in providers.iter().enumerate() {
        let multicall_return = match provider.call(&tx).await {
            Ok(multicall_return) => multicall_return,
            Err(e) => {
                println!(
                    "Provider {} failed to fetch node IP data, trying next provider...\r",
                    provider_index
                );
                last_error = Some(anyhow::anyhow!(
                    "Failed to fetch node IP data from hypermap: {e}"
                ));
                continue;
            }
        };

        let Ok(results) = aggregateCall::abi_decode_returns(&multicall_return, false) else {
            println!(
                "Provider {} returned invalid multicall data, trying next provider...\r",
                provider_index
            );
            last_error = Some(anyhow::anyhow!("Failed to decode hypermap multicall data"));
            continue;
        };

        let Ok(netkey) = getCall::abi_decode_returns(&results.returnData[0], false) else {
            println!(
                "Provider {} returned invalid netkey data, trying next provider...\r",
                provider_index
            );
            last_error = Some(anyhow::anyhow!("Failed to decode netkey data"));
            continue;
        };

        // If all fields are zero/empty, the provider likely has stale data.
        // Try the next provider.
        if is_hypermap_entry_empty(&netkey) {
            println!(
                "Provider {} returned empty netkey entry, trying next provider...\r",
                provider_index
            );
            last_error = Some(anyhow::anyhow!(
                "Provider returned empty hypermap entry (stale data)"
            ));
            continue;
        }

        if netkey.data.to_string() != our.networking_key {
            return Err(anyhow::anyhow!(
                "Networking key from PKI ({}) does not match our saved networking key ({})",
                netkey.data.to_string(),
                our.networking_key
            ));
        }

        // Successfully validated netkey, now decode the rest
        let ws = getCall::abi_decode_returns(&results.returnData[1], false)?;
        let tcp = getCall::abi_decode_returns(&results.returnData[2], false)?;
        let ip = getCall::abi_decode_returns(&results.returnData[3], false)?;

        let ip = keygen::bytes_to_ip(&ip.data);
        let ws = keygen::bytes_to_port(&ws.data);
        let tcp = keygen::bytes_to_port(&tcp.data);

        if !our.is_direct() {
            // indirect node
            return Ok(());
        }

        if ip.is_ok() && (ws.is_ok() || tcp.is_ok()) {
            // direct node
            let mut ports = std::collections::BTreeMap::new();
            if let Ok(ws) = ws {
                if ws_networking_port.1 && ws != ws_networking_port.0 {
                    return Err(anyhow::anyhow!(
                        "Binary used --ws-port flag to set port to {}, but node is using port {} onchain.",
                        ws_networking_port.0,
                        ws
                    ));
                }
                ports.insert("ws".to_string(), ws);
            }
            if let Ok(tcp) = tcp {
                if tcp_networking_port.1 && tcp != tcp_networking_port.0 {
                    return Err(anyhow::anyhow!(
                        "Binary used --tcp-port flag to set port to {}, but node is using port {} onchain.",
                        tcp_networking_port.0,
                        tcp
                    ));
                }
                ports.insert("tcp".to_string(), tcp);
            }
            our.routing = NodeRouting::Direct {
                ip: ip.unwrap().to_string(),
                ports,
            };
        }
        // Success - return early
        return Ok(());
    }

    // All providers failed
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No providers available")))
}

async fn success_response(
    sender: Arc<RegistrationSender>,
    our: Identity,
    decoded_keyfile: Keyfile,
    encoded_keyfile: Vec<u8>,
    cache_source_vector: Vec<String>,
    base_l2_access_source_vector: Vec<String>,
) -> Result<warp::reply::Response, Rejection> {
    let encoded_keyfile_str = base64_standard.encode(&encoded_keyfile);
    let token = match keygen::generate_jwt(&decoded_keyfile.jwt_secret_bytes, &our.name, &None) {
        Some(token) => token,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Failed to generate JWT".to_string()),
                StatusCode::SERVICE_UNAVAILABLE,
            )
            .into_response())
        }
    };

    sender
        .send((
            our.clone(),
            decoded_keyfile,
            encoded_keyfile,
            cache_source_vector,
            base_l2_access_source_vector,
        ))
        .await
        .unwrap();

    let mut response =
        warp::reply::with_status(warp::reply::json(&encoded_keyfile_str), StatusCode::FOUND)
            .into_response();

    match HeaderValue::from_str(&format!("hyperware-auth_{}={token};", our.name)) {
        Ok(v) => {
            response.headers_mut().append(SET_COOKIE, v);
            response
                .headers_mut()
                .append("HttpOnly", HeaderValue::from_static("true"));
            response
                .headers_mut()
                .append("Secure", HeaderValue::from_static("true"));
            response
                .headers_mut()
                .append("SameSite", HeaderValue::from_static("Strict"));
        }
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Failed to generate Auth JWT".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response())
        }
    }

    Ok(response)
}
