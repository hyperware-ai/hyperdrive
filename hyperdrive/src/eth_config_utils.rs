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
