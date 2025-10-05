use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;

// Import the types we need from the eth module
use lib::eth::{NodeOrRpcUrl, ProviderConfig, SavedConfigs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsConfig {
    pub cache_sources: Vec<String>,
    pub base_l2_providers: Vec<String>,
}

impl Default for OptionsConfig {
    fn default() -> Self {
        Self {
            cache_sources: Vec::new(),
            base_l2_providers: Vec::new(),
        }
    }
}

const OPTIONS_CONFIG_FILE: &str = ".options_config.json";

/// Global home directory path, initialized once on first access
static HOME_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    // First try environment variable
    if let Ok(home_path) = std::env::var("HYPERDRIVE_HOME") {
        return PathBuf::from(home_path);
    }

    // Fall back to default location in user's home directory
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hyperdrive")
});

/// Initialize the home directory path explicitly (must be called before first use of config functions)
pub fn initialize_home_directory(home_directory_path: PathBuf) {
    std::env::set_var(
        "HYPERDRIVE_HOME",
        home_directory_path.to_string_lossy().to_string(),
    );
}

/// Get the configured home directory path
pub fn get_home_directory() -> &'static PathBuf {
    &HOME_DIR
}

/// Load the options configuration from the configured home directory.
/// Returns default config if file doesn't exist or can't be parsed.
pub async fn load_options_config() -> OptionsConfig {
    let config_path = HOME_DIR.join(OPTIONS_CONFIG_FILE);

    match tokio::fs::read_to_string(&config_path).await {
        Ok(contents) => match serde_json::from_str::<OptionsConfig>(&contents) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Warning: Failed to parse options config: {}", e);
                OptionsConfig::default()
            }
        },
        Err(_) => {
            // File doesn't exist, return defaults
            OptionsConfig::default()
        }
    }
}

/// Save the options configuration to the configured home directory.
pub async fn save_options_config(config: &OptionsConfig) -> Result<(), std::io::Error> {
    let config_path = HOME_DIR.join(OPTIONS_CONFIG_FILE);

    // Ensure the directory exists
    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let contents = serde_json::to_string_pretty(config)?;
    tokio::fs::write(config_path, contents).await
}

/// Update the options configuration with new values for both fields.
/// This is the function that should be called when you want to update both cache sources and base L2 providers.
pub async fn update_options_config(
    current_cache_sources: Vec<String>,
    current_base_l2_providers: Vec<String>,
) -> Result<(), std::io::Error> {
    let config = OptionsConfig {
        cache_sources: current_cache_sources,
        base_l2_providers: current_base_l2_providers,
    };

    save_options_config(&config).await?;
    println!("Options configuration saved to {}", OPTIONS_CONFIG_FILE);
    Ok(())
}

/// Update only the base L2 providers, keeping existing cache sources unchanged.
/// This loads the current config, updates only the base_l2_providers field, and saves it back.
pub async fn update_base_l2_providers(
    new_base_l2_providers: Vec<String>,
) -> Result<(), std::io::Error> {
    let mut config = load_options_config().await;
    config.base_l2_providers = new_base_l2_providers;

    save_options_config(&config).await?;
    println!("Base L2 providers updated in {}", OPTIONS_CONFIG_FILE);
    Ok(())
}

/// Update base L2 providers from a SavedConfigs object, extracting only RPC URLs without authentication.
/// This filters the SavedConfigs to find RpcUrl providers that have no auth (None),
/// extracts their URLs, and saves them as the new base L2 providers.
/// Keeps existing cache sources unchanged.
pub async fn update_base_l2_providers_from_saved_configs(
    saved_configs: &SavedConfigs,
) -> Result<(), std::io::Error> {
    let mut config = load_options_config().await;

    // Extract URLs from RpcUrl providers that have no authentication
    let unauthenticated_urls: Vec<String> = saved_configs
        .0
        .iter()
        .filter_map(|provider_config| {
            match &provider_config.provider {
                NodeOrRpcUrl::RpcUrl { url, auth } => {
                    // Only include URLs that have no authentication
                    if auth.is_none() {
                        Some(url.clone())
                    } else {
                        None
                    }
                }
                NodeOrRpcUrl::Node { .. } => None, // Skip node providers
            }
        })
        .collect();

    config.base_l2_providers = unauthenticated_urls;

    save_options_config(&config).await?;
    println!(
        "Base L2 providers updated from SavedConfigs (unauthenticated RPC URLs only) in {}",
        OPTIONS_CONFIG_FILE
    );
    Ok(())
}

/// Update only the cache sources, keeping existing base L2 providers unchanged.
/// This loads the current config, updates only the cache_sources field, and saves it back.
pub async fn update_cache_sources(new_cache_sources: Vec<String>) -> Result<(), std::io::Error> {
    let mut config = load_options_config().await;
    config.cache_sources = new_cache_sources;

    save_options_config(&config).await?;
    println!("Cache sources updated in {}", OPTIONS_CONFIG_FILE);
    Ok(())
}

/// Update the options configuration with optional values.
/// Pass None to keep the existing value for that field, or Some(vec) to update it.
pub async fn update_options_config_partial(
    cache_sources: Option<Vec<String>>,
    base_l2_providers: Option<Vec<String>>,
) -> Result<(), std::io::Error> {
    let mut config = load_options_config().await;

    if let Some(new_cache_sources) = cache_sources {
        config.cache_sources = new_cache_sources;
    }

    if let Some(new_base_l2_providers) = base_l2_providers {
        config.base_l2_providers = new_base_l2_providers;
    }

    save_options_config(&config).await?;
    println!("Options configuration updated in {}", OPTIONS_CONFIG_FILE);
    Ok(())
}
