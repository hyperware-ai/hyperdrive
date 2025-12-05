//! chain:app-store:sys
//! This process manages the onchain interactions for the App Store system in the Hyperware ecosystem.
//! It is responsible for indexing and tracking app metadata stored on the blockchain.
//!
//! ## Responsibilities:
//!
//! 1. Index and track app metadata from the blockchain.
//! 2. Manage subscriptions to relevant blockchain events.
//! 3. Provide up-to-date information about available apps and their metadata.
//! 4. Handle auto-update settings for apps.
//!
//! ## Key Components:
//!
//! - `handle_eth_log`: Processes blockchain events related to app metadata updates.
//! - `fetch_and_subscribe_logs`: Initializes and maintains blockchain event subscriptions.
//!
//! ## Interaction Flow:
//!
//! 1. The process subscribes to relevant blockchain events on startup.
//! 2. When new events are received, they are processed to update the local state.
//! 3. Other processes (like main) can request information about apps.
//! 4. The chain process responds with the most up-to-date information from its local state.
//!
//! Note: This process does not handle app binaries or installation. It focuses solely on
//! metadata management and providing information about available apps.
//!
use crate::hyperware::process::chain::{
    ChainError, ChainRequest, OnchainApp, OnchainMetadata, OnchainProperties,
};
use crate::hyperware::process::downloads::{AutoUpdateRequest, DownloadRequest};
use alloy_primitives::{keccak256, U256};
use alloy_sol_types::SolEvent;
use hex;
use hyperware::process::chain::ChainResponse;
use hyperware_process_lib::{
    await_message,
    bindings::{
        contract::{
            BindAmountIncreased, BindCreated, BindDurationExtended, ExpiredBindReclaimed,
            LockExtended, TokensLocked, TokensWithdrawn,
        },
        decode_bind_amount_increased_log, decode_bind_created_log,
        decode_bind_duration_extended_log, decode_expired_bind_reclaimed_log,
        decode_lock_extended_log, decode_tokens_locked_log, decode_tokens_withdrawn_log, Bindings,
        BINDINGS_FIRST_BLOCK,
    },
    call_init, eth, get_blob, http, hypermap, kernel_types as kt, print_to_terminal, println,
    sqlite::{self, Sqlite},
    timer, Address, Message, PackageId, Request, Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

wit_bindgen::generate!({
    path: "../target/wit",
    generate_unused_types: true,
    world: "app-store-sys-v2",
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

#[cfg(not(feature = "simulation-mode"))]
const CHAIN_ID: u64 = hypermap::HYPERMAP_CHAIN_ID;
#[cfg(feature = "simulation-mode")]
const CHAIN_ID: u64 = 31337; // local

const CHAIN_TIMEOUT: u64 = 60; // 60s

const HYPERMAP_ADDRESS: &'static str = hypermap::HYPERMAP_ADDRESS;

const DELAY_MS: u64 = 1_000; // 1s

const SUBSCRIPTION_NUMBER: u64 = 1;
const BINDINGS_SUBSCRIPTION: u64 = 2;

pub struct State {
    /// the hypermap helper we are using
    pub hypermap: hypermap::Hypermap,
    /// the bindings helper for token registry
    pub bindings: Bindings,
    /// the last block at which we saved the state of the listings to disk.
    /// when we boot, we can read logs starting from this block and
    /// rebuild latest state.
    pub last_saved_block: u64,
    /// the last block for binding events
    pub last_bindings_block: u64,
    /// tables: listings: <packade_id, listing>, published: vec<package_id>
    pub db: DB,
}

/// listing information derived from metadata hash in listing event
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PackageListing {
    pub tba: eth::Address,
    pub metadata_uri: String,
    pub metadata_hash: String,
    pub metadata: Option<kt::Erc721Metadata>,
    pub auto_update: bool,
    pub block: u64,
}

/// User lock state from TokenRegistry
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserLock {
    pub user_address: String,
    pub amount: String, // U256 as string
    pub end_time: u64,
    pub block: u64,
}

/// User bind to an app namehash
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserBind {
    pub namehash: String,
    pub user_address: String,
    pub amount: String, // U256 as string
    pub end_time: u64,
    pub block: u64,
}

pub struct DB {
    inner: Sqlite,
}

const DB_VERSION: u64 = 2;

impl DB {
    pub fn connect(our: &Address) -> anyhow::Result<Self> {
        let inner = sqlite::open(our.package_id(), "app_store_chain.sqlite", Some(10))?;
        // create tables
        inner.write(CREATE_META_TABLE.into(), vec![], None)?;
        inner.write(CREATE_LISTINGS_TABLE.into(), vec![], None)?;
        inner.write(CREATE_PUBLISHED_TABLE.into(), vec![], None)?;
        // binding-related tables
        inner.write(CREATE_APP_NAMEHASHES_TABLE.into(), vec![], None)?;
        inner.write(CREATE_USER_LOCKS_TABLE.into(), vec![], None)?;
        inner.write(CREATE_USER_BINDS_TABLE.into(), vec![], None)?;
        inner.write(CREATE_USER_BINDS_INDEX.into(), vec![], None)?;

        let db = Self { inner };

        // versions and migrations
        let version = db.get_version()?;

        if version.is_none() {
            // clean up inconsistent state by re-indexing from block 0
            db.set_last_saved_block(0)?;
            db.set_last_bindings_block(0)?;
            db.set_version(DB_VERSION)?;
        } else if version == Some(1) {
            // Migrate from version 1 to 2: re-index everything to populate app_namehashes
            db.set_last_saved_block(0)?;
            db.set_last_bindings_block(0)?;
            db.set_version(DB_VERSION)?;
        }

        Ok(db)
    }

    pub fn drop_all(&self) -> anyhow::Result<()> {
        self.inner.write(DROP_META_TABLE.into(), vec![], None)?;
        self.inner.write(DROP_LISTINGS_TABLE.into(), vec![], None)?;
        self.inner
            .write(DROP_PUBLISHED_TABLE.into(), vec![], None)?;
        // binding-related tables
        self.inner
            .write(DROP_APP_NAMEHASHES_TABLE.into(), vec![], None)?;
        self.inner
            .write(DROP_USER_LOCKS_TABLE.into(), vec![], None)?;
        self.inner
            .write(DROP_USER_BINDS_TABLE.into(), vec![], None)?;

        Ok(())
    }

    pub fn get_last_saved_block(&self) -> anyhow::Result<u64> {
        let query = "SELECT value FROM meta WHERE key = 'last_saved_block'";
        let rows = self.inner.read(query.into(), vec![])?;
        if let Some(row) = rows.get(0) {
            if let Some(val_str) = row.get("value").and_then(|v| v.as_str()) {
                if let Ok(block) = val_str.parse::<u64>() {
                    return Ok(block);
                }
            }
        }
        Ok(0)
    }

    pub fn set_last_saved_block(&self, block: u64) -> anyhow::Result<()> {
        let query = "INSERT INTO meta (key, value) VALUES ('last_saved_block', ?)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value";
        let params = vec![block.to_string().into()];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn insert_or_update_listing(
        &self,
        package_id: &PackageId,
        listing: &PackageListing,
    ) -> anyhow::Result<()> {
        let metadata_json = if let Some(m) = &listing.metadata {
            serde_json::to_string(m)?
        } else {
            "".to_string()
        };

        let query = "INSERT INTO listings (package_name, publisher_node, tba, metadata_uri, metadata_hash, metadata_json, auto_update, block)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(package_name, publisher_node)
            DO UPDATE SET
              tba=excluded.tba,
              metadata_uri=excluded.metadata_uri,
              metadata_hash=excluded.metadata_hash,
              metadata_json=excluded.metadata_json,
              auto_update=excluded.auto_update,
              block=excluded.block";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
            listing.tba.to_string().into(),
            listing.metadata_uri.clone().into(),
            listing.metadata_hash.clone().into(),
            metadata_json.into(),
            (if listing.auto_update { 1 } else { 0 }).into(),
            listing.block.into(),
        ];

        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn delete_listing(&self, package_id: &PackageId) -> anyhow::Result<()> {
        let query = "DELETE FROM listings WHERE package_name = ? AND publisher_node = ?";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn get_listing(&self, package_id: &PackageId) -> anyhow::Result<Option<PackageListing>> {
        let query = "SELECT tba, metadata_uri, metadata_hash, metadata_json, auto_update, block FROM listings WHERE package_name = ? AND publisher_node = ?";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
        ];
        let rows = self.inner.read(query.into(), params)?;
        if let Some(row) = rows.get(0) {
            Ok(Some(self.row_to_listing(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_listings(&self) -> anyhow::Result<Vec<(PackageId, PackageListing)>> {
        let query = "SELECT package_name, publisher_node, tba, metadata_uri, metadata_hash, metadata_json, auto_update, block FROM listings";
        let rows = self.inner.read(query.into(), vec![])?;
        let mut listings = Vec::new();
        for row in rows {
            let pid = PackageId {
                package_name: row["package_name"].as_str().unwrap_or("").to_string(),
                publisher_node: row["publisher_node"].as_str().unwrap_or("").to_string(),
            };
            let listing = self.row_to_listing(&row)?;
            listings.push((pid, listing));
        }
        Ok(listings)
    }

    pub fn get_listings_batch(
        &self,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<(PackageId, PackageListing)>> {
        let query = format!(
            "SELECT package_name, publisher_node, tba, metadata_uri, metadata_hash, metadata_json, auto_update, block
             FROM listings
             ORDER BY package_name, publisher_node
             LIMIT {} OFFSET {}",
            limit, offset
        );

        let rows = self.inner.read(query, vec![])?;
        let mut listings = Vec::new();
        for row in rows {
            let pid = PackageId {
                package_name: row["package_name"].as_str().unwrap_or("").to_string(),
                publisher_node: row["publisher_node"].as_str().unwrap_or("").to_string(),
            };
            let listing = self.row_to_listing(&row)?;
            listings.push((pid, listing));
        }
        Ok(listings)
    }

    pub fn get_listings_since_block(
        &self,
        block_number: u64,
    ) -> anyhow::Result<Vec<(PackageId, PackageListing)>> {
        let query = "SELECT package_name, publisher_node, tba, metadata_uri, metadata_hash, metadata_json, auto_update, block
                     FROM listings
                     WHERE block >= ?";
        let params = vec![block_number.into()];
        let rows = self.inner.read(query.into(), params)?;
        let mut listings = Vec::new();
        for row in rows {
            let pid = PackageId {
                package_name: row["package_name"].as_str().unwrap_or("").to_string(),
                publisher_node: row["publisher_node"].as_str().unwrap_or("").to_string(),
            };
            let listing = self.row_to_listing(&row)?;
            listings.push((pid, listing));
        }
        Ok(listings)
    }

    pub fn row_to_listing(
        &self,
        row: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<PackageListing> {
        let tba_str = row["tba"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid tba"))?;
        let tba = tba_str.parse::<eth::Address>()?;
        let metadata_uri = row["metadata_uri"].as_str().unwrap_or("").to_string();
        let metadata_hash = row["metadata_hash"].as_str().unwrap_or("").to_string();
        let metadata_json = row["metadata_json"].as_str().unwrap_or("");
        let metadata: Option<hyperware_process_lib::kernel_types::Erc721Metadata> =
            if metadata_json.is_empty() {
                None
            } else {
                serde_json::from_str(metadata_json)?
            };
        let auto_update = row["auto_update"].as_i64().unwrap_or(0) == 1;
        let block = row["block"].as_i64().unwrap_or(0) as u64;

        Ok(PackageListing {
            tba,
            metadata_uri,
            metadata_hash,
            metadata,
            auto_update,
            block,
        })
    }

    pub fn get_published(&self, package_id: &PackageId) -> anyhow::Result<bool> {
        let query = "SELECT 1 FROM published WHERE package_name = ? AND publisher_node = ?";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
        ];
        let rows = self.inner.read(query.into(), params)?;
        Ok(!rows.is_empty())
    }

    pub fn insert_published(&self, package_id: &PackageId) -> anyhow::Result<()> {
        let query = "INSERT INTO published (package_name, publisher_node) VALUES (?, ?) ON CONFLICT DO NOTHING";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn delete_published(&self, package_id: &PackageId) -> anyhow::Result<()> {
        let query = "DELETE FROM published WHERE package_name = ? AND publisher_node = ?";
        let params = vec![
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn get_all_published(&self) -> anyhow::Result<Vec<PackageId>> {
        let query = "SELECT package_name, publisher_node FROM published";
        let rows = self.inner.read(query.into(), vec![])?;
        let mut result = Vec::new();
        for row in rows {
            let pid = PackageId {
                package_name: row["package_name"].as_str().unwrap_or("").to_string(),
                publisher_node: row["publisher_node"].as_str().unwrap_or("").to_string(),
            };
            result.push(pid);
        }
        Ok(result)
    }

    pub fn get_version(&self) -> anyhow::Result<Option<u64>> {
        let rows = self.inner.read(
            "SELECT value FROM meta WHERE key = 'version'".into(),
            vec![],
        )?;

        if let Some(row) = rows.first() {
            if let Some(value) = row.get("value") {
                if let serde_json::Value::String(version_str) = value {
                    return Ok(Some(version_str.parse()?));
                }
            }
        }
        Ok(None)
    }

    pub fn set_version(&self, version: u64) -> anyhow::Result<()> {
        self.inner.write(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('version', ?)".into(),
            vec![serde_json::Value::String(version.to_string())],
            None,
        )?;
        Ok(())
    }

    // Binding-related methods

    pub fn get_last_bindings_block(&self) -> anyhow::Result<u64> {
        let query = "SELECT value FROM meta WHERE key = 'last_bindings_block'";
        let rows = self.inner.read(query.into(), vec![])?;
        if let Some(row) = rows.first() {
            if let Some(val_str) = row.get("value").and_then(|v| v.as_str()) {
                if let Ok(block) = val_str.parse::<u64>() {
                    return Ok(block);
                }
            }
        }
        Ok(0)
    }

    pub fn set_last_bindings_block(&self, block: u64) -> anyhow::Result<()> {
        let query = "INSERT INTO meta (key, value) VALUES ('last_bindings_block', ?)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value";
        let params = vec![block.to_string().into()];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn insert_app_namehash(
        &self,
        namehash: &str,
        package_id: &PackageId,
        block: u64,
    ) -> anyhow::Result<()> {
        let query = "INSERT INTO app_namehashes (namehash, package_name, publisher_node, block)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(namehash) DO UPDATE SET
              package_name=excluded.package_name,
              publisher_node=excluded.publisher_node,
              block=excluded.block";
        let params = vec![
            namehash.into(),
            package_id.package_name.clone().into(),
            package_id.publisher_node.clone().into(),
            block.into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn get_app_namehash(&self, namehash: &str) -> anyhow::Result<Option<PackageId>> {
        let query = "SELECT package_name, publisher_node FROM app_namehashes WHERE namehash = ?";
        let params = vec![namehash.into()];
        let rows = self.inner.read(query.into(), params)?;
        if let Some(row) = rows.first() {
            let package_name = row["package_name"].as_str().unwrap_or("").to_string();
            let publisher_node = row["publisher_node"].as_str().unwrap_or("").to_string();
            return Ok(Some(PackageId {
                package_name,
                publisher_node,
            }));
        }
        Ok(None)
    }

    pub fn upsert_user_lock(
        &self,
        user_address: &str,
        amount: &str,
        end_time: u64,
        block: u64,
    ) -> anyhow::Result<()> {
        let query = "INSERT INTO user_locks (user_address, amount, end_time, block)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(user_address) DO UPDATE SET
              amount=excluded.amount,
              end_time=excluded.end_time,
              block=excluded.block";
        let params = vec![
            user_address.into(),
            amount.into(),
            end_time.into(),
            block.into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn get_user_lock(&self, user_address: &str) -> anyhow::Result<Option<UserLock>> {
        let query = "SELECT amount, end_time, block FROM user_locks WHERE user_address = ?";
        let params = vec![user_address.into()];
        let rows = self.inner.read(query.into(), params)?;
        if let Some(row) = rows.first() {
            return Ok(Some(UserLock {
                user_address: user_address.to_string(),
                amount: row["amount"].as_str().unwrap_or("0").to_string(),
                end_time: row["end_time"].as_u64().unwrap_or(0),
                block: row["block"].as_u64().unwrap_or(0),
            }));
        }
        Ok(None)
    }

    pub fn delete_user_lock(&self, user_address: &str) -> anyhow::Result<()> {
        let query = "DELETE FROM user_locks WHERE user_address = ?";
        let params = vec![user_address.into()];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn upsert_user_bind(
        &self,
        namehash: &str,
        user_address: &str,
        amount: &str,
        end_time: u64,
        block: u64,
    ) -> anyhow::Result<()> {
        let query = "INSERT INTO user_binds (namehash, user_address, amount, end_time, block)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(namehash, user_address) DO UPDATE SET
              amount=excluded.amount,
              end_time=excluded.end_time,
              block=excluded.block";
        let params = vec![
            namehash.into(),
            user_address.into(),
            amount.into(),
            end_time.into(),
            block.into(),
        ];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn delete_user_bind(&self, namehash: &str, user_address: &str) -> anyhow::Result<()> {
        let query = "DELETE FROM user_binds WHERE namehash = ? AND user_address = ?";
        let params = vec![namehash.into(), user_address.into()];
        self.inner.write(query.into(), params, None)?;
        Ok(())
    }

    pub fn get_all_binds_for_app(&self, namehash: &str) -> anyhow::Result<Vec<UserBind>> {
        // Only return binds that correspond to known apps
        let query = "SELECT ub.namehash, ub.user_address, ub.amount, ub.end_time, ub.block
            FROM user_binds ub
            INNER JOIN app_namehashes an ON ub.namehash = an.namehash
            WHERE ub.namehash = ?";
        let params = vec![namehash.into()];
        let rows = self.inner.read(query.into(), params)?;
        let mut binds = Vec::new();
        for row in rows {
            binds.push(UserBind {
                namehash: row["namehash"].as_str().unwrap_or("").to_string(),
                user_address: row["user_address"].as_str().unwrap_or("").to_string(),
                amount: row["amount"].as_str().unwrap_or("0").to_string(),
                end_time: row["end_time"].as_u64().unwrap_or(0),
                block: row["block"].as_u64().unwrap_or(0),
            });
        }
        Ok(binds)
    }
}

const CREATE_META_TABLE: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT
);";

const CREATE_LISTINGS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS listings (
    package_name TEXT NOT NULL,
    publisher_node TEXT NOT NULL,
    tba TEXT NOT NULL,
    metadata_uri TEXT,
    metadata_hash TEXT,
    metadata_json TEXT,
    auto_update INTEGER NOT NULL DEFAULT 0,
    block INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (package_name, publisher_node)
);";

const CREATE_PUBLISHED_TABLE: &str = "
CREATE TABLE IF NOT EXISTS published (
    package_name TEXT NOT NULL,
    publisher_node TEXT NOT NULL,
    PRIMARY KEY (package_name, publisher_node)
);";

const DROP_META_TABLE: &str = "
DROP TABLE IF EXISTS meta;
";

const DROP_LISTINGS_TABLE: &str = "
DROP TABLE IF EXISTS listings;
";

const DROP_PUBLISHED_TABLE: &str = "
DROP TABLE IF EXISTS published;
";

// Binding-related tables
const CREATE_APP_NAMEHASHES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS app_namehashes (
    namehash TEXT PRIMARY KEY,
    package_name TEXT NOT NULL,
    publisher_node TEXT NOT NULL,
    block INTEGER NOT NULL DEFAULT 0
);";

const CREATE_USER_LOCKS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS user_locks (
    user_address TEXT PRIMARY KEY,
    amount TEXT NOT NULL,
    end_time INTEGER NOT NULL,
    block INTEGER NOT NULL DEFAULT 0
);";

const CREATE_USER_BINDS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS user_binds (
    namehash TEXT NOT NULL,
    user_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    end_time INTEGER NOT NULL,
    block INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (namehash, user_address)
);";

const CREATE_USER_BINDS_INDEX: &str = "
CREATE INDEX IF NOT EXISTS idx_user_binds_namehash ON user_binds(namehash);
";

const DROP_APP_NAMEHASHES_TABLE: &str = "DROP TABLE IF EXISTS app_namehashes;";
const DROP_USER_LOCKS_TABLE: &str = "DROP TABLE IF EXISTS user_locks;";
const DROP_USER_BINDS_TABLE: &str = "DROP TABLE IF EXISTS user_binds;";

call_init!(init);
fn init(our: Address) {
    loop {
        println!("started");

        let eth_provider: eth::Provider = eth::Provider::new(CHAIN_ID, CHAIN_TIMEOUT);

        let db = DB::connect(&our).expect("failed to open DB");
        let hypermap_helper = hypermap::Hypermap::new(
            eth_provider.clone(),
            eth::Address::from_str(HYPERMAP_ADDRESS).unwrap(),
        );
        let bindings_helper = Bindings::default(CHAIN_TIMEOUT);
        let last_saved_block = db
            .get_last_saved_block()
            .unwrap_or(hypermap::HYPERMAP_FIRST_BLOCK);
        let last_bindings_block = db.get_last_bindings_block().unwrap_or(BINDINGS_FIRST_BLOCK);

        let mut state = State {
            hypermap: hypermap_helper,
            bindings: bindings_helper,
            last_saved_block,
            last_bindings_block,
            db,
        };

        fetch_and_subscribe_logs(&our, &mut state, last_saved_block);

        loop {
            match await_message() {
                Ok(message) => match handle_message(&our, &mut state, &message) {
                    Ok(true) => {
                        // reset state
                        break;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        print_to_terminal(0, &format!("chain indexer: error handling message: {e}"))
                    }
                },
                Err(send_error) => {
                    // we never send requests, so this is never expected
                    print_to_terminal(0, &format!("chain indexer: got send error: {send_error}"));
                }
            }
        }
    }
}

/// returns true if we should re-index
/// Context wrapper to distinguish log types in timer callbacks
#[derive(Debug, Deserialize, Serialize)]
enum LogContext {
    Hypermap(eth::Log),
    Bindings(eth::Log),
}

fn handle_message(our: &Address, state: &mut State, message: &Message) -> anyhow::Result<bool> {
    if !message.is_local() {
        // networking is off: we will never get non-local messages
        return Ok(false);
    }
    if !message.is_request() {
        // all responses should come from the timer process because it's the only process we request to
        if message.source().process == "timer:distro:sys" {
            let Some(context) = message.context() else {
                return Err(anyhow::anyhow!("No context in timer message"));
            };
            let log_context: LogContext = serde_json::from_slice(context)?;
            match log_context {
                LogContext::Hypermap(log) => {
                    handle_eth_log(our, state, log, false)?;
                }
                LogContext::Bindings(log) => {
                    handle_binding_log(state, log)?;
                    // Save bindings block after processing
                    if let Err(e) = state.db.set_last_bindings_block(state.last_bindings_block) {
                        print_to_terminal(0, &format!("error saving bindings block: {e}"));
                    }
                }
            }
            return Ok(false);
        }
    } else {
        if message.source().process == "eth:distro:sys" {
            let eth_result = serde_json::from_slice::<eth::EthSubResult>(message.body())?;
            if let Ok(eth::EthSub { id, result }) = eth_result {
                if let Ok(eth::SubscriptionResult::Log(ref log)) =
                    serde_json::from_value::<eth::SubscriptionResult>(result)
                {
                    // Determine which subscription this is from
                    // Note: log is Box<eth::Log>, we need to dereference it
                    let log_ref: &eth::Log = &**log;
                    let context = if id == SUBSCRIPTION_NUMBER {
                        LogContext::Hypermap(log_ref.clone())
                    } else if id == BINDINGS_SUBSCRIPTION {
                        LogContext::Bindings(log_ref.clone())
                    } else {
                        return Ok(false); // Unknown subscription
                    };
                    // delay handling of ETH RPC subscriptions by DELAY_MS
                    // to allow hns to have a chance to process block
                    timer::set_timer(DELAY_MS, Some(serde_json::to_vec(&context)?));
                }
            } else {
                // unsubscribe to make sure we have cleaned up after ourselves;
                //  drop Result since we don't care if no subscription exists,
                //  just being diligent in case it does!
                let _ = state.hypermap.provider.unsubscribe(SUBSCRIPTION_NUMBER);
                let _ = state.bindings.provider.unsubscribe(BINDINGS_SUBSCRIPTION);
                // re-subscribe if error
                state.hypermap.provider.subscribe_loop(
                    SUBSCRIPTION_NUMBER,
                    app_store_filter(state),
                    1,
                    0,
                );
                state.bindings.provider.subscribe_loop(
                    BINDINGS_SUBSCRIPTION,
                    bindings_filter(&state.bindings),
                    1,
                    0,
                );
            }
        } else {
            let req = serde_json::from_slice::<ChainRequest>(message.body())?;
            return handle_local_request(state, req);
        }
    }
    Ok(false)
}

fn handle_local_request(state: &mut State, req: ChainRequest) -> anyhow::Result<bool> {
    // Get current timestamp for binding power calculations
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    match req {
        ChainRequest::GetApp(package_id) => {
            let pid = package_id.clone().to_process_lib();
            let listing = state.db.get_listing(&pid)?;
            let onchain_app = listing.map(|app| {
                // Compute binding power for this app
                // hypermap::namehash returns a hex String, use it directly
                let namehash =
                    hypermap::namehash(&format!("{}.{}", pid.package_name, pid.publisher_node));
                let power = compute_app_total_binding_power(&state.db, &namehash, now)
                    .ok()
                    .map(|p| p.to_string());
                app.to_onchain_app(&pid, power)
            });
            let response = ChainResponse::GetApp(onchain_app);
            Response::new().body(&response).send()?;
        }
        ChainRequest::GetApps => {
            let listings = state.db.get_all_listings()?;
            // Compute binding power for each app and collect with power for sorting
            let mut apps_with_power: Vec<(OnchainApp, U256)> = listings
                .into_iter()
                .map(|(pid, listing)| {
                    // hypermap::namehash returns a hex String, use it directly
                    let namehash = hypermap::namehash(&format!(
                        "{}.{}",
                        pid.package_name, pid.publisher_node
                    ));
                    let power = compute_app_total_binding_power(&state.db, &namehash, now)
                        .unwrap_or(U256::ZERO);
                    print_to_terminal(
                        2,
                        &format!(
                            "[DEBUG BINDING] GetApps: {}.{}\n  computed namehash: {}\n  binding_power: {}",
                            pid.package_name, pid.publisher_node, namehash, power
                        ),
                    );
                    let app = listing.to_onchain_app(&pid, Some(power.to_string()));
                    (app, power)
                })
                .collect();

            // Sort by binding power descending
            apps_with_power.sort_by(|a, b| b.1.cmp(&a.1));

            let apps: Vec<OnchainApp> = apps_with_power.into_iter().map(|(app, _)| app).collect();
            let response = ChainResponse::GetApps(apps);
            Response::new().body(&response).send()?;
        }
        ChainRequest::GetOurApps => {
            let published_list = state.db.get_all_published()?;
            let mut apps_with_power: Vec<(OnchainApp, U256)> = Vec::new();
            for pid in published_list {
                if let Some(listing) = state.db.get_listing(&pid)? {
                    // hypermap::namehash returns a hex String, use it directly
                    let namehash =
                        hypermap::namehash(&format!("{}.{}", pid.package_name, pid.publisher_node));
                    let power = compute_app_total_binding_power(&state.db, &namehash, now)
                        .unwrap_or(U256::ZERO);
                    apps_with_power
                        .push((listing.to_onchain_app(&pid, Some(power.to_string())), power));
                }
            }
            // Sort by binding power descending
            apps_with_power.sort_by(|a, b| b.1.cmp(&a.1));
            let apps: Vec<OnchainApp> = apps_with_power.into_iter().map(|(app, _)| app).collect();
            let response = ChainResponse::GetOurApps(apps);
            Response::new().body(&response).send()?;
        }
        ChainRequest::StartAutoUpdate(package_id) => {
            let pid = package_id.to_process_lib();
            if let Some(mut listing) = state.db.get_listing(&pid)? {
                listing.auto_update = true;
                state.db.insert_or_update_listing(&pid, &listing)?;
                let response = ChainResponse::AutoUpdateStarted;
                Response::new().body(&response).send()?;
            } else {
                let error_response = ChainResponse::Err(ChainError::NoPackage);
                Response::new().body(&error_response).send()?;
            }
        }
        ChainRequest::StopAutoUpdate(package_id) => {
            let pid = package_id.to_process_lib();
            if let Some(mut listing) = state.db.get_listing(&pid)? {
                listing.auto_update = false;
                state.db.insert_or_update_listing(&pid, &listing)?;
                let response = ChainResponse::AutoUpdateStopped;
                Response::new().body(&response).send()?;
            } else {
                let error_response = ChainResponse::Err(ChainError::NoPackage);
                Response::new().body(&error_response).send()?;
            }
        }
        ChainRequest::Reset => {
            Response::new().body(&ChainResponse::ResetOk).send()?;
            println!("re-indexing state!");
            // set last_saved_block to 0 & drop tables to force re-index
            state.last_saved_block = 0;
            state.last_bindings_block = 0;
            state.db.set_last_saved_block(0)?;
            state.db.set_last_bindings_block(0)?;
            state.db.drop_all()?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn handle_eth_log(
    our: &Address,
    state: &mut State,
    log: eth::Log,
    startup: bool,
) -> anyhow::Result<()> {
    let block_number: u64 = log
        .block_number
        .ok_or(anyhow::anyhow!("log missing block number"))?;
    let Ok(note) = hypermap::decode_note_log(&log) else {
        // ignore invalid logs here -- they're not actionable
        return Ok(());
    };

    let package_id = note
        .parent_path
        .split_once('.')
        .ok_or(anyhow::anyhow!("invalid publisher name"))
        .and_then(|(package, publisher)| {
            if package.is_empty() || publisher.is_empty() {
                Err(anyhow::anyhow!("invalid publisher name"))
            } else {
                Ok(PackageId::new(package, publisher))
            }
        })?;

    // the app store exclusively looks for ~metadata-uri postings: if one is
    // observed, we then *query* for ~metadata-hash to verify the content
    // at the URI.

    let metadata_uri = String::from_utf8_lossy(&note.data).to_string();
    let is_our_package = package_id.publisher() == our.node();

    let (tba, metadata_hash) = if !startup {
        // generate ~metadata-hash full-path
        let hash_note = format!("~metadata-hash.{}", note.parent_path);

        // owner can change which we don't track (yet?) so don't save, need to get when desired
        let (tba, _owner, data) = match state.hypermap.get(&hash_note) {
            Ok(gr) => Ok(gr),
            Err(e) => match e {
                eth::EthError::RpcError(_) => {
                    // retry on RpcError after DELAY_MS sleep
                    // sleep here rather than with, e.g., a message to
                    // `timer:distro:sys` so that events are processed in
                    // order of receipt!
                    std::thread::sleep(std::time::Duration::from_millis(DELAY_MS));
                    state.hypermap.get(&hash_note)
                }
                _ => Err(e),
            },
        }
        .map_err(|e| anyhow::anyhow!("Couldn't find {hash_note}: {e:?}"))?;

        match data {
            None => {
                // unpublish if metadata_uri empty
                if metadata_uri.is_empty() {
                    state.db.delete_published(&package_id)?;
                    state.db.delete_listing(&package_id)?;
                    if !startup {
                        if block_number - 1 > state.last_saved_block {
                            state.last_saved_block = block_number - 1;
                            state.db.set_last_saved_block(block_number - 1)?;
                        }
                    }
                    return Ok(());
                }
                return Err(anyhow::anyhow!(
                    "metadata hash not found: {package_id}, {metadata_uri}"
                ));
            }
            Some(hash_note) => (tba, String::from_utf8_lossy(&hash_note).to_string()),
        }
    } else {
        (eth::Address::ZERO, String::new())
    };

    if is_our_package {
        state.db.insert_published(&package_id)?;
    }

    // if this is a startup event, we don't need to fetch metadata from the URI --
    // we'll loop over all listings after processing all logs and fetch them as needed.
    // fetch metadata from the URI (currently only handling HTTP(S) URLs!)
    // assert that the metadata hash matches the fetched data
    let metadata = if !startup {
        Some(fetch_metadata_from_url(&metadata_uri, &metadata_hash, 30)?)
    } else {
        None
    };

    let mut listing = state
        .db
        .get_listing(&package_id)?
        .unwrap_or(PackageListing {
            tba,
            metadata_uri: metadata_uri.clone(),
            metadata_hash: metadata_hash.clone(),
            metadata: metadata.clone(),
            auto_update: false,
            block: block_number,
        });
    // update fields
    listing.tba = tba;
    listing.metadata_uri = metadata_uri;
    listing.metadata_hash = metadata_hash;
    listing.block = block_number;
    if !startup {
        listing.metadata = metadata.clone();
    }

    state.db.insert_or_update_listing(&package_id, &listing)?;

    // Store the app's namehash for binding power lookups.
    // The parenthash is topics[1] from the Note event.
    if log.topics().len() >= 2 {
        let parenthash = log.topics()[1];
        let namehash_str = format!("0x{}", hex::encode(parenthash));
        // hypermap::namehash returns a hex String, use it directly
        let computed_namehash = hypermap::namehash(&format!(
            "{}.{}",
            package_id.package_name, package_id.publisher_node
        ));
        print_to_terminal(
            2,
            &format!(
                "[DEBUG BINDING] handle_eth_log: storing app namehash\n  package_id: {}\n  stored (topics[1]): {}\n  computed_namehash: {}\n  match: {}",
                package_id, namehash_str, computed_namehash, namehash_str == computed_namehash
            ),
        );
        state
            .db
            .insert_app_namehash(&namehash_str, &package_id, block_number)?;
    }

    if !startup && listing.auto_update {
        println!("kicking off auto-update for {package_id}");
        Request::to(("our", "downloads", "app-store", "sys"))
            .body(&DownloadRequest::AutoUpdate(AutoUpdateRequest {
                package_id: crate::hyperware::process::main::PackageId::from_process_lib(
                    package_id,
                ),
                metadata: metadata.unwrap().into(),
            }))
            .send()
            .unwrap();
    }

    if !startup {
        state.last_saved_block = block_number - 1;
        state.db.set_last_saved_block(block_number - 1)?;
    }

    Ok(())
}

/// after startup, fetch metadata for all listings
/// we do this as a separate step to not repeatedly fetch outdated metadata
/// as we process logs.
fn update_all_metadata(state: &mut State, last_saved_block: u64) {
    let updated_listings = match state.db.get_listings_since_block(last_saved_block) {
        Ok(listings) => listings,
        Err(e) => {
            print_to_terminal(
                0,
                &format!("error fetching updated listings since block {last_saved_block}: {e}"),
            );
            return;
        }
    };

    for (pid, mut listing) in updated_listings {
        let hash_note = format!("~metadata-hash.{}.{}", pid.package(), pid.publisher());
        let (tba, metadata_hash) = match state.hypermap.get(&hash_note) {
            Ok((t, _owner, data)) => {
                match data {
                    None => {
                        // If metadata_uri empty, unpublish
                        if listing.metadata_uri.is_empty() {
                            if let Err(e) = state.db.delete_published(&pid) {
                                print_to_terminal(1, &format!("error deleting published: {e}"));
                            }
                        }
                        if let Err(e) = state.db.delete_listing(&pid) {
                            print_to_terminal(1, &format!("error deleting listing: {e}"));
                        }
                        continue;
                    }
                    Some(hash_note) => (t, String::from_utf8_lossy(&hash_note).to_string()),
                }
            }
            Err(e) => {
                // If RpcError, retry once after delay
                if let eth::EthError::RpcError(_) = e {
                    std::thread::sleep(std::time::Duration::from_millis(DELAY_MS));
                    match state.hypermap.get(&hash_note) {
                        Ok((t, _owner, data)) => {
                            if let Some(hash_note) = data {
                                (t, String::from_utf8_lossy(&hash_note).to_string())
                            } else {
                                // no data again after retry
                                if listing.metadata_uri.is_empty() {
                                    if let Err(e) = state.db.delete_published(&pid) {
                                        print_to_terminal(
                                            1,
                                            &format!("error deleting published: {e}"),
                                        );
                                    }
                                }
                                if let Err(e) = state.db.delete_listing(&pid) {
                                    print_to_terminal(1, &format!("error deleting listing: {e}"));
                                }
                                continue;
                            }
                        }
                        Err(e2) => {
                            print_to_terminal(
                                1,
                                &format!("error retrieving metadata-hash after retry: {e2:?}"),
                            );
                            continue;
                        }
                    }
                } else {
                    print_to_terminal(
                        1,
                        &format!("error retrieving metadata-hash: {e:?} for {pid}"),
                    );
                    continue;
                }
            }
        };

        // Update listing fields
        listing.tba = tba;
        listing.metadata_hash = metadata_hash;

        let metadata =
            match fetch_metadata_from_url(&listing.metadata_uri, &listing.metadata_hash, 30) {
                Ok(md) => Some(md),
                Err(err) => {
                    print_to_terminal(0, &format!("error fetching metadata for {pid}: {err}"));
                    None
                }
            };
        listing.metadata = metadata.clone();

        if let Err(e) = state.db.insert_or_update_listing(&pid, &listing) {
            print_to_terminal(0, &format!("error updating listing {pid}: {e}"));
        }

        if listing.auto_update {
            if let Some(md) = metadata {
                print_to_terminal(0, &format!("kicking off auto-update for {pid}"));
                if let Err(e) = Request::to(("our", "downloads", "app-store", "sys"))
                    .body(&DownloadRequest::AutoUpdate(AutoUpdateRequest {
                        package_id: crate::hyperware::process::main::PackageId::from_process_lib(
                            pid.clone(),
                        ),
                        metadata: md.into(),
                    }))
                    .send()
                {
                    print_to_terminal(0, &format!("error sending auto-update request: {e}"));
                }
            }
        }
    }
}

/// create the filter used for app store getLogs and subscription.
/// the app store exclusively looks for ~metadata-uri postings: if one is
/// observed, we then *query* for ~metadata-hash to verify the content
/// at the URI.
///
/// this means that ~metadata-hash should be *posted before or at the same time* as ~metadata-uri!
pub fn app_store_filter(state: &State) -> eth::Filter {
    let notes = vec![keccak256("~metadata-uri")];

    eth::Filter::new()
        .address(*state.hypermap.address())
        .events([hypermap::contract::Note::SIGNATURE])
        .topic3(notes)
}

/// create a filter for binding events from TokenRegistry
fn bindings_filter(bindings: &Bindings) -> eth::Filter {
    eth::Filter::new().address(*bindings.address()).events([
        TokensLocked::SIGNATURE,
        LockExtended::SIGNATURE,
        TokensWithdrawn::SIGNATURE,
        BindCreated::SIGNATURE,
        BindAmountIncreased::SIGNATURE,
        BindDurationExtended::SIGNATURE,
        ExpiredBindReclaimed::SIGNATURE,
    ])
}

/// handle a binding log from the TokenRegistry contract
fn handle_binding_log(state: &mut State, log: eth::Log) -> anyhow::Result<()> {
    let block_number = log
        .block_number
        .ok_or(anyhow::anyhow!("log missing block number"))?;
    let topic0 = *log
        .topics()
        .first()
        .ok_or(anyhow::anyhow!("log missing topic"))?;

    // Handle lock events
    if topic0 == TokensLocked::SIGNATURE_HASH {
        let parsed = decode_tokens_locked_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode TokensLocked: {:?}", e))?;
        state.db.upsert_user_lock(
            &parsed.account.to_string(),
            &parsed.balance.to_string(),
            parsed.end_time.to::<u64>(),
            block_number,
        )?;
    } else if topic0 == LockExtended::SIGNATURE_HASH {
        let parsed = decode_lock_extended_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode LockExtended: {:?}", e))?;
        state.db.upsert_user_lock(
            &parsed.account.to_string(),
            &parsed.balance.to_string(),
            parsed.end_time.to::<u64>(),
            block_number,
        )?;
    } else if topic0 == TokensWithdrawn::SIGNATURE_HASH {
        let parsed = decode_tokens_withdrawn_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode TokensWithdrawn: {:?}", e))?;
        if parsed.remaining_amount.is_zero() {
            state.db.delete_user_lock(&parsed.user.to_string())?;
        } else {
            state.db.upsert_user_lock(
                &parsed.user.to_string(),
                &parsed.remaining_amount.to_string(),
                parsed.end_time.to::<u64>(),
                block_number,
            )?;
        }
    }
    // Handle bind events - store all, filter on query
    else if topic0 == BindCreated::SIGNATURE_HASH {
        let parsed = decode_bind_created_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode BindCreated: {:?}", e))?;
        let namehash_str = format!("0x{}", hex::encode(parsed.namehash));
        print_to_terminal(
            2,
            &format!(
                "[DEBUG BINDING] handle_binding_log BindCreated:\n  user: {}\n  namehash: {}\n  amount: {}\n  end_time: {}",
                parsed.user, namehash_str, parsed.amount, parsed.end_time
            ),
        );
        state.db.upsert_user_bind(
            &namehash_str,
            &parsed.user.to_string(),
            &parsed.amount.to_string(),
            parsed.end_time.to::<u64>(),
            block_number,
        )?;
    } else if topic0 == BindAmountIncreased::SIGNATURE_HASH {
        let parsed = decode_bind_amount_increased_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode BindAmountIncreased: {:?}", e))?;
        let namehash_str = format!("0x{}", hex::encode(parsed.namehash));
        state.db.upsert_user_bind(
            &namehash_str,
            &parsed.user.to_string(),
            &parsed.amount.to_string(),
            parsed.end_time.to::<u64>(),
            block_number,
        )?;
    } else if topic0 == BindDurationExtended::SIGNATURE_HASH {
        let parsed = decode_bind_duration_extended_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode BindDurationExtended: {:?}", e))?;
        let namehash_str = format!("0x{}", hex::encode(parsed.namehash));
        state.db.upsert_user_bind(
            &namehash_str,
            &parsed.user.to_string(),
            &parsed.amount.to_string(),
            parsed.end_time.to::<u64>(),
            block_number,
        )?;
    } else if topic0 == ExpiredBindReclaimed::SIGNATURE_HASH {
        let parsed = decode_expired_bind_reclaimed_log(&log)
            .map_err(|e| anyhow::anyhow!("failed to decode ExpiredBindReclaimed: {:?}", e))?;
        let namehash_str = format!("0x{}", hex::encode(parsed.namehash));
        state
            .db
            .delete_user_bind(&namehash_str, &parsed.user.to_string())?;
    }

    if block_number > state.last_bindings_block {
        state.last_bindings_block = block_number;
    }
    Ok(())
}

// Binding power formula constants (matching Solidity contract)
const BINDING_POWER_A: u128 = 1;
const BINDING_POWER_B: u128 = 2_000_000_000_000_000_000_000_000_000; // 2 * 1 * 1_000_000_000e18
const MIN_LOCK_DURATION: u64 = 4 * 7 * 24 * 60 * 60; // 4 weeks in seconds
const MAX_LOCK_DURATION: u64 = 4 * 52 * 7 * 24 * 60 * 60; // ~4 years in seconds
const BINDING_POWER_C: u64 = MIN_LOCK_DURATION / 100; // 24192
const BINDING_POWER_D: u128 = 2 * (BINDING_POWER_C as u128) * (MAX_LOCK_DURATION as u128);

/// Compute binding power for a single bind using the sublinear formula
/// from the Solidity contract
fn compute_binding_power(value: U256, remaining_duration: u64) -> U256 {
    if remaining_duration == 0 || value.is_zero() {
        return U256::ZERO;
    }

    let value_u128: u128 = value.try_into().unwrap_or(u128::MAX);
    // Round up duration to minimum if below
    let duration = remaining_duration.max(MIN_LOCK_DURATION) as u128;

    // value_term = (value / A - value * value / B)
    let value_term = (value_u128 / BINDING_POWER_A)
        .saturating_sub(value_u128.saturating_mul(value_u128) / BINDING_POWER_B);

    // duration_term = (duration / C - duration * duration / D)
    let duration_term = (duration / BINDING_POWER_C as u128)
        .saturating_sub(duration.saturating_mul(duration) / BINDING_POWER_D);

    U256::from(value_term.saturating_mul(duration_term))
}

/// Compute total binding power for an app by summing across all user binds
fn compute_app_total_binding_power(db: &DB, namehash: &str, now: u64) -> anyhow::Result<U256> {
    let binds = db.get_all_binds_for_app(namehash)?;
    print_to_terminal(
        2,
        &format!(
            "[DEBUG BINDING] compute_app_total_binding_power:\n  query namehash: {}\n  binds found: {}\n  now: {}",
            namehash, binds.len(), now
        ),
    );
    for bind in &binds {
        print_to_terminal(
            2,
            &format!(
                "  bind: user={}, amount={}, end_time={}",
                bind.user_address, bind.amount, bind.end_time
            ),
        );
    }
    let mut total = U256::ZERO;

    for bind in binds {
        if let Some(lock) = db.get_user_lock(&bind.user_address)? {
            // effective_duration = min(lock_end_time, bind_end_time) - now
            let effective_end = std::cmp::min(lock.end_time, bind.end_time);
            if effective_end > now {
                let remaining = effective_end - now;
                let amount = U256::from_str(&bind.amount).unwrap_or(U256::ZERO);
                total += compute_binding_power(amount, remaining);
            }
        }
    }

    Ok(total)
}

/// create a filter to fetch app store event logs from chain and subscribe to new events
pub fn fetch_and_subscribe_logs(our: &Address, state: &mut State, last_saved_block: u64) {
    let filter = app_store_filter(state);
    // get past logs, subscribe to new ones.
    // subscribe first so we don't miss any logs
    //
    // unsubscribe to make sure we have cleaned up after ourselves;
    //  drop Result since we don't care if no subscription exists,
    //  just being diligent in case it does!
    let _ = state.hypermap.provider.unsubscribe(SUBSCRIPTION_NUMBER);
    state
        .hypermap
        .provider
        .subscribe_loop(SUBSCRIPTION_NUMBER, filter.clone(), 1, 0);

    let mut maybe_block = None;
    match state.hypermap.bootstrap(
        Some(last_saved_block),
        vec![filter.clone()],
        Some((5, None)),
        None,
    ) {
        Err(e) => println!("bootstrap from cache failed: {e:?}"),
        Ok((block, mut logs)) => {
            maybe_block = Some(block);
            assert_eq!(logs.len(), 1);
            if let Some(logs) = logs.pop() {
                for log in logs {
                    if let Err(e) = handle_eth_log(our, state, log, true) {
                        print_to_terminal(1, &format!("error ingesting log: {e}"));
                    };
                }
            }
        }
    }

    // update metadata for all cached elements:
    //  need to update here so we can update block number or else `fetch_logs()`
    //  will grab blocks we just got from cache!
    update_all_metadata(state, last_saved_block);
    if let Some(block) = maybe_block {
        if block > state.last_saved_block {
            // save updated last_saved_block
            state.last_saved_block = block;
            if let Err(e) = state.db.set_last_saved_block(block) {
                print_to_terminal(0, &format!("error saving last block after startup: {e}"));
            }
        }
    }

    let block_from_cache = state.last_saved_block;
    // println!("fetching old logs from block {last_saved_block}");
    for log in fetch_logs(
        &state.hypermap.provider,
        &filter.from_block(state.last_saved_block),
    ) {
        if let Err(e) = handle_eth_log(our, state, log, true) {
            print_to_terminal(1, &format!("error ingesting log: {e}"));
        };
    }

    // update metadata for any noncached elements
    update_all_metadata(state, block_from_cache);

    // Now handle bindings subscription and bootstrap
    let bindings_fltr = bindings_filter(&state.bindings);
    let _ = state.bindings.provider.unsubscribe(BINDINGS_SUBSCRIPTION);
    state
        .bindings
        .provider
        .subscribe_loop(BINDINGS_SUBSCRIPTION, bindings_fltr.clone(), 1, 0);

    // Bootstrap bindings from cacher
    println!(
        "bootstrapping bindings from block {}",
        state.last_bindings_block
    );
    match state
        .bindings
        .get_bootstrap(Some(state.last_bindings_block), Some((5, None)), None)
    {
        Err(e) => println!("bindings bootstrap from cache failed: {e:?}"),
        Ok((block, logs)) => {
            for log in logs {
                if let Err(e) = handle_binding_log(state, log) {
                    print_to_terminal(1, &format!("error ingesting binding log: {e}"));
                }
            }
            if block > state.last_bindings_block {
                state.last_bindings_block = block;
                if let Err(e) = state.db.set_last_bindings_block(block) {
                    print_to_terminal(0, &format!("error saving bindings block: {e}"));
                }
            }
        }
    }

    // Fetch remaining binding logs via RPC
    for log in fetch_logs(
        &state.bindings.provider,
        &bindings_fltr.from_block(state.last_bindings_block),
    ) {
        if let Err(e) = handle_binding_log(state, log) {
            print_to_terminal(1, &format!("error ingesting binding log: {e}"));
        }
    }
    println!(
        "bindings bootstrap complete, last block: {}",
        state.last_bindings_block
    );
}

/// fetch logs from the chain with a given filter
fn fetch_logs(eth_provider: &eth::Provider, filter: &eth::Filter) -> Vec<eth::Log> {
    loop {
        match eth_provider.get_logs(filter) {
            Ok(res) => return res,
            Err(_) => {
                println!("failed to fetch logs! trying again in 5s...");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        }
    }
}

/// fetch metadata from url and verify it matches metadata_hash
pub fn fetch_metadata_from_url(
    metadata_url: &str,
    metadata_hash: &str,
    timeout: u64,
) -> Result<kt::Erc721Metadata, anyhow::Error> {
    if let Ok(url) = url::Url::parse(metadata_url) {
        if let Ok(_) =
            http::client::send_request_await_response(http::Method::GET, url, None, timeout, vec![])
        {
            if let Some(body) = get_blob() {
                let hash = keccak_256_hash(&body.bytes);
                if &hash == metadata_hash {
                    return Ok(serde_json::from_slice::<kt::Erc721Metadata>(&body.bytes)
                        .map_err(|_| anyhow::anyhow!("metadata not found"))?);
                } else {
                    return Err(anyhow::anyhow!("metadata hash mismatch"));
                }
            }
        }
    }
    Err(anyhow::anyhow!("metadata not found"))
}

/// generate a Keccak-256 hash string (with 0x prefix) of the metadata bytes
pub fn keccak_256_hash(bytes: &[u8]) -> String {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    format!("0x{:x}", hasher.finalize())
}

// quite annoyingly, we must convert from our gen'd version of PackageId
// to the process_lib's gen'd version. this is in order to access custom
// Impls that we want to use
impl crate::hyperware::process::main::PackageId {
    pub fn to_process_lib(self) -> PackageId {
        PackageId {
            package_name: self.package_name,
            publisher_node: self.publisher_node,
        }
    }
    pub fn from_process_lib(package_id: PackageId) -> Self {
        Self {
            package_name: package_id.package_name,
            publisher_node: package_id.publisher_node,
        }
    }
}

impl PackageListing {
    pub fn to_onchain_app(
        &self,
        package_id: &PackageId,
        binding_power: Option<String>,
    ) -> OnchainApp {
        OnchainApp {
            package_id: crate::hyperware::process::main::PackageId::from_process_lib(
                package_id.clone(),
            ),
            tba: self.tba.to_string(),
            metadata_uri: self.metadata_uri.clone(),
            metadata_hash: self.metadata_hash.clone(),
            metadata: self.metadata.as_ref().map(|m| m.clone().into()),
            auto_update: self.auto_update,
            binding_power,
        }
    }
}

impl From<kt::Erc721Metadata> for OnchainMetadata {
    fn from(erc: kt::Erc721Metadata) -> Self {
        OnchainMetadata {
            name: erc.name,
            description: erc.description,
            image: erc.image,
            external_url: erc.external_url,
            animation_url: erc.animation_url,
            properties: OnchainProperties {
                package_name: erc.properties.package_name,
                publisher: erc.properties.publisher,
                current_version: erc.properties.current_version,
                mirrors: erc.properties.mirrors,
                code_hashes: erc.properties.code_hashes.into_iter().collect(),
                license: erc.properties.license,
                screenshots: erc.properties.screenshots,
                wit_version: erc.properties.wit_version,
                dependencies: erc.properties.dependencies,
            },
        }
    }
}
