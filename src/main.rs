use tokio;

mod block_manager;
mod data_processor;
mod types;
mod utils;
mod websocket_manager;

use crate::types::{Config, Result};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    println!("[main] Starting up...");

    let config = Config {
        ws_endpoint: "wss://base-mainnet.g.alchemy.com/v2/_API_KEY_".into(),
        rpc_endpoint: "https://base-mainnet.g.alchemy.com/v2/_API_KEY_".into(),
        v3_pool_address: "0xd82a47fdebB5bf5329b09441C3DaB4b5df2153Ad".into(),
        checkpoint_file: "./last_block.json".into(),
        csv_file: "./aave_base_v3_core.csv".into(),
        max_reorg_depth: 12,
        rate_limit: 10,
    };

    let block_manager = block_manager::BlockManager::new(&config).await?;
    let websocket_manager = websocket_manager::WebSocketManager::new(&config, block_manager.clone());
    let data_processor = data_processor::DataProcessor::new(&config, block_manager.clone());

    let handles = vec![
        tokio::spawn(async move { websocket_manager.run().await }),
        tokio::spawn(async move { data_processor.run().await }),
        tokio::spawn(async move { block_manager.run().await }),
    ];

    for handle in handles {
        handle.await??;
    }

    println!("[main] All tasks finished. Exiting.");
    Ok(())
}
