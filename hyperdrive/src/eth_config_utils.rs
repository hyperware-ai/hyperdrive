use lib::eth::{ProviderConfig, SavedConfigs};

pub fn add_provider_to_config(
    eth_provider_config: &mut SavedConfigs,
    new_provider: ProviderConfig,
) {
    match &new_provider.provider {
        lib::eth::NodeOrRpcUrl::RpcUrl { url, .. } => {
            // Remove any existing provider with this URL
            eth_provider_config.0.retain(|config| {
                if let lib::eth::NodeOrRpcUrl::RpcUrl {
                    url: existing_url, ..
                } = &config.provider
                {
                    existing_url != url
                } else {
                    true
                }
            });
        }
        lib::eth::NodeOrRpcUrl::Node { hns_update, .. } => {
            // Remove any existing provider with this node name
            eth_provider_config.0.retain(|config| {
                if let lib::eth::NodeOrRpcUrl::Node {
                    hns_update: existing_update,
                    ..
                } = &config.provider
                {
                    existing_update.name != hns_update.name
                } else {
                    true
                }
            });
        }
    }

    // Insert the new provider at the front (position 0)
    eth_provider_config.0.insert(0, new_provider);
}

/// Extract unauthenticated RPC URLs from SavedConfigs
pub fn extract_rpc_url_providers_for_default_chain(
    saved_configs: &lib::eth::SavedConfigs,
) -> Vec<lib::eth::NodeOrRpcUrl> {
    saved_configs
        .0
        .iter()
        .filter_map(|provider_config| {
            // Only include providers for the default chain (8453 for mainnet, 31337 for simulation)
            #[cfg(not(feature = "simulation-mode"))]
            let target_chain_id = crate::CHAIN_ID; // 8453
            #[cfg(feature = "simulation-mode")]
            let target_chain_id = crate::CHAIN_ID; // 31337

            if provider_config.chain_id != target_chain_id {
                return None;
            }

            match &provider_config.provider {
                lib::eth::NodeOrRpcUrl::RpcUrl { url, auth } => {
                    // Return the full RpcUrl enum variant with both url and auth
                    Some(lib::eth::NodeOrRpcUrl::RpcUrl {
                        url: url.clone(),
                        auth: auth.clone(),
                    })
                }
                lib::eth::NodeOrRpcUrl::Node { .. } => None, // Skip node providers
            }
        })
        .collect()
}
