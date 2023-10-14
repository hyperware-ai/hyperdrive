use aes_gcm::aead::KeyInit;
use hmac::Hmac;
use jwt::SignWithKey;
use ring::pkcs8::Document;
use ring::rand::SystemRandom;
use ring::signature;
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use warp::{
    http::header::{HeaderValue, SET_COOKIE},
    Filter, Rejection, Reply,
};

use crate::http_server;
use crate::keygen;
use crate::types::*;

type RegistrationSender = mpsc::Sender<(Identity, String, Document, Vec<u8>)>;

pub fn generate_jwt(jwt_secret_bytes: &[u8], username: String) -> Option<String> {
    let jwt_secret: Hmac<Sha256> = match Hmac::new_from_slice(&jwt_secret_bytes) {
        Ok(secret) => secret,
        Err(_) => return None,
    };

    let claims = JwtClaims {
        username: username.clone(),
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
    port: u16,
    redir_port: u16,
) {
    let our = Arc::new(Mutex::new(None));
    let pw = Arc::new(Mutex::new(None));
    let networking_keypair = Arc::new(Mutex::new(None));

    let our_post = our.clone();
    let pw_post = pw.clone();
    let networking_keypair_post = networking_keypair.clone();

    let static_files = warp::path("static").and(warp::fs::dir("./src/register_app/static/"));
    let react_app = warp::path::end()
        .and(warp::get())
        .and(warp::fs::file("./src/register_app/index.html"));

    let api = warp::path("get-ws-info").and(
        // 1. Get uqname (already on chain) and return networking information
        warp::post()
            .and(warp::body::content_length_limit(1024 * 16))
            .and(warp::body::json())
            .and(warp::any().map(move || ip.clone()))
            .and(warp::any().map(move || our_post.clone()))
            .and(warp::any().map(move || pw_post.clone()))
            .and(warp::any().map(move || networking_keypair_post.clone()))
            .and_then(handle_post)
            // 2. trigger for finalizing registration once on-chain actions are done
            .or(warp::put()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::any().map(move || tx.clone()))
                .and(warp::any().map(move || our.lock().unwrap().take().unwrap()))
                .and(warp::any().map(move || pw.lock().unwrap().take().unwrap()))
                .and(warp::any().map(move || networking_keypair.lock().unwrap().take().unwrap()))
                .and(warp::any().map(move || redir_port))
                .and_then(handle_put)),
    );

    let routes = static_files.or(react_app).or(api);

    let _ = open::that(format!("http://localhost:{}/", port));
    warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], port), async {
            kill_rx.await.ok();
        })
        .1
        .await;
}

async fn handle_post(
    info: Registration,
    ip: String,
    our_post: Arc<Mutex<Option<Identity>>>,
    pw_post: Arc<Mutex<Option<String>>>,
    networking_keypair_post: Arc<Mutex<Option<Document>>>,
) -> Result<impl Reply, Rejection> {
    // 1. Generate networking keys
    let (public_key, serialized_networking_keypair) = keygen::generate_networking_key();
    *networking_keypair_post.lock().unwrap() = Some(serialized_networking_keypair);

    // 2. generate ws and routing information
    // TODO: if IP is localhost, assign a router...
    let ws_port = http_server::find_open_port(9000).await.unwrap();
    let our = Identity {
        name: info.username.clone(),
        networking_key: public_key,
        ws_routing: if ip == "localhost" || !info.direct {
            None
        } else {
            Some((ip.clone(), ws_port))
        },
        allowed_routers: if ip == "localhost" || !info.direct {
            vec![
                "uqbar-router-1.uq".into(), // "0x8d9e54427c50660c6d4802f63edca86a9ca5fd6a78070c4635950e9d149ed441".into(),
                "uqbar-router-2.uq".into(), // "0x06d331ed65843ecf0860c73292005d8103af20820546b2f8f9007d01f60595b1".into(),
                "uqbar-router-3.uq".into(), // "0xe6ab611eb62e8aee0460295667f8179cda4315982717db4b0b3da6022deecac1".into(),
            ]
        } else {
            vec![]
        },
    };
    *our_post.lock().unwrap() = Some(our.clone());
    *pw_post.lock().unwrap() = Some(info.password);
    // Return a response containing all networking information
    Ok(warp::reply::json(&our))
}

async fn handle_put(
    sender: RegistrationSender,
    our: Identity,
    pw: String,
    networking_keypair: Document,
    _redir_port: u16,
) -> Result<impl Reply, Rejection> {
    let seed = SystemRandom::new();
    let mut jwt_secret = [0u8; 32];
    ring::rand::SecureRandom::fill(&seed, &mut jwt_secret).unwrap();

    let token = match generate_jwt(&jwt_secret, our.name.clone()) {
        Some(token) => token,
        None => return Err(warp::reject()),
    };
    let cookie_value = format!("uqbar-auth_{}={};", &our.name, &token);
    let ws_cookie_value = format!("uqbar-ws-auth_{}={};", &our.name, &token);

    let mut response = warp::reply::html("Success".to_string()).into_response();

    let headers = response.headers_mut();
    headers.append(SET_COOKIE, HeaderValue::from_str(&cookie_value).unwrap());
    headers.append(SET_COOKIE, HeaderValue::from_str(&ws_cookie_value).unwrap());

    sender
        .send((our, pw, networking_keypair, jwt_secret.to_vec()))
        .await
        .unwrap();
    Ok(response)
}

/// Serve the login page, just get a password
pub async fn login(
    tx: mpsc::Sender<(
        String,
        Vec<String>,
        signature::Ed25519KeyPair,
        Vec<u8>,
        Vec<u8>,
    )>,
    kill_rx: oneshot::Receiver<bool>,
    keyfile: Vec<u8>,
    port: u16,
) {
    let login_page = include_str!("login.html");
    let routes = warp::path("login").and(
        // 1. serve login.html right here
        warp::get()
            .map(move || warp::reply::html(login_page))
            // 2. await a single POST
            //    - password
            .or(warp::post()
                .and(warp::body::content_length_limit(1024 * 16))
                .and(warp::body::json())
                .and(warp::any().map(move || keyfile.clone()))
                .and(warp::any().map(move || tx.clone()))
                .and_then(handle_password)),
    );

    let _ = open::that(format!("http://localhost:{}/login", port));
    warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], port), async {
            kill_rx.await.ok();
        })
        .1
        .await;
}

async fn handle_password(
    password: serde_json::Value,
    keyfile: Vec<u8>,
    tx: mpsc::Sender<(
        String,
        Vec<String>,
        signature::Ed25519KeyPair,
        Vec<u8>,
        Vec<u8>,
    )>,
) -> Result<impl Reply, Rejection> {
    let password = match password["password"].as_str() {
        Some(p) => p,
        None => return Err(warp::reject()),
    };
    // use password to decrypt networking keys
    let (username, routers, networking_keypair, jwt_secret_bytes, file_key) =
        keygen::decode_keyfile(keyfile, password);

    let token = match generate_jwt(&jwt_secret_bytes, username.clone()) {
        Some(token) => token,
        None => return Err(warp::reject()),
    };
    let cookie_value = format!("uqbar-auth_{}={};", &username, &token);
    let ws_cookie_value = format!("uqbar-ws-auth_{}={};", &username, &token);

    let mut response = warp::reply::html("Success".to_string()).into_response();

    let headers = response.headers_mut();
    headers.append(SET_COOKIE, HeaderValue::from_str(&cookie_value).unwrap());
    headers.append(SET_COOKIE, HeaderValue::from_str(&ws_cookie_value).unwrap());

    tx.send((
        username,
        routers,
        networking_keypair,
        jwt_secret_bytes.to_vec(),
        file_key.to_vec(),
    ))
    .await
    .unwrap();
    // TODO unhappy paths where key has changed / can't be decrypted
    Ok(response)
}
