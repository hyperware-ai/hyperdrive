use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;

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

/// Update the options configuration with new values.
/// This is the function that should be called from anywhere in the codebase
/// when the runtime state changes and needs to be persisted.
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
