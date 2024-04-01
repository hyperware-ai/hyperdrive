use crate::keygen;
use aes_gcm::aead::KeyInit;
use alloy_primitives::{Address as EthAddress, Bytes, FixedBytes, U256};
use alloy_providers::provider::{Provider, TempProvider};
use alloy_pubsub::PubSubFrontend;
use alloy_rpc_client::ClientBuilder;
use alloy_rpc_types::request::{TransactionInput, TransactionRequest};
use alloy_signer::Signature;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use alloy_transport_ws::WsConnect;
use hmac::Hmac;
use jwt::SignWithKey;
use lib::types::core::*;
use ring::rand::SystemRandom;
use ring::signature;
use ring::signature::KeyPair;
use sha2::Sha256;
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

pub const KNS_SEPOLIA_ADDRESS: EthAddress = EthAddress::new([
    0x38, 0x07, 0xFB, 0xD6, 0x92, 0xAa, 0x5c, 0x96, 0xF1, 0xD8, 0xD7, 0xc5, 0x9a, 0x13, 0x46, 0xa8,
    0x85, 0xF4, 0x0B, 0x1C,
]);

pub const KNS_OPTIMISM_ADDRESS: EthAddress = EthAddress::new([
    0xca, 0x5b, 0x58, 0x11, 0xc0, 0xC4, 0x0a, 0xAB, 0x32, 0x95, 0xf9, 0x32, 0xb1, 0xB5, 0x11, 0x2E,
    0xb7, 0xbb, 0x4b, 0xD6,
]);

sol! {
    function auth(
        bytes32 _node,
        address _sender
    ) public view virtual returns (bool authed);

    function nodes(bytes32) external view returns (address, uint96);

    function ip(bytes32) external view returns (uint128, uint16, uint16, uint16, uint16);
}

pub fn _ip_to_number(ip: &str) -> Result<u32, &'static str> {
    let octets: Vec<&str> = ip.split('.').collect();

    if octets.len() != 4 {
        return Err("Invalid IP address");
    }

    let mut ip_num: u32 = 0;
    for &octet in octets.iter() {
        ip_num <<= 8;
        match octet.parse::<u32>() {
            Ok(num) => {
                if num > 255 {
                    return Err("Invalid octet in IP address");
                }
                ip_num += num;
            }
            Err(_) => return Err("Invalid number in IP address"),
        }
    }

    Ok(ip_num)
}

fn _hex_string_to_u8_array(hex_str: &str) -> Result<[u8; 32], &'static str> {
    if !hex_str.starts_with("0x") || hex_str.len() != 66 {
        // "0x" + 64 hex chars
        return Err("Invalid hex format or length");
    }

    let no_prefix = &hex_str[2..];
    let mut bytes = [0_u8; 32];
    for (i, byte) in no_prefix.as_bytes().chunks(2).enumerate() {
        let hex_byte = std::str::from_utf8(byte)
            .map_err(|_| "Invalid UTF-8 sequence")?
            .parse::<u8>()
            .map_err(|_| "Failed to parse hex byte")?;
        bytes[i] = hex_byte;
    }

    Ok(bytes)
}

pub fn generate_jwt(jwt_secret_bytes: &[u8], username: &str) -> Option<String> {
    let jwt_secret: Hmac<Sha256> = match Hmac::new_from_slice(jwt_secret_bytes) {
        Ok(secret) => secret,
        Err(_) => return None,
    };

    let claims = crate::http::server_types::JwtClaims {
        username: username.to_string(),
        expiration: 0,
    };

    match claims.sign_with_key(&jwt_secret) {
        Ok(token) => Some(token),
        Err(_) => None,
    }
}

/// Serve the registration page and receive POSTs and PUTs from it
pub async fn register(
    tx: RegistrationSender,
    kill_rx: oneshot::Receiver<bool>,
    ip: String,
    ws_networking_port: Option<u16>,
    http_port: u16,
    keyfile: Option<Vec<u8>>,
    testnet: bool,
) {
    // Networking info is generated and passed to the UI, but not used until confirmed
    let (public_key, serialized_networking_keypair) = keygen::generate_networking_key();
    let net_keypair = Arc::new(serialized_networking_keypair.as_ref().to_vec());
    let tx = Arc::new(tx);

    let (ws_port, flag_used) = if let Some(port) = ws_networking_port {
        (
            crate::http::utils::find_open_port(port, port + 1)
                .await
                .expect(&format!(
                    "error: couldn't bind {}; first available port found was {}. \
                Set an available port with `--ws-port` and try again.",
                    port,
                    crate::http::utils::find_open_port(port, port + 1000)
                        .await
                        .expect("no ports found in range"),
                )),
            true,
        )
    } else {
        (crate::http::utils::find_open_port(9000, 65535)
            .await
            .expect(
            "Unable to find free port between 9000 and 65535 for a new websocket, are you kidding?",
        ), false)
    };

    // This is a temporary identity, passed to the UI. If it is confirmed through a /boot or /confirm-change-network-keys, then it will be used to replace the current identity
    let our_temp_id = Arc::new(Identity {
        networking_key: format!("0x{}", public_key),
        name: "".to_string(),
        ws_routing: Some((ip.clone(), ws_port)),
        allowed_routers: vec![
            // "next-release-router.os".into(),
            "default-router-1.os".into(),
            "default-router-2.os".into(),
            "default-router-3.os".into(),
        ],
    });

    // KnsRegistrar contract address
    let kns_address = if testnet {
        KNS_SEPOLIA_ADDRESS
    } else {
        KNS_OPTIMISM_ADDRESS
    };

    // This ETH provider uses public rpc endpoints to verify registration signatures.
    let url = if testnet {
        "wss://ethereum-sepolia-rpc.publicnode.com".to_string()
    } else {
        "wss://optimism-rpc.publicnode.com".to_string()
    };
    let connector = WsConnect { url, auth: None };
    let client = ClientBuilder::default().ws(connector).await.unwrap();

    let provider = Arc::new(Provider::new_with_client(client));

    let keyfile = warp::any().map(move || keyfile.clone());
    let our_temp_id = warp::any().map(move || our_temp_id.clone());
    let net_keypair = warp::any().map(move || net_keypair.clone());
    let tx = warp::any().map(move || tx.clone());
    let ip = warp::any().map(move || ip.clone());

    let static_files = warp::path("static").and(static_dir!("src/register-ui/build/static/"));

    let react_app = warp::path::end()
        .and(warp::get())
        .map(move || warp::reply::html(include_str!("register-ui/build/index.html")))
        .or(warp::path("login")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("register-name")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("claim-invite")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("reset")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("import-keyfile")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("set-password")
            .and(warp::get())
            .map(move || warp::reply::html(include_str!("register-ui/build/index.html"))))
        .or(warp::path("current-chain").and(warp::get()).map(move || {
            if testnet {
                warp::reply::json(&"0xaa36a7")
            } else {
                warp::reply::json(&"0xa")
            }
        }))
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
                        return warp::reply::json(&username);
                    }
                }
                warp::reply::json(&"")
            },
        ));

    let boot_provider = provider.clone();
    let login_provider = provider.clone();
    let import_provider = provider.clone();

    let api = warp::path("info")
        .and(
            warp::get()
                .and(keyfile.clone())
                .and_then(get_unencrypted_info),
        )
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
                        testnet,
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
                .and(warp::any().map(move || if flag_used { Some(ws_port) } else { None }))
                .and(tx.clone())
                .and_then(move |boot_info, ip, ws_port, tx| {
                    let import_provider = import_provider.clone();
                    handle_import_keyfile(boot_info, ip, ws_port, tx, kns_address, import_provider)
                }),
        ))
        .or(warp::path("login").and(
            warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(ip)
                .and(warp::any().map(move || if flag_used { Some(ws_port) } else { None }))
                .and(tx.clone())
                .and(keyfile.clone())
                .and_then(move |boot_info, ip, ws_port, tx, keyfile| {
                    let login_provider = login_provider.clone();
                    handle_login(
                        boot_info,
                        ip,
                        ws_port,
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
    // .or(
    //     warp::path("our").and(warp::get().and(keyfile.clone()).and_then(move |keyfile| {
    //         if let Some(keyfile) = keyfile {
    //             if let Ok((username, _, _, _, _, _)) = bincode::deserialize::<(
    //                 String,
    //                 Vec<String>,
    //                 Vec<u8>,
    //                 Vec<u8>,
    //                 Vec<u8>,
    //                 Vec<u8>,
    //             )>(&keyfile)
    //             {
    //                 return Ok(warp::reply::with_status(
    //                     warp::reply::json(&username),
    //                     StatusCode::OK,
    //                 )
    //                 .into_response());
    //             }
    //         }
    //         Ok(
    //             warp::reply::with_status(warp::reply::json(&""), StatusCode::OK)
    //                 .into_response(),
    //         )
    //     })),
    // );

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
        false => base64::decode(payload.keyfile).map_err(|_| warp::reject())?,
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
    testnet: bool,
    kns_address: EthAddress,
    provider: Arc<Provider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    let mut our = our.as_ref().clone();

    our.name = info.username;
    if info.direct {
        our.allowed_routers = vec![];
    } else {
        our.ws_routing = None;
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
    let tx = TransactionRequest {
        to: Some(kns_address),
        input: tx_input,
        ..Default::default()
    };

    // this call can fail if the indexer has not caught up to the transaction
    // that just got confirmed on our frontend. for this reason, we retry
    // the call a few times before giving up.

    let mut attempts = 0;
    let mut tld_result = Err(());
    while attempts < 5 {
        match provider.call(tx.clone(), None).await {
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
    let tx = TransactionRequest {
        to: Some(tld_address),
        input: tx_input,
        ..Default::default()
    };

    let Ok(authed) = provider.call(tx, None).await else {
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

    let chain_id: u64 = if testnet { 11155111 } else { 10 };

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
        routers: our.allowed_routers.clone(),
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
    ws_networking_port: Option<u16>,
    sender: Arc<RegistrationSender>,
    kns_address: EthAddress,
    provider: Arc<Provider<PubSubFrontend>>,
) -> Result<impl Reply, Rejection> {
    // if keyfile was not present in node and is present from user upload
    let encoded_keyfile = match base64::decode(info.keyfile.clone()) {
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
                    ws_routing: if k.routers.is_empty() {
                        Some((ip, 9000))
                    } else {
                        None
                    },
                    allowed_routers: k.routers.clone(),
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

    if let Err(e) = assign_ws_routing(&mut our, kns_address, provider, ws_networking_port).await {
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
    ws_networking_port: Option<u16>,
    sender: Arc<RegistrationSender>,
    encoded_keyfile: Option<Vec<u8>>,
    kns_address: EthAddress,
    provider: Arc<Provider<PubSubFrontend>>,
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
                    ws_routing: if k.routers.is_empty() {
                        Some((ip, 9000))
                    } else {
                        None
                    },
                    allowed_routers: k.routers.clone(),
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

    if let Err(e) = assign_ws_routing(&mut our, kns_address, provider, ws_networking_port).await {
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
        our.allowed_routers = vec![];
    } else {
        our.ws_routing = None;
    }

    let decoded_keyfile = Keyfile {
        username: our.name.clone(),
        routers: our.allowed_routers.clone(),
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

    success_response(sender, our.clone(), decoded_keyfile, encoded_keyfile).await
}

async fn assign_ws_routing(
    our: &mut Identity,
    kns_address: EthAddress,
    provider: Arc<Provider<PubSubFrontend>>,
    ws_networking_port: Option<u16>,
) -> anyhow::Result<()> {
    let namehash = FixedBytes::<32>::from_slice(&keygen::namehash(&our.name));
    let ip_call = ipCall { _0: namehash }.abi_encode();
    let tx_input = TransactionInput::new(Bytes::from(ip_call));
    let tx = TransactionRequest {
        to: Some(kns_address),
        input: tx_input,
        ..Default::default()
    };

    let Ok(ip_data) = provider.call(tx, None).await else {
        return Err(anyhow::anyhow!("Failed to fetch node IP data from PKI"));
    };

    let Ok((ip, ws, _wt, _tcp, _udp)) = <(u128, u16, u16, u16, u16)>::abi_decode(&ip_data, false)
    else {
        return Err(anyhow::anyhow!("Failed to decode node IP data from PKI"));
    };

    let node_ip = format!(
        "{}.{}.{}.{}",
        (ip >> 24) & 0xFF,
        (ip >> 16) & 0xFF,
        (ip >> 8) & 0xFF,
        ip & 0xFF
    );

    if node_ip != *"0.0.0.0" || ws != 0 {
        // direct node
        if let Some(chosen_port) = ws_networking_port {
            if chosen_port != ws {
                return Err(anyhow::anyhow!(
                    "Binary used --ws-port flag to set port to {}, but node is using port {} onchain.",
                    chosen_port,
                    ws
                ));
            }
        }
        our.ws_routing = Some((node_ip, ws));
    }
    Ok(())
}

async fn success_response(
    sender: Arc<RegistrationSender>,
    our: Identity,
    decoded_keyfile: Keyfile,
    encoded_keyfile: Vec<u8>,
) -> Result<warp::reply::Response, Rejection> {
    let encoded_keyfile_str = base64::encode(&encoded_keyfile);
    let token = match generate_jwt(&decoded_keyfile.jwt_secret_bytes, &our.name) {
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

    let headers = response.headers_mut();

    match HeaderValue::from_str(&format!("kinode-auth_{}={};", &our.name, &token)) {
        Ok(v) => {
            headers.append(SET_COOKIE, v);
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
