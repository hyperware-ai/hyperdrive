use std::cmp::{max, min};

use crate::hyperware::process::hypermap_cacher::{
    CacherRequest, CacherResponse, CacherStatus, GetLogsByRangeOkResponse,
};

use hyperware_process_lib::{
    http,
    logging::{error, info, warn},
    our, vfs, Address, Response,
};

use crate::constants::DEFAULT_NODES;
use crate::types::{LogCacheInternal, ManifestInternal, State};
use crate::utils::is_local_request;

pub fn http_handler(
    state: &mut State,
    path: &str,
) -> anyhow::Result<(http::server::HttpResponse, Vec<u8>)> {
    let response = http::server::HttpResponse::new(http::StatusCode::OK)
        .header("Content-Type", "application/json");

    // Basic routing based on path
    Ok(if path == "/manifest" || path == "/manifest.json" {
        let manifest_path = format!("{}/{}", state.drive_path, state.manifest.manifest_filename);
        let manifest_file = vfs::open_file(&manifest_path, true, None)?;
        match manifest_file.read() {
            Ok(content) => (response, content),
            Err(e) => {
                error!(
                    "HTTP: Failed to read manifest file {}: {:?}",
                    manifest_path, e
                );
                (
                    http::server::HttpResponse::new(http::StatusCode::NOT_FOUND),
                    b"Manifest not found".to_vec(),
                )
            }
        }
    } else if path.starts_with("/log-cache/") {
        let filename = path.trim_start_matches("/log-cache/");
        if filename.is_empty() || filename.contains("..") {
            // Basic security check
            return Ok((
                http::server::HttpResponse::new(http::StatusCode::BAD_REQUEST),
                b"Invalid filename".to_vec(),
            ));
        }
        let log_cache_path = format!("{}/{}", state.drive_path, filename);
        let log_cache_file = vfs::open_file(&log_cache_path, true, None)?;
        match log_cache_file.read() {
            Ok(content) => (response, content),
            Err(e) => {
                error!(
                    "HTTP: Failed to read log cache file {}: {:?}",
                    log_cache_path, e
                );
                (
                    http::server::HttpResponse::new(http::StatusCode::NOT_FOUND),
                    b"Log cache file not found".to_vec(),
                )
            }
        }
    } else if path == "/status" {
        let status_info = CacherStatus {
            last_cached_block: state.last_cached_block,
            chain_id: state.chain_id.clone(),
            protocol_version: state.protocol_version.clone(),
            next_cache_attempt_in_seconds: if state.is_cache_timer_live {
                Some(state.cache_interval_s)
            } else {
                None
            },
            manifest_filename: state.manifest.manifest_filename.clone(),
            log_files_count: state.manifest.items.len() as u32,
            our_address: our().to_string(),
            is_providing: state.is_providing,
        };
        match serde_json::to_vec(&status_info) {
            Ok(body) => (response, body),
            Err(e) => {
                error!("HTTP: Failed to serialize status: {:?}", e);
                (
                    http::server::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR),
                    b"Error serializing status".to_vec(),
                )
            }
        }
    } else {
        (
            http::server::HttpResponse::new(http::StatusCode::NOT_FOUND),
            b"Not Found".to_vec(),
        )
    })
}

pub fn handle_request(
    our: &Address,
    source: &Address,
    state: &mut State,
    request: CacherRequest,
) -> anyhow::Result<()> {
    let is_local = is_local_request(our, source);

    // If we're still starting, respond with IsStarting to all requests
    if state.is_starting {
        Response::new().body(CacherResponse::IsStarting).send()?;
        return Ok(());
    }

    if !is_local && source.process.to_string() != "hypermap-cacher:hypermap-cacher:sys" {
        warn!("Rejecting remote request from non-hypermap-cacher: {source}");
        Response::new().body(CacherResponse::Rejected).send()?;
        return Ok(());
    }

    if !is_local
        && !state.is_providing
        && source.process.to_string() == "hypermap-cacher:hypermap-cacher:sys"
    {
        warn!("Rejecting remote request from {source} - not in provider mode");
        Response::new().body(CacherResponse::Rejected).send()?;
        return Ok(());
    }
    let response_body = match request {
        CacherRequest::GetManifest => {
            let manifest_path =
                format!("{}/{}", state.drive_path, state.manifest.manifest_filename);
            if state.manifest.items.is_empty() && vfs::metadata(&manifest_path, None).is_err() {
                CacherResponse::GetManifest(None)
            } else {
                // Ensure manifest is loaded from VFS if state is fresh and manifest file exists
                // This is usually handled by State::load, but as a fallback:
                if state.manifest.items.is_empty() {
                    // If manifest in memory is empty, try to load it
                    let manifest_file = vfs::open_file(&manifest_path, true, None)?;
                    if let Ok(bytes) = manifest_file.read() {
                        if let Ok(disk_manifest) =
                            serde_json::from_slice::<ManifestInternal>(&bytes)
                        {
                            state.manifest = disk_manifest;
                        }
                    }
                }
                CacherResponse::GetManifest(Some(state.to_wit_manifest()))
            }
        }
        CacherRequest::GetLogCacheContent(filename) => {
            let log_cache_path = format!("{}/{}", state.drive_path, filename);
            let log_cache_file = vfs::open_file(&log_cache_path, true, None)?;
            match log_cache_file.read() {
                Ok(content_bytes) => {
                    // Content is raw JSON bytes of LogCacheInternal.
                    // The WIT expects a string.
                    match String::from_utf8(content_bytes) {
                        Ok(content_str) => {
                            CacherResponse::GetLogCacheContent(Ok(Some(content_str)))
                        }
                        Err(e) => {
                            error!("Failed to convert log cache content to UTF-8 string: {}", e);
                            CacherResponse::GetLogCacheContent(Err(format!(
                                "File content not valid UTF-8: {}",
                                e
                            )))
                        }
                    }
                }
                Err(_) => CacherResponse::GetLogCacheContent(Ok(None)),
            }
        }
        CacherRequest::GetStatus => {
            let status = CacherStatus {
                last_cached_block: state.last_cached_block,
                chain_id: state.chain_id.clone(),
                protocol_version: state.protocol_version.clone(),
                next_cache_attempt_in_seconds: if state.is_cache_timer_live {
                    Some(state.cache_interval_s)
                } else {
                    None
                },
                manifest_filename: state.manifest.manifest_filename.clone(),
                log_files_count: state.manifest.items.len() as u32,
                our_address: our.to_string(),
                is_providing: state.is_providing,
            };
            CacherResponse::GetStatus(status)
        }
        CacherRequest::GetLogsByRange(req_params) => {
            let mut relevant_caches: Vec<LogCacheInternal> = Vec::new();
            let req_from_block = req_params.from_block;
            // If req_params.to_block is None, we effectively want to go up to the highest block available in caches.
            // For simplicity in overlap calculation, we can treat None as u64::MAX here.
            let effective_req_to_block = req_params.to_block.unwrap_or(u64::MAX);

            for item in state.manifest.items.values() {
                // Skip items that don't have an actual file (e.g., empty log ranges not written to disk).
                if item.file_name.is_empty() {
                    continue;
                }

                let cache_from = match item.metadata.from_block.parse::<u64>() {
                    Ok(b) => b,
                    Err(_) => {
                        warn!(
                            "Could not parse from_block for cache item {}: {}",
                            item.file_name, item.metadata.from_block
                        );
                        continue;
                    }
                };
                let cache_to = match item.metadata.to_block.parse::<u64>() {
                    Ok(b) => b,
                    Err(_) => {
                        warn!(
                            "Could not parse to_block for cache item {}: {}",
                            item.file_name, item.metadata.to_block
                        );
                        continue;
                    }
                };

                // Check for overlap: max(start1, start2) <= min(end1, end2)
                if max(req_from_block, cache_from) <= min(effective_req_to_block, cache_to) {
                    // This cache file overlaps with the requested range.
                    let file_vfs_path = format!("{}/{}", state.drive_path, item.file_name);
                    match vfs::open_file(&file_vfs_path, false, None) {
                        Ok(file) => match file.read() {
                            Ok(content_bytes) => {
                                match serde_json::from_slice::<LogCacheInternal>(&content_bytes) {
                                    Ok(log_cache) => relevant_caches.push(log_cache),
                                    Err(e) => {
                                        error!(
                                            "Failed to deserialize LogCacheInternal from {}: {:?}",
                                            item.file_name, e
                                        );
                                        // Decide: return error or skip this cache? For now, skip.
                                    }
                                }
                            }
                            Err(e) => error!("Failed to read VFS file {}: {:?}", item.file_name, e),
                        },
                        Err(e) => error!("Failed to open VFS file {}: {e:?}", item.file_name),
                    }
                }
            }

            // Sort caches by their from_block.
            relevant_caches
                .sort_by_key(|cache| cache.metadata.from_block.parse::<u64>().unwrap_or(0));

            if relevant_caches.is_empty() {
                CacherResponse::GetLogsByRange(Ok(GetLogsByRangeOkResponse::Latest(
                    state.last_cached_block,
                )))
            } else {
                match serde_json::to_string(&relevant_caches) {
                    Ok(json_string) => CacherResponse::GetLogsByRange(Ok(
                        GetLogsByRangeOkResponse::Logs((state.last_cached_block, json_string)),
                    )),
                    Err(e) => CacherResponse::GetLogsByRange(Err(format!(
                        "Failed to serialize relevant caches: {e}"
                    ))),
                }
            }
        }
        CacherRequest::StartProviding => {
            if !is_local {
                // should never happen: should be caught in check above
                Response::new().body(CacherResponse::Rejected).send()?;
                return Ok(());
            }
            state.is_providing = true;
            state.save();
            info!("Provider mode enabled");
            CacherResponse::StartProviding(Ok("Provider mode enabled".to_string()))
        }
        CacherRequest::StopProviding => {
            if !is_local {
                Response::new().body(CacherResponse::Rejected).send()?;
                warn!("Rejecting remote request from {source} to alter provider mode");
                return Ok(());
            }
            state.is_providing = false;
            state.save();
            info!("Provider mode disabled");
            CacherResponse::StopProviding(Ok("Provider mode disabled".to_string()))
        }
        CacherRequest::SetNodes(new_nodes) => {
            if !is_local {
                Response::new().body(CacherResponse::Rejected).send()?;
                warn!("Rejecting remote request from {source} to set nodes");
                return Ok(());
            }
            state.nodes = new_nodes;
            state.save();
            info!("Nodes updated to: {:?}", state.nodes);
            CacherResponse::SetNodes(Ok("Nodes updated successfully".to_string()))
        }
        CacherRequest::Reset(custom_nodes) => {
            if !is_local {
                Response::new().body(CacherResponse::Rejected).send()?;
                warn!("Rejecting remote request from {source} to reset");
                return Ok(());
            }

            info!("Resetting hypermap-cacher state and clearing VFS...");

            // Clear all files from the drive
            if let Err(e) = state.clear_drive() {
                error!("Failed to clear drive during reset: {:?}", e);
                CacherResponse::Reset(Err(format!("Failed to clear drive: {:?}", e)))
            } else {
                // Create new state with custom nodes if provided, otherwise use defaults
                let nodes = match custom_nodes {
                    Some(nodes) => nodes,
                    None => DEFAULT_NODES.iter().map(|s| s.to_string()).collect(),
                };

                *state = State::new(&state.drive_path);
                state.nodes = nodes;
                state.save();

                info!(
                    "Hypermap-cacher reset complete. New nodes: {:?}",
                    state.nodes
                );
                CacherResponse::Reset(Ok(
                    "Reset completed successfully. Cacher will restart with new settings."
                        .to_string(),
                ))
            }
        }
    };

    Response::new().body(response_body).send()?;
    Ok(())
}
