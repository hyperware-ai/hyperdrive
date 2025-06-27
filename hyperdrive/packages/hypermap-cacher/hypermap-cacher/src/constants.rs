pub const PROTOCOL_VERSION: &str = "0";
pub const DEFAULT_BLOCK_BATCH_SIZE: u64 = 500;
pub const DEFAULT_CACHE_INTERVAL_S: u64 = 2 * 500; // 500 blocks, 2s / block = 1000s;
pub const MAX_LOG_RETRIES: u8 = 3;
pub const RETRY_DELAY_S: u64 = 10;
pub const LOG_ITERATION_DELAY_MS: u64 = 200;

#[cfg(not(feature = "simulation-mode"))]
pub const DEFAULT_NODES: &[&str] = &["nick.hypr", "nick1udwig.os"];
#[cfg(feature = "simulation-mode")]
pub const DEFAULT_NODES: &[&str] = &["fake.os"];
