use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key,
};
use alloy_primitives::keccak256;
use anyhow::Result;
use generic_array::GenericArray;
use hmac::Hmac;
use jwt::SignWithKey;
use lib::types::core::Keyfile;
use ring::pbkdf2;
use ring::pkcs8::Document;
use ring::rand::SystemRandom;
use ring::signature::{self, KeyPair};
use ring::{digest as ring_digest, rand::SecureRandom};
use sha2::Sha256;
use std::num::NonZeroU32;

type DiskKey = [u8; CREDENTIAL_LEN];

pub const CREDENTIAL_LEN: usize = ring_digest::SHA256_OUTPUT_LEN;
pub const ITERATIONS: u32 = 1_000_000;
pub static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256; // TODO maybe look into Argon2

pub fn encode_keyfile(
    password_hash: String,
    username: String,
    routers: Vec<String>,
    networking_key: &[u8],
    jwt: &[u8],
    file_key: &[u8],
) -> Vec<u8> {
    let mut disk_key: DiskKey = [0u8; CREDENTIAL_LEN];

    let rng = SystemRandom::new();
    let mut salt = [0u8; 32]; // generate a unique salt
    rng.fill(&mut salt).unwrap();

    pbkdf2::derive(
        PBKDF2_ALG,
        NonZeroU32::new(ITERATIONS).unwrap(),
        &salt,
        password_hash.as_bytes(),
        &mut disk_key,
    );

    let key = Key::<Aes256Gcm>::from_slice(&disk_key);
    let cipher = Aes256Gcm::new(key);

    let network_nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
    let jwt_nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let file_nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let keyciphertext: Vec<u8> = cipher.encrypt(&network_nonce, networking_key).unwrap();
    let jwtciphertext: Vec<u8> = cipher.encrypt(&jwt_nonce, jwt).unwrap();
    let fileciphertext: Vec<u8> = cipher.encrypt(&file_nonce, file_key.as_ref()).unwrap();

    bincode::serialize(&(
        username.clone(),
        routers.clone(),
        salt.to_vec(),
        [network_nonce.to_vec(), keyciphertext].concat(),
        [jwt_nonce.to_vec(), jwtciphertext].concat(),
        [file_nonce.to_vec(), fileciphertext].concat(),
    ))
    .unwrap()
}

pub fn decode_keyfile(keyfile: &[u8], password: &str) -> Result<Keyfile, &'static str> {
    let (username, routers, salt, key_enc, jwt_enc, file_enc) =
        bincode::deserialize::<(String, Vec<String>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)>(keyfile)
            .map_err(|_| "failed to deserialize keyfile")?;

    // rederive disk key
    let mut disk_key: DiskKey = [0u8; CREDENTIAL_LEN];
    pbkdf2::derive(
        PBKDF2_ALG,
        NonZeroU32::new(ITERATIONS).unwrap(),
        &salt,
        password.as_bytes(),
        &mut disk_key,
    );

    let cipher_key = Key::<Aes256Gcm>::from_slice(&disk_key);
    let cipher = Aes256Gcm::new(cipher_key);

    let net_nonce = GenericArray::from_slice(&key_enc[..12]);
    let jwt_nonce = GenericArray::from_slice(&jwt_enc[..12]);
    let file_nonce = GenericArray::from_slice(&file_enc[..12]);

    let serialized_networking_keypair: Vec<u8> = cipher
        .decrypt(net_nonce, &key_enc[12..])
        .map_err(|_| "failed to decrypt networking keys")?;

    let networking_keypair = signature::Ed25519KeyPair::from_pkcs8(&serialized_networking_keypair)
        .map_err(|_| "failed to parse networking keys")?;

    let jwt_secret_bytes: Vec<u8> = cipher
        .decrypt(jwt_nonce, &jwt_enc[12..])
        .map_err(|_| "failed to decrypt jwt secret")?;

    let file_key: Vec<u8> = cipher
        .decrypt(file_nonce, &file_enc[12..])
        .map_err(|_| "failed to decrypt file key")?;

    Ok(Keyfile {
        username,
        routers,
        networking_keypair,
        jwt_secret_bytes,
        file_key,
    })
}

pub fn generate_jwt(
    jwt_secret_bytes: &[u8],
    username: &str,
    subdomain: &Option<String>,
) -> Option<String> {
    let jwt_secret: Hmac<Sha256> = Hmac::new_from_slice(jwt_secret_bytes).ok()?;

    let subdomain = match subdomain.clone().unwrap_or_default().as_str() {
        "" => None,
        subdomain => Some(subdomain.to_string()),
    };

    let claims = crate::http::server_types::JwtClaims {
        username: username.to_string(),
        subdomain,
        expiration: 0,
    };

    claims.sign_with_key(&jwt_secret).ok()
}

#[cfg(not(feature = "simulation-mode"))]
pub fn get_username_and_routers(keyfile: &[u8]) -> Result<(String, Vec<String>), &'static str> {
    let (username, routers, _salt, _key_enc, _jwt_enc) =
        bincode::deserialize::<(String, Vec<String>, Vec<u8>, Vec<u8>, Vec<u8>)>(keyfile)
            .map_err(|_| "failed to deserialize keyfile")?;

    Ok((username, routers))
}

pub fn namehash(name: &str) -> Vec<u8> {
    let mut node = vec![0u8; 32];
    if name.is_empty() {
        return node;
    }
    let mut labels: Vec<&str> = name.split(".").collect();
    labels.reverse();
    for label in labels.iter() {
        node.append(&mut keccak256(label.as_bytes()).to_vec());
        node = keccak256(node.as_slice()).to_vec();
    }
    node
}

/// randomly generated key to encrypt file chunks,
pub fn generate_file_key() -> Vec<u8> {
    let mut key = [0u8; 32];
    let rng = SystemRandom::new();
    rng.fill(&mut key).unwrap();
    key.to_vec()
}

/// # Returns
/// a pair of (public key (encoded as a hex string), serialized key as a pkcs8 Document)
pub fn generate_networking_key() -> (String, Document) {
    let seed = SystemRandom::new();
    let doc = signature::Ed25519KeyPair::generate_pkcs8(&seed).unwrap();
    let keys = signature::Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
    (hex::encode(keys.public_key().as_ref()), doc)
}
