use serde::{Deserialize, Serialize};

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type Result<T> = std::result::Result<T, DynError>;

#[derive(Clone)]
pub struct Config {
    pub ws_endpoint: String,
    pub rpc_endpoint: String,
    pub v3_pool_address: String,
    pub checkpoint_file: String,
    pub csv_file: String,
    pub max_reorg_depth: u64,
    pub rate_limit: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenData {
    pub chain: String,
    pub market: String,
    pub version: String,
    pub token_symbol: String,
    pub token_address: String,
    pub block: u64,
    pub total_supply: String,
    pub total_borrow: String,
    pub utilization_rate: f64,
    pub supply_rate: f64,
    pub borrow_rate: f64,
}
