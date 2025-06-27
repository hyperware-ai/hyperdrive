use std::{
    cmp::{max, min},
    collections::HashMap,
    str::FromStr,
};

use alloy::hex;
use alloy_primitives::keccak256;
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::hyperware::process::hypermap_cacher::{
    CacherRequest, CacherResponse, GetLogsByRangeOkResponse, GetLogsByRangeRequest,
    LogsMetadata as WitLogsMetadata, Manifest as WitManifest, ManifestItem as WitManifestItem,
};

use hyperware_process_lib::{
    eth, get_state, hypermap,
    logging::{error, info, warn},
    net::{NetAction, NetResponse},
    our, set_state, sign, vfs, Address, Request,
};

use crate::constants::{
    DEFAULT_BLOCK_BATCH_SIZE, DEFAULT_CACHE_INTERVAL_S, DEFAULT_NODES, LOG_ITERATION_DELAY_MS,
    MAX_LOG_RETRIES, PROTOCOL_VERSION, RETRY_DELAY_S,
};
use crate::types::{
    LogCacheInternal, LogsMetadataInternal, ManifestInternal, ManifestItemInternal, State,
};
use crate::utils::get_current_timestamp_str;

impl State {
    pub fn new(drive_path: &str) -> Self {
        let chain_id = hypermap::HYPERMAP_CHAIN_ID.to_string();
        let hypermap_address = eth::Address::from_str(hypermap::HYPERMAP_ADDRESS)
            .expect("Failed to parse HYPERMAP_ADDRESS");

        let manifest_filename = format!(
            "manifest-chain{}-protocol{}.json",
            chain_id, PROTOCOL_VERSION
        );
        let initial_manifest = ManifestInternal {
            items: HashMap::new(),
            manifest_filename: manifest_filename.clone(),
            chain_id: chain_id.clone(),
            protocol_version: PROTOCOL_VERSION.to_string(),
        };

        State {
            hypermap_address,
            manifest: initial_manifest,
            last_cached_block: hypermap::HYPERMAP_FIRST_BLOCK,
            chain_id,
            protocol_version: PROTOCOL_VERSION.to_string(),
            cache_interval_s: DEFAULT_CACHE_INTERVAL_S,
            block_batch_size: DEFAULT_BLOCK_BATCH_SIZE,
            is_cache_timer_live: false,
            drive_path: drive_path.to_string(),
            is_providing: false,
            nodes: DEFAULT_NODES.iter().map(|s| s.to_string()).collect(),
            is_starting: true,
        }
    }

    pub fn load(drive_path: &str) -> Self {
        match get_state() {
            Some(state_bytes) => match serde_json::from_slice::<Self>(&state_bytes) {
                Ok(mut loaded_state) => {
                    info!("Successfully loaded state from checkpoint.");
                    // Always start in starting mode to bootstrap from other nodes
                    // is_starting is not serialized, so it defaults to false and we set it to true
                    loaded_state.is_starting = true;
                    loaded_state.drive_path = drive_path.to_string();

                    // Validate state against manifest file on disk
                    if let Err(e) = loaded_state.validate_state_against_manifest() {
                        warn!("State validation failed: {:?}. Clearing drive and creating fresh state.", e);
                        if let Err(clear_err) = loaded_state.clear_drive() {
                            error!("Failed to clear drive: {:?}", clear_err);
                        }
                        return Self::new(drive_path);
                    }

                    loaded_state
                }
                Err(e) => {
                    warn!(
                        "Failed to deserialize saved state: {:?}. Creating new state.",
                        e
                    );
                    Self::new(drive_path)
                }
            },
            None => {
                info!("No saved state found. Creating new state.");
                Self::new(drive_path)
            }
        }
    }

    pub fn save(&self) {
        match serde_json::to_vec(self) {
            Ok(state_bytes) => set_state(&state_bytes),
            Err(e) => error!("Fatal: Failed to serialize state for saving: {:?}", e),
        }
        info!(
            "State checkpoint saved. Last cached block: {}",
            self.last_cached_block
        );
    }

    // Core logic for fetching logs, creating cache files, and updating the manifest.
    pub fn cache_logs_and_update_manifest(
        &mut self,
        hypermap: &hypermap::Hypermap,
    ) -> anyhow::Result<()> {
        let current_chain_head = match hypermap.provider.get_block_number() {
            Ok(block_num) => block_num,
            Err(e) => {
                error!(
                    "Failed to get current block number: {:?}. Skipping cycle.",
                    e
                );
                return Err(anyhow::anyhow!("Failed to get block number: {:?}", e));
            }
        };

        if self.last_cached_block >= current_chain_head {
            info!(
                "Already caught up to chain head ({}). Nothing to cache.",
                current_chain_head
            );
            return Ok(());
        }

        while self.last_cached_block != current_chain_head {
            self.cache_logs_and_update_manifest_step(hypermap, Some(current_chain_head))?;

            std::thread::sleep(std::time::Duration::from_millis(LOG_ITERATION_DELAY_MS));
        }

        Ok(())
    }

    pub fn cache_logs_and_update_manifest_step(
        &mut self,
        hypermap: &hypermap::Hypermap,
        to_block: Option<u64>,
    ) -> anyhow::Result<()> {
        info!(
            "Starting caching cycle. From block: {}",
            self.last_cached_block + 1
        );

        let current_chain_head = match to_block {
            Some(b) => b,
            None => match hypermap.provider.get_block_number() {
                Ok(block_num) => block_num,
                Err(e) => {
                    error!(
                        "Failed to get current block number: {:?}. Skipping cycle.",
                        e
                    );
                    return Err(anyhow::anyhow!("Failed to get block number: {:?}", e));
                }
            },
        };

        if self.last_cached_block >= current_chain_head {
            info!(
                "Already caught up to chain head ({}). Nothing to cache.",
                current_chain_head
            );
            return Ok(());
        }

        let from_block = self.last_cached_block + 1;
        let mut to_block = from_block + self.block_batch_size - 1;
        if to_block > current_chain_head {
            to_block = current_chain_head;
        }

        if from_block > to_block {
            info!("From_block {} is greater than to_block {}. Chain might not have advanced enough. Skipping.", from_block, to_block);
            return Ok(());
        }

        let filter = eth::Filter::new()
            .address(self.hypermap_address)
            .from_block(from_block)
            .to_block(eth::BlockNumberOrTag::Number(to_block));

        let logs = {
            let mut attempt = 0;
            loop {
                match hypermap.provider.get_logs(&filter) {
                    Ok(logs) => break logs,
                    Err(e) => {
                        attempt += 1;
                        if attempt >= MAX_LOG_RETRIES {
                            error!(
                                "Failed to get logs after {} retries: {:?}",
                                MAX_LOG_RETRIES, e
                            );
                            return Err(anyhow::anyhow!("Failed to get logs: {:?}", e));
                        }
                        warn!(
                            "Error getting logs (attempt {}/{}): {:?}. Retrying in {}s...",
                            attempt, MAX_LOG_RETRIES, e, RETRY_DELAY_S
                        );
                        std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_S));
                    }
                }
            }
        };

        info!(
            "Fetched {} logs from block {} to {}.",
            logs.len(),
            from_block,
            to_block
        );

        let our = our();

        let metadata = LogsMetadataInternal {
            chain_id: self.chain_id.clone(),
            from_block: from_block.to_string(),
            to_block: to_block.to_string(),
            time_created: get_current_timestamp_str(),
            created_by: our.to_string(),
            signature: "".to_string(),
        };

        let mut log_cache = LogCacheInternal {
            metadata,
            logs: logs.clone(),
        };

        let mut logs_bytes_for_sig = serde_json::to_vec(&log_cache.logs).unwrap_or_default();
        logs_bytes_for_sig.extend_from_slice(&from_block.to_be_bytes());
        logs_bytes_for_sig.extend_from_slice(&to_block.to_be_bytes());
        let logs_hash_for_sig = keccak256(&logs_bytes_for_sig);

        let signature = sign::net_key_sign(logs_hash_for_sig.to_vec())?;

        log_cache.metadata.signature = format!("0x{}", hex::encode(signature));

        // Final serialization of LogCacheInternal with the signature.
        let final_log_cache_bytes = match serde_json::to_vec(&log_cache) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(
                    "Failed to re-serialize LogCacheInternal with signature: {:?}",
                    e
                );
                return Err(e.into());
            }
        };

        let file_hash_for_manifest =
            format!("0x{}", hex::encode(keccak256(&final_log_cache_bytes)));

        let log_cache_filename = format!(
            "{}-chain{}-from{}-to{}-protocol{}.json",
            log_cache
                .metadata
                .time_created
                .replace(":", "")
                .replace("-", ""), // Make timestamp filename-safe
            self.chain_id,
            from_block,
            to_block,
            self.protocol_version
        );

        if !logs.is_empty() {
            let log_cache_path = format!("{}/{}", self.drive_path, log_cache_filename);
            let mut log_cache_file = vfs::open_file(&log_cache_path, true, None)?;

            if let Err(e) = log_cache_file.write_all(&final_log_cache_bytes) {
                error!("Failed to write log cache file {}: {:?}", log_cache_path, e);
                return Err(e.into());
            }
            info!("Successfully wrote log cache file: {}", log_cache_path);
        }

        let manifest_item = ManifestItemInternal {
            metadata: log_cache.metadata.clone(),
            is_empty: logs.is_empty(),
            file_hash: file_hash_for_manifest,
            file_name: if logs.is_empty() {
                "".to_string()
            } else {
                log_cache_filename.clone()
            },
        };
        self.manifest
            .items
            .insert(log_cache_filename.clone(), manifest_item);
        self.manifest.chain_id = self.chain_id.clone();
        self.manifest.protocol_version = self.protocol_version.clone();

        let manifest_bytes = match serde_json::to_vec(&self.manifest) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to serialize manifest: {:?}", e);
                return Err(e.into());
            }
        };

        let manifest_path = format!("{}/{}", self.drive_path, self.manifest.manifest_filename);
        let manifest_file = vfs::open_file(&manifest_path, true, None)?;

        if let Err(e) = manifest_file.write(&manifest_bytes) {
            error!("Failed to write manifest file {}: {:?}", manifest_path, e);
            return Err(e.into());
        }
        info!(
            "Successfully updated and wrote manifest file: {}",
            manifest_path
        );

        self.last_cached_block = to_block;
        self.save();

        Ok(())
    }

    // Validate that the in-memory state matches the manifest file on disk
    pub fn validate_state_against_manifest(&self) -> anyhow::Result<()> {
        let manifest_path = format!("{}/{}", self.drive_path, self.manifest.manifest_filename);

        // Check if manifest file exists
        match vfs::open_file(&manifest_path, false, None) {
            Ok(manifest_file) => {
                match manifest_file.read() {
                    Ok(disk_manifest_bytes) => {
                        match serde_json::from_slice::<ManifestInternal>(&disk_manifest_bytes) {
                            Ok(disk_manifest) => {
                                // Compare key aspects of the manifests
                                if self.manifest.chain_id != disk_manifest.chain_id {
                                    return Err(anyhow::anyhow!(
                                        "Chain ID mismatch: state has {}, disk has {}",
                                        self.manifest.chain_id,
                                        disk_manifest.chain_id
                                    ));
                                }

                                if self.manifest.protocol_version != disk_manifest.protocol_version
                                {
                                    return Err(anyhow::anyhow!(
                                        "Protocol version mismatch: state has {}, disk has {}",
                                        self.manifest.protocol_version,
                                        disk_manifest.protocol_version
                                    ));
                                }

                                // Check if all files mentioned in state manifest exist on disk
                                for (_filename, item) in &self.manifest.items {
                                    if !item.file_name.is_empty() {
                                        let file_path =
                                            format!("{}/{}", self.drive_path, item.file_name);
                                        if vfs::metadata(&file_path, None).is_err() {
                                            return Err(anyhow::anyhow!(
                                                "File {} mentioned in state manifest does not exist on disk",
                                                item.file_name
                                            ));
                                        }
                                    }
                                }

                                // Check if disk manifest has more recent data than our state
                                let disk_max_block = disk_manifest
                                    .items
                                    .values()
                                    .filter_map(|item| item.metadata.to_block.parse::<u64>().ok())
                                    .max()
                                    .unwrap_or(0);

                                let state_max_block = self
                                    .manifest
                                    .items
                                    .values()
                                    .filter_map(|item| item.metadata.to_block.parse::<u64>().ok())
                                    .max()
                                    .unwrap_or(0);

                                if disk_max_block > state_max_block {
                                    return Err(anyhow::anyhow!(
                                        "Disk manifest has more recent data (block {}) than state (block {})",
                                        disk_max_block, state_max_block
                                    ));
                                }

                                info!("State validation passed - state matches manifest file");
                                Ok(())
                            }
                            Err(e) => {
                                Err(anyhow::anyhow!("Failed to parse manifest file: {:?}", e))
                            }
                        }
                    }
                    Err(e) => Err(anyhow::anyhow!("Failed to read manifest file: {:?}", e)),
                }
            }
            Err(_) => {
                // Manifest file doesn't exist - this is okay for new installs
                if self.manifest.items.is_empty() {
                    info!("No manifest file found, but state is also empty - validation passed");
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "State has manifest items but no manifest file exists on disk"
                    ))
                }
            }
        }
    }

    // Clear all files from the drive
    pub fn clear_drive(&self) -> anyhow::Result<()> {
        info!("Clearing all files from drive: {}", self.drive_path);

        // Remove the manifest file
        let manifest_path = format!("{}/{}", self.drive_path, self.manifest.manifest_filename);
        match vfs::remove_file(&manifest_path, None) {
            Ok(_) => info!("Removed manifest file: {}", manifest_path),
            Err(e) => warn!("Failed to remove manifest file {}: {:?}", manifest_path, e),
        }

        // Remove all files mentioned in the manifest
        for (_, item) in &self.manifest.items {
            if !item.file_name.is_empty() {
                let file_path = format!("{}/{}", self.drive_path, item.file_name);
                match vfs::remove_file(&file_path, None) {
                    Ok(_) => info!("Removed cache file: {}", file_path),
                    Err(e) => warn!("Failed to remove cache file {}: {:?}", file_path, e),
                }
            }
        }

        info!("Drive clearing completed");
        Ok(())
    }

    // Bootstrap state from other nodes, then fallback to RPC
    pub fn bootstrap_state(&mut self, hypermap: &hypermap::Hypermap) -> anyhow::Result<()> {
        info!("Starting state bootstrap process...");

        // Try to bootstrap from other nodes first
        if let Ok(()) = self.try_bootstrap_from_nodes() {
            info!("Successfully bootstrapped from other nodes");
        }

        self.try_bootstrap_from_rpc(hypermap)?;

        // Mark as no longer starting
        self.is_starting = false;
        self.save();
        info!("Bootstrap process completed, cacher is now ready");
        Ok(())
    }

    // Try to bootstrap from other hypermap-cacher nodes
    pub fn try_bootstrap_from_nodes(&mut self) -> anyhow::Result<()> {
        if self.nodes.is_empty() {
            info!("No nodes configured for bootstrap, will fallback to RPC");
            return Err(anyhow::anyhow!("No nodes configured for bootstrap"));
        }

        info!("Attempting to bootstrap from {} nodes", self.nodes.len());

        let mut nodes = self.nodes.clone();

        // If using default nodes, shuffle them for random order
        let default_nodes: Vec<String> = DEFAULT_NODES.iter().map(|s| s.to_string()).collect();
        if nodes == default_nodes {
            nodes.shuffle(&mut thread_rng());
        }

        let mut nodes_not_yet_in_net = nodes.clone();
        let num_retries = 10;
        for _ in 0..num_retries {
            nodes_not_yet_in_net.retain(|node| {
                let Ok(Ok(response)) = Request::new()
                    .target(("our", "net", "distro", "sys"))
                    .body(rmp_serde::to_vec(&NetAction::GetPeer(node.clone())).unwrap())
                    .send_and_await_response(1)
                else {
                    return true; // keep the node
                };

                !matches!(
                    rmp_serde::from_slice::<NetResponse>(response.body()),
                    Ok(NetResponse::Peer(Some(_))),
                )
            });
            if nodes_not_yet_in_net.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if !nodes_not_yet_in_net.is_empty() {
            error!("failed to get peering info for {nodes_not_yet_in_net:?}");
        }

        for node in nodes {
            info!("Requesting logs from node: {}", node);

            let cacher_process_address =
                Address::new(&node, ("hypermap-cacher", "hypermap-cacher", "sys"));

            if cacher_process_address == our() {
                continue;
            }

            // ping node for quicker failure if not online/providing/...
            let Ok(Ok(response)) = Request::to(cacher_process_address.clone())
                .body(CacherRequest::GetStatus)
                .send_and_await_response(3)
            else {
                warn!("Node {node} failed to respond to ping; trying next one...");
                continue;
            };
            let Ok(CacherResponse::GetStatus(_)) = response.body().try_into() else {
                warn!("Node {node} failed to respond to ping with expected GetStatus; trying next one...");
                continue;
            };

            // get the logs
            let get_logs_request = GetLogsByRangeRequest {
                from_block: self.last_cached_block + 1,
                to_block: None, // Get all available logs
            };

            match Request::to(cacher_process_address.clone())
                .body(CacherRequest::GetLogsByRange(get_logs_request))
                .send_and_await_response(15)
            {
                Ok(Ok(response_msg)) => match response_msg.body().try_into() {
                    Ok(CacherResponse::GetLogsByRange(Ok(get_logs))) => {
                        match get_logs {
                            GetLogsByRangeOkResponse::Logs((block, json_string)) => {
                                if let Ok(log_caches) =
                                    serde_json::from_str::<Vec<LogCacheInternal>>(&json_string)
                                {
                                    self.process_received_log_caches(log_caches)?;
                                }
                                if block > self.last_cached_block {
                                    self.last_cached_block = block;
                                }
                            }
                            GetLogsByRangeOkResponse::Latest(block) => {
                                if block > self.last_cached_block {
                                    self.last_cached_block = block;
                                }
                            }
                        }
                        return Ok(());
                    }
                    Ok(CacherResponse::GetLogsByRange(Err(e))) => {
                        warn!("Node {} returned error: {}", cacher_process_address, e);
                    }
                    Ok(CacherResponse::IsStarting) => {
                        info!(
                            "Node {} is still starting, trying next node",
                            cacher_process_address
                        );
                    }
                    Ok(CacherResponse::Rejected) => {
                        warn!("Node {} rejected our request", cacher_process_address);
                    }
                    Ok(_) => {
                        warn!(
                            "Node {} returned unexpected response type",
                            cacher_process_address
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse response from {}: {:?}",
                            cacher_process_address, e
                        );
                    }
                },
                Ok(Err(e)) => {
                    warn!("Error response from {}: {:?}", cacher_process_address, e);
                }
                Err(e) => {
                    warn!(
                        "Failed to send request to {}: {:?}",
                        cacher_process_address, e
                    );
                }
            }
        }

        Err(anyhow::anyhow!("Failed to bootstrap from any node"))
    }

    // Process received log caches and write them to VFS
    pub fn process_received_log_caches(
        &mut self,
        log_caches: Vec<LogCacheInternal>,
    ) -> anyhow::Result<()> {
        info!("Processing {} received log caches", log_caches.len());

        for log_cache in log_caches {
            // Validate the log cache signature
            if !self.validate_log_cache(&log_cache)? {
                warn!("Invalid log cache signature, skipping");
                continue;
            }

            // Generate filename from metadata
            let filename = format!(
                "{}-chain{}-from{}-to{}-protocol{}.json",
                log_cache
                    .metadata
                    .time_created
                    .replace(":", "")
                    .replace("-", ""),
                log_cache.metadata.chain_id,
                log_cache.metadata.from_block,
                log_cache.metadata.to_block,
                PROTOCOL_VERSION
            );

            // Write log cache to VFS
            let file_path = format!("{}/{}", self.drive_path, filename);
            let log_cache_bytes = serde_json::to_vec(&log_cache)?;

            let mut file = vfs::open_file(&file_path, true, None)?;
            file.write_all(&log_cache_bytes)?;

            info!("Wrote log cache file: {}", file_path);

            // Update manifest
            let file_hash = format!("0x{}", hex::encode(keccak256(&log_cache_bytes)));
            let manifest_item = ManifestItemInternal {
                metadata: log_cache.metadata.clone(),
                is_empty: log_cache.logs.is_empty(),
                file_hash,
                file_name: filename.clone(),
            };

            self.manifest.items.insert(filename, manifest_item);

            // Update last cached block if this cache goes beyond it
            if let Ok(to_block) = log_cache.metadata.to_block.parse::<u64>() {
                if to_block > self.last_cached_block {
                    self.last_cached_block = to_block;
                }
            }
        }

        // Write updated manifest
        self.write_manifest()?;

        Ok(())
    }

    // Validate a log cache signature
    pub fn validate_log_cache(&self, log_cache: &LogCacheInternal) -> anyhow::Result<bool> {
        let from_block = log_cache.metadata.from_block.parse::<u64>()?;
        let to_block = log_cache.metadata.to_block.parse::<u64>()?;

        let mut bytes_to_verify = serde_json::to_vec(&log_cache.logs)?;
        bytes_to_verify.extend_from_slice(&from_block.to_be_bytes());
        bytes_to_verify.extend_from_slice(&to_block.to_be_bytes());
        let hashed_data = keccak256(&bytes_to_verify);

        let signature_hex = log_cache.metadata.signature.trim_start_matches("0x");
        let signature_bytes = hex::decode(signature_hex)?;

        let created_by_address = log_cache.metadata.created_by.parse::<Address>()?;

        Ok(sign::net_key_verify(
            hashed_data.to_vec(),
            &created_by_address,
            signature_bytes,
        )?)
    }

    // Write manifest to VFS
    pub fn write_manifest(&self) -> anyhow::Result<()> {
        let manifest_bytes = serde_json::to_vec(&self.manifest)?;
        let manifest_path = format!("{}/{}", self.drive_path, self.manifest.manifest_filename);
        let manifest_file = vfs::open_file(&manifest_path, true, None)?;
        manifest_file.write(&manifest_bytes)?;
        info!("Updated manifest file: {}", manifest_path);
        Ok(())
    }

    // Fallback to RPC bootstrap - catch up from where we left off
    pub fn try_bootstrap_from_rpc(&mut self, hypermap: &hypermap::Hypermap) -> anyhow::Result<()> {
        info!(
            "Bootstrapping from RPC, starting from block {}",
            self.last_cached_block + 1
        );

        // Only run RPC bootstrap if we're behind the current chain head
        let current_chain_head = hypermap.provider.get_block_number()?;
        if self.last_cached_block < current_chain_head {
            self.cache_logs_and_update_manifest(hypermap)?;

            // run it twice for fresh boot case:
            // - initial bootstrap takes much time
            // - in that time, the block you are updating to is no longer the head of the chain
            // - so run again to get to the head of the chain
            self.cache_logs_and_update_manifest(hypermap)?;
        } else {
            info!(
                "Already caught up to chain head ({}), no RPC bootstrap needed",
                current_chain_head
            );
        }
        Ok(())
    }

    pub fn to_wit_manifest(&self) -> WitManifest {
        let items = self
            .manifest
            .items
            .iter()
            .map(|(k, v)| {
                let wit_meta = WitLogsMetadata {
                    chain_id: v.metadata.chain_id.clone(),
                    from_block: v.metadata.from_block.clone(),
                    to_block: v.metadata.to_block.clone(),
                    time_created: v.metadata.time_created.clone(),
                    created_by: v.metadata.created_by.clone(),
                    signature: v.metadata.signature.clone(),
                };
                let wit_item = WitManifestItem {
                    metadata: wit_meta,
                    is_empty: v.is_empty,
                    file_hash: v.file_hash.clone(),
                    file_name: v.file_name.clone(),
                };
                (k.clone(), wit_item)
            })
            .collect::<Vec<(String, WitManifestItem)>>();

        WitManifest {
            items,
            manifest_filename: self.manifest.manifest_filename.clone(),
            chain_id: self.manifest.chain_id.clone(),
            protocol_version: self.manifest.protocol_version.clone(),
        }
    }
}
