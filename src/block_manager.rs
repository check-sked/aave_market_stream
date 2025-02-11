use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::{Config, Result};

pub struct BlockManager {
    pub config: Arc<Config>,
    pub last_processed_block: Arc<Mutex<u64>>,
    pub missing_blocks: Arc<Mutex<BinaryHeap<u64>>>,
}

impl BlockManager {
    pub async fn new(config: &Config) -> Result<Arc<Self>> {
        println!(
            "[BlockManager::new] Using checkpoint file: {}",
            config.checkpoint_file
        );

        Ok(Arc::new(Self {
            config: Arc::new(config.clone()),
            last_processed_block: Arc::new(Mutex::new(0)),
            missing_blocks: Arc::new(Mutex::new(BinaryHeap::new())),
        }))
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        println!("[BlockManager::run] Started block manager loop.");

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            println!("[BlockManager::run] Still running...");
        }
        // Ok(())
    }
}
