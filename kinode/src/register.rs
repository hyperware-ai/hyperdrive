use crate::keygen;
use crate::KNS_ADDRESS;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy::signers::Signature;
use alloy_primitives::{Address as EthAddress, Bytes, FixedBytes, U256};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use base64::{engine::general_purpose::STANDARD as base64_standard, Engine};
use lib::types::core::*;
use ring::rand::SystemRandom;
use ring::signature;
use ring::signature::KeyPair;
use static_dir::static_dir;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use warp::{
    http::{
        header::{HeaderMap, HeaderValue, SET_COOKIE},
        StatusCode,
    },
    Filter, Rejection, Reply,
};

type RegistrationSender = mpsc::Sender<(Identity, Keyfile, Vec<u8>)>;

sol! {
    function auth(
        bytes32 _node,
        address _sender
    ) public view virtual returns (bool authed);
    function key(bytes32) external view returns (bytes32);
    function nodes(bytes32) external view returns (address, uint96);
    function ip(bytes32) external view returns (uint128, uint16, uint16, uint16, uint16);
    function routers(bytes32) external view returns (bytes32[]);
}

/// Serve the registration page and receive POSTs and PUTs from it
pub async fn register(
    tx: RegistrationSender,
    kill_rx: oneshot::Receiver<bool>,
    ip: String,
    ws_networking: (Option<&tokio::net::TcpListener>, bool),
    tcp_networking: (Option<&tokio::net::TcpListener>, bool),
    http_port: u16,
    keyfile: Option<Vec<u8>>,
    maybe_rpc: Option<String>,
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
    // If it is confirmed through a /boot or /confirm-change-network-keys,
    // then it will be used to replace the current identity.
    let our_temp_id = Arc::new(Identity {
        networking_key: format!("0x{}", public_key),
        name: "".to_string(),
        routing: NodeRouting::Both {
            ip: ip.clone(),
            ports: ports_map,
            routers: vec![
                "default-router-1.os".into(),
                "default-router-2.os".into(),
                "default-router-3.os".into(),
            ],
        },
    });

    // KnsRegistrar contract address
    let kns_address = EthAddress::from_str(KNS_ADDRESS).unwrap();

    let provider = Arc::new(connect_to_provider(maybe_rpc).await);

    let keyfile = warp::any().map(move || keyfile.clone());
    let our_temp_id = warp::any().map(move || our_temp_id.clone());
    let net_keypair = warp::any().map(move || net_keypair.clone());
    let tx = warp::any().map(move || tx.clone());
    let ip = warp::any().map(move || ip.clone());
    let ws_port = warp::any().map(move || (ws_port, ws_flag_used));
    let tcp_port = warp::any().map(move || (tcp_port, tcp_flag_used));

    let static_files = warp::path("assets").and(static_dir!("src/register-ui/build/assets/"));

    let react_app = warp::path::end()
        .or(warp::path("login"))
        .or(warp::path("register-name"))
        .or(warp::path("claim-invite"))
        .or(warp::path("reset"))
        .or(warp::path("import-keyfile"))
        .or(warp::path("set-password"))
        .and(warp::get())
        .map(move |_| warp::reply::html(include_str!("register-ui/build/index.html")));

    let boot_provider = provider.clone();
    let login_provider = provider.clone();
    let import_provider = provider.clone();

    let api = warp::path("info")
        .and(
            warp::get()
                .and(keyfile.clone())
                .and_then(get_unencrypted_info),
        )
        .or(warp::path("current-chain")
            .and(warp::get())
            .map(move || warp::reply::json(&"0xa")))
        .or(warp::path("our").and(warp::get()).and(keyfile.clone()).map(
            move |keyfile: Option<Vec<u8>>| {
                if let Some(keyfile) = keyfile {
                    if let Ok((username, _, _, _, _, _)) = bincode::deserialize::<(
                        String,
                        Vec<String>,
                        Vec<u8>,
                        Vec<u8>,
                        Vec<u8>,
                        Vec<u8>,
                    )>(keyfile.as_ref())
                    {
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
        .or(warp::path("vet-keyfile").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(keyfile.clone())
                .and_then(handle_keyfile_vet),
        ))
        .or(warp::path("boot").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(tx.clone())
                .and(our_temp_id.clone())
                .and(net_keypair.clone())
                .and_then(move |boot_info, tx, our_temp_id, net_keypair| {
                    let boot_provider = boot_provider.clone();
                    handle_boot(
                        boot_info,
                        tx,
                        our_temp_id,
                        net_keypair,
                        kns_address,
                        boot_provider,
                    )
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
                    let import_provider = import_provider.clone();
                    handle_import_keyfile(
                        boot_info,
                        ip,
                        ws_port,
                        tcp_port,
                        tx,
                        kns_address,
                        import_provider,
                    )
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
                    let login_provider = login_provider.clone();
                    handle_login(
                        boot_info,
                        ip,
                        ws_port,
                        tcp_port,
                        tx,
                        keyfile,
                        kns_address,
                        login_provider,
                    )
                }),
        ))
        .or(warp::path("confirm-change-network-keys").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(tx)
                .and(our_temp_id)
                .and(net_keypair)
                .and(keyfile)
                .and_then(confirm_change_network_keys),
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

    let _ = open::that(format!("http://localhost:{}/", http_port));
    warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], http_port), async {
            kill_rx.await.ok();
        })
        .1
        .await;
}

pub async fn connect_to_provider(maybe_rpc: Option<String>) -> RootProvider<PubSubFrontend> {
    // This ETH provider uses public rpc endpoints to verify registration signatures.
    let url = if let Some(rpc_url) = maybe_rpc {
        rpc_url
    } else {
        "wss://optimism-rpc.publicnode.com".to_string()
    };
    println!(
        "Connecting to Optimism RPC at {url}\n\
        Specify a different RPC URL with the --rpc flag."
    );
    let ws = WsConnect::new(url);
    // this fails occasionally in certain networking environments. i'm not sure why.
    // frustratingly, the exact same call does not fail in the eth module. more investigation needed.
    let Ok(client) = ProviderBuilder::new().on_ws(ws).await else {
        panic!(
            "Error: runtime could not connect to ETH RPC.\n\
            This is necessary in order to verify node identity onchain.\n\
            Please make sure you are using a valid WebSockets URL if using \
            the --rpc flag, and you are connected to the internet."
        );
    };
    println!("Connected to Optimism RPC");

    client
}

async fn get_unencrypted_info(keyfile: Option<Vec<u8>>) -> Result<impl Reply, Rejection> {
    let (name, allowed_routers) = {
        match keyfile {
            Some(encoded_keyfile) => match keygen::get_username_and_routers(&encoded_keyfile) {
                Ok(k) => k,
                Err(_) => {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&"keyfile deserialization went wrong"),
                        StatusCode::UNAUTHORIZED,
                    )
                    .into_response())
                }
            },
            None => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&"Keyfile not present"),
                    StatusCode::NOT_FOUND,
                )
                .into_response())
            }
        }
    };
    return Ok(warp::reply::with_status(
        warp::reply::json(&UnencryptedIdentity {
            name,
            allowed_routers,
        }),
        StatusCode::OK,
    )
    .into_response());
}

async fn generate_networking_info(our_temp_id: Arc<Identity>) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(our_temp_id.as_ref()))
}

async fn handle_keyfile_vet(
    payload: KeyfileVet,
    keyfile: Option<Vec<u8>>,
) -> Result<impl Reply, Rejection> {
    // additional checks?
    let encoded_keyfile = match payload.keyfile.is_empty() {
        true => keyfile.ok_or(warp::reject())?,
        false => base64_standard
            .decode(payload.keyfile)
            .map_err(|_| warp::reject())?,
    };

    let decoded_keyfile = keygen::decode_keyfile(&encoded_keyfile, &payload.password_hash)
        .map_err(|_| warp::reject())?;

    Ok(warp::reply::json(&KeyfileVetted {
        username: decoded_keyfile.username,
        networking_key: format!(
            "0x{}",
            hex::encode(decoded_keyfile.networking_keypair.public_key().as_ref())
        ),
        routers: decoded_keyfile.routers,
    }))
}

async fn handle_boot(
    info: BootInfo,
    sender: Arc<RegistrationSender>,
    our: Arc<Identity>,
    networking_keypair: Arc<Vec<u8>>,
    kns_address: EthAddress,
    provider: Arc<RootProvider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    let mut our = our.as_ref().clone();

    our.name = info.username;
    if info.direct {
        our.both_to_direct();
    } else {
        our.both_to_routers();
    }
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
            warp::reply::json(&"Timestamp is outdated."),
            StatusCode::UNAUTHORIZED,
        )
        .into_response());
    }

    let namehash = FixedBytes::<32>::from_slice(&keygen::namehash(&our.name));

    let tld_call = nodesCall { _0: namehash }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(tld_call));

    let tx = TransactionRequest::default()
        .to(kns_address)
        .input(tx_input);

    // this call can fail if the indexer has not caught up to the transaction
    // that just got confirmed on our frontend. for this reason, we retry
    // the call a few times before giving up.

    let mut attempts = 0;
    let mut tld_result = Err(());
    while attempts < 5 {
        match provider.call(&tx).await {
            Ok(tld) => {
                tld_result = Ok(tld);
                break;
            }
            Err(_) => {
                attempts += 1;
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
    let Ok(tld) = tld_result else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Failed to fetch TLD contract for username"),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response());
    };

    let Ok((tld_address, _)) = <(EthAddress, U256)>::abi_decode(&tld, false) else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Failed to decode TLD contract from return bytes"),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response());
    };
    let owner = EthAddress::from_str(&info.owner).map_err(|_| warp::reject())?;

    let auth_call = authCall {
        _node: namehash,
        _sender: owner,
    }
    .abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(auth_call));
    let tx = TransactionRequest::default()
        .to(tld_address)
        .input(tx_input);

    let Ok(authed) = provider.call(&tx).await else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Failed to fetch associated address for username"),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response());
    };

    let is_ok = bool::abi_decode(&authed, false).unwrap_or(false);

    if !is_ok {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Address is not authorized for username!"),
            StatusCode::UNAUTHORIZED,
        )
        .into_response());
    };

    let chain_id: u64 = 10;

    // manual json creation to preserve order..
    let sig_data_json = format!(
        r#"{{"username":"{}","password_hash":"{}","timestamp":{},"direct":{},"reset":{},"chain_id":{}}}"#,
        our.name, info.password_hash, info.timestamp, info.direct, info.reset, chain_id
    );
    let sig_data = sig_data_json.as_bytes();

    let sig = Signature::from_str(&info.signature).map_err(|_| warp::reject())?;

    let recovered_address = sig
        .recover_address_from_msg(sig_data)
        .map_err(|_| warp::reject())?;

    if recovered_address != owner {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Recovered address does not match owner"),
            StatusCode::UNAUTHORIZED,
        )
        .into_response());
    }

    let decoded_keyfile = Keyfile {
        username: our.name.clone(),
        routers: our.routers().unwrap_or(&vec![]).clone(),
        networking_keypair: signature::Ed25519KeyPair::from_pkcs8(networking_keypair.as_ref())
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

    success_response(sender, our, decoded_keyfile, encoded_keyfile).await
}

async fn handle_import_keyfile(
    info: ImportKeyfileInfo,
    ip: String,
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
    sender: Arc<RegistrationSender>,
    kns_address: EthAddress,
    provider: Arc<RootProvider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    // if keyfile was not present in node and is present from user upload
    let encoded_keyfile = match base64_standard.decode(info.keyfile.clone()) {
        Ok(k) => k,
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Keyfile not valid base64"),
                StatusCode::BAD_REQUEST,
            )
            .into_response())
        }
    };

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
                    warp::reply::json(&"Incorrect password_hash".to_string()),
                    StatusCode::UNAUTHORIZED,
                )
                .into_response())
            }
        };

    if let Err(e) = assign_routing(
        &mut our,
        kns_address,
        provider,
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
    success_response(sender, our, decoded_keyfile, encoded_keyfile).await
}

async fn handle_login(
    info: LoginInfo,
    ip: String,
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
    sender: Arc<RegistrationSender>,
    encoded_keyfile: Option<Vec<u8>>,
    kns_address: EthAddress,
    provider: Arc<RootProvider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    if encoded_keyfile.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Keyfile not present"),
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
                    warp::reply::json(&"Incorrect password_hash"),
                    StatusCode::UNAUTHORIZED,
                )
                .into_response())
            }
        };

    if let Err(e) = assign_routing(
        &mut our,
        kns_address,
        provider,
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
    success_response(sender, our, decoded_keyfile, encoded_keyfile).await
}

async fn confirm_change_network_keys(
    info: LoginAndResetInfo,
    sender: Arc<RegistrationSender>,
    our: Arc<Identity>,
    networking_keypair: Arc<Vec<u8>>,
    encoded_keyfile: Option<Vec<u8>>,
) -> Result<impl Reply, Rejection> {
    if encoded_keyfile.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&"Keyfile not present"),
            StatusCode::NOT_FOUND,
        )
        .into_response());
    }
    let encoded_keyfile = encoded_keyfile.unwrap();
    let mut our = our.as_ref().clone();

    // Get our name from our current keyfile
    let old_decoded_keyfile = match keygen::decode_keyfile(&encoded_keyfile, &info.password_hash) {
        Ok(k) => {
            our.name = k.username.clone();
            k
        }
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Invalid password"),
                StatusCode::UNAUTHORIZED,
            )
            .into_response());
        }
    };

    // Determine if direct node or not

    if info.direct {
        our.both_to_direct();
    } else {
        our.both_to_routers();
    }

    let decoded_keyfile = Keyfile {
        username: our.name.clone(),
        routers: our.routers().unwrap_or(&vec![]).clone(),
        networking_keypair: signature::Ed25519KeyPair::from_pkcs8(networking_keypair.as_ref())
            .unwrap(),
        jwt_secret_bytes: old_decoded_keyfile.jwt_secret_bytes,
        file_key: old_decoded_keyfile.file_key,
    };

    let encoded_keyfile = keygen::encode_keyfile(
        info.password_hash,
        decoded_keyfile.username.clone(),
        decoded_keyfile.routers.clone(),
        &networking_keypair,
        &decoded_keyfile.jwt_secret_bytes,
        &decoded_keyfile.file_key,
    );

    our.networking_key = format!(
        "0x{}",
        hex::encode(decoded_keyfile.networking_keypair.public_key().as_ref())
    );

    success_response(sender, our, decoded_keyfile, encoded_keyfile).await
}

pub async fn assign_routing(
    our: &mut Identity,
    kns_address: EthAddress,
    provider: Arc<RootProvider<PubSubFrontend>>,
    ws_networking_port: (u16, bool),
    tcp_networking_port: (u16, bool),
) -> anyhow::Result<()> {
    let namehash = FixedBytes::<32>::from_slice(&keygen::namehash(&our.name));
    let ip_call = ipCall { _0: namehash }.abi_encode();
    let key_call = keyCall { _0: namehash }.abi_encode();
    let router_call = routersCall { _0: namehash }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(ip_call));
    let tx = TransactionRequest::default()
        .to(kns_address)
        .input(tx_input);

    let Ok(ip_data) = provider.call(&tx).await else {
        return Err(anyhow::anyhow!("Failed to fetch node IP data from PKI"));
    };

    let Ok((ip, ws, _wt, tcp, _udp)) = <(u128, u16, u16, u16, u16)>::abi_decode(&ip_data, false)
    else {
        return Err(anyhow::anyhow!("Failed to decode node IP data from PKI"));
    };

    let key_tx_input = TransactionInput::new(Bytes::from(key_call));
    let key_tx = TransactionRequest::default()
        .to(kns_address)
        .input(key_tx_input);

    let Ok(public_key) = provider.call(&key_tx).await else {
        return Err(anyhow::anyhow!("Failed to fetch node key from PKI"));
    };

    if format!("0x{}", hex::encode(&public_key)) != our.networking_key {
        return Err(anyhow::anyhow!(
            "Networking key from PKI does not match our saved networking key"
        ));
    }

    let router_tx_input = TransactionInput::new(Bytes::from(router_call));
    let router_tx = TransactionRequest::default()
        .to(kns_address)
        .input(router_tx_input);

    let Ok(routers) = provider.call(&router_tx).await else {
        return Err(anyhow::anyhow!("Failed to fetch node routers from PKI"));
    };
    let Ok(routers) = <Vec<FixedBytes<32>>>::abi_decode(&routers, false) else {
        return Err(anyhow::anyhow!("Failed to decode node routers from PKI"));
    };

    let node_ip = format!(
        "{}.{}.{}.{}",
        (ip >> 24) & 0xFF,
        (ip >> 16) & 0xFF,
        (ip >> 8) & 0xFF,
        ip & 0xFF
    );

    if !routers.is_empty() {
        // indirect node
        return Ok(());
    }
    if node_ip != *"0.0.0.0" && (ws != 0 || tcp != 0) {
        // direct node
        let mut ports = std::collections::BTreeMap::new();
        if ws != 0 {
            if ws_networking_port.1 && ws != ws_networking_port.0 {
                return Err(anyhow::anyhow!(
                    "Binary used --ws-port flag to set port to {}, but node is using port {} onchain.",
                    ws_networking_port.0,
                    ws
                ));
            }
            ports.insert("ws".to_string(), ws);
        }
        if tcp != 0 {
            if tcp_networking_port.1 && tcp != tcp_networking_port.0 {
                return Err(anyhow::anyhow!(
                    "Binary used --tcp-port flag to set port to {}, but node is using port {} onchain.",
                    tcp_networking_port.0,
                    tcp
                ));
            }
            ports.insert("tcp".to_string(), tcp);
        }
        our.routing = NodeRouting::Direct { ip: node_ip, ports };
    }
    Ok(())
}
async fn success_response(
    sender: Arc<RegistrationSender>,
    our: Identity,
    decoded_keyfile: Keyfile,
    encoded_keyfile: Vec<u8>,
) -> Result<warp::reply::Response, Rejection> {
    let encoded_keyfile_str = base64_standard.encode(&encoded_keyfile);
    let token = match keygen::generate_jwt(&decoded_keyfile.jwt_secret_bytes, &our.name, &None) {
        Some(token) => token,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Failed to generate JWT"),
                StatusCode::SERVICE_UNAVAILABLE,
            )
            .into_response())
        }
    };

    sender
        .send((our.clone(), decoded_keyfile, encoded_keyfile))
        .await
        .unwrap();

    let mut response =
        warp::reply::with_status(warp::reply::json(&encoded_keyfile_str), StatusCode::FOUND)
            .into_response();

    match HeaderValue::from_str(&format!("kinode-auth_{}={token};", our.name)) {
        Ok(v) => {
            response.headers_mut().append(SET_COOKIE, v);
        }
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&"Failed to generate Auth JWT"),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response())
        }
    }

    Ok(response)
}
