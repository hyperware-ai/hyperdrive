use hyperware_process_lib::eth;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Internal representation of LogsMetadata, similar to WIT but for Rust logic.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogsMetadataInternal {
    #[serde(rename = "chainId")]
    pub chain_id: String,
    #[serde(rename = "fromBlock")]
    pub from_block: String,
    #[serde(rename = "toBlock")]
    pub to_block: String,
    #[serde(rename = "timeCreated")]
    pub time_created: String,
    #[serde(rename = "createdBy")]
    pub created_by: String,
    pub signature: String, // Keccak256 hash of the log file content.
}

// Internal representation of a LogCache, containing metadata and actual logs.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogCacheInternal {
    pub metadata: LogsMetadataInternal,
    pub logs: Vec<eth::Log>, // The actual Ethereum logs.
}

// Internal representation of a ManifestItem.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManifestItemInternal {
    pub metadata: LogsMetadataInternal,
    #[serde(rename = "isEmpty")]
    pub is_empty: bool,
    #[serde(rename = "fileHash")]
    pub file_hash: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
}

// Internal representation of the Manifest.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ManifestInternal {
    pub items: HashMap<String, ManifestItemInternal>,
    pub manifest_filename: String,
    pub chain_id: String,
    pub protocol_version: String,
}

// The main state structure for the Hypermap Cacher process.
#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub hypermap_address: eth::Address,
    pub manifest: ManifestInternal,
    pub last_cached_block: u64,
    pub chain_id: String,
    pub protocol_version: String,
    pub cache_interval_s: u64,
    pub block_batch_size: u64,
    pub is_cache_timer_live: bool,
    pub drive_path: String,
    pub is_providing: bool,
    pub nodes: Vec<String>,
    #[serde(skip)]
    pub is_starting: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HttpApi {
    GetManifest,
    GetLogCacheFile(String),
    GetStatus,
}
