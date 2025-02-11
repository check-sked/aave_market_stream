use std::sync::Arc;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;

use crate::block_manager::BlockManager;
use crate::types::{Config, Result};

pub struct WebSocketManager {
    config: Arc<Config>,
    block_manager: Arc<BlockManager>,
}

impl WebSocketManager {
    pub fn new(config: &Config, block_manager: Arc<BlockManager>) -> Self {
        Self {
            config: Arc::new(config.clone()),
            block_manager,
        }
    }

    pub async fn run(&self) -> Result<()> {
        println!(
            "[WebSocketManager::run] Connecting to WebSocket endpoint: {}",
            self.config.ws_endpoint
        );

        let url = url::Url::parse(&self.config.ws_endpoint)?;

        loop {
            match connect_async(url.clone()).await {
                Ok((ws_stream, _)) => {
                    println!("[WebSocketManager] Connected successfully!");
                    let (mut write, mut read) = ws_stream.split();

                    // Subscribe to new heads
                    let subscribe_msg = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "eth_subscribe",
                        "params": ["newHeads"]
                    });

                    println!("[WebSocketManager] Subscribing to newHeads...");
                    write.send(tokio_tungstenite::tungstenite::Message::Text(
                        subscribe_msg.to_string()
                    )).await?;

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                // Debug logging
                                println!("[WebSocketManager] Received text: {}", text);

                                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if let Some(params) = value.get("params") {
                                        if let Some(block_hex) = params.get("result")
                                            .and_then(|r| r.get("number"))
                                            .and_then(|n| n.as_str())
                                        {
                                            let block_number = u64::from_str_radix(
                                                &block_hex.trim_start_matches("0x"),
                                                16
                                            )?;

                                            // Push block into missing_blocks
                                            let mut missing_blocks =
                                                self.block_manager.missing_blocks.lock().await;
                                            missing_blocks.push(block_number);

                                            println!(
                                                "[WebSocketManager] Pushed block {} to missing_blocks",
                                                block_number
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[WebSocketManager] WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    println!("[WebSocketManager] WebSocket disconnected. Reconnecting soon...");
                }
                Err(e) => {
                    eprintln!("[WebSocketManager] Connection error: {}", e);
                }
            }

            // Wait
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        // Ok(())
    }
}
