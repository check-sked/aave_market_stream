use std::fs::OpenOptions;
use std::sync::Arc;

use csv::Writer;
use tokio::sync::Mutex;

use ethabi::Token;
use web3::contract::{Contract, Options};
use web3::transports::Http;
use web3::types::{Address, CallRequest, U256, Bytes};
use web3::Web3;

use crate::block_manager::BlockManager;
use crate::types::{Config, Result, TokenData};
use crate::utils::{calculate_utilization, ray_to_percent};

pub struct DataProcessor {
    config: Arc<Config>,
    web3: Web3<Http>,

    block_manager: Arc<BlockManager>,
    csv_writer: Arc<Mutex<Writer<std::fs::File>>>,

    /// Aave Data Provider contract in main
    data_provider_contract: Contract<Http>,
}

impl DataProcessor {
    pub fn new(config: &Config, block_manager: Arc<BlockManager>) -> Self {
        let transport = Http::new(&config.rpc_endpoint).unwrap();
        let web3 = Web3::new(transport.clone());

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.csv_file)
            .expect("[DataProcessor] Failed to open CSV file");
        let writer = csv::Writer::from_writer(file);

        println!("[DataProcessor::new] Writing to CSV file: {}", config.csv_file);

        let abi_data = std::fs::read("src/aave_pool_abi.json")
            .expect("Failed to read src/aave_pool_abi.json");

        let data_provider_contract = Contract::from_json(
            web3.eth(),
            config.v3_pool_address.parse::<Address>().unwrap(),
            &abi_data
        ).expect("Failed to parse DataProvider ABI");

        Self {
            config: Arc::new(config.clone()),
            web3,
            block_manager,
            csv_writer: Arc::new(Mutex::new(writer)),
            data_provider_contract,
        }
    }

    pub async fn run(&self) -> Result<()> {
        println!("[DataProcessor::run] Started Data Processor loop.");

        loop {
            let maybe_block = {
                let mut blocks = self.block_manager.missing_blocks.lock().await;
                blocks.pop()
            };

            match maybe_block {
                Some(block_number) => {
                    // 1) Get the list of (symbol, address) from getAllReservesTokens
                    let all_reserves = self.get_all_reserves_tokens(block_number).await?;
                    println!(
                        "[DataProcessor] Found {} reserves at block {}",
                        all_reserves.len(),
                        block_number
                    );

                    // 2) For each reserve, call getReserveData
                    for (symbol, token_address) in all_reserves {
                        match self.fetch_reserve_data(block_number, &symbol, token_address).await {
                            Ok(_) => {
                                println!(
                                    "[DataProcessor] Wrote data for {} at block {}.",
                                    symbol, block_number
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "[DataProcessor] Error fetching data for {} at block {}: {}",
                                    symbol, block_number, e
                                );
                            }
                        }
                    }
                }
                None => {
                    println!("[DataProcessor::run] No blocks in queue. Sleeping...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }
        // Ok(())
    }

    /// Manually call "getAllReservesTokens" via `eth_call`, decode (string, address)[] from the output.
    async fn get_all_reserves_tokens(&self, block_number: u64) -> Result<Vec<(String, Address)>> {
        println!(
            "[DataProcessor::get_all_reserves_tokens] block={}, building manual eth_call...",
            block_number
        );

        // 1) Grab the function from the contract ABI
        let function = self
            .data_provider_contract
            .abi()
            .function("getAllReservesTokens")?;

        // 2) Encode the call data with zero parameters
        let encoded_data = function.encode_input(&[])?; // no params

        // 3) Build eth_call request
        let contract_addr = self.data_provider_contract.address();
        let call_req = CallRequest {
            from: None,
            to: Some(contract_addr),
            gas: None,
            gas_price: None,
            value: None,
            data: Some(Bytes(encoded_data)),
            transaction_type: None,
            // *** Fields required in web3 0.19 - set to None. ***
            access_list: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        // 4) Perform the call at the given block (or None for latest).
        let raw_bytes = self
            .web3
            .eth()
            .call(call_req, None)
            .await?;

        // 5) Decode output tokens using the ABI function
        let tokens = function.decode_output(&raw_bytes.0)?;
        let first_token = tokens
            .into_iter()
            .next()
            .ok_or("No output tokens from getAllReservesTokens!")?;

        let mut reserves: Vec<(String, Address)> = Vec::new();

        match first_token {
            ethabi::Token::Array(arr) => {
                for item in arr {
                    match item {
                        ethabi::Token::Tuple(fields) if fields.len() == 2 => {
                            let sym_tok = &fields[0];
                            let addr_tok = &fields[1];

                            if let (ethabi::Token::String(sym), ethabi::Token::Address(a)) =
                                (sym_tok, addr_tok)
                            {
                                reserves.push((sym.clone(), *a));
                            } else {
                                return Err("Tuple fields not (String, Address).".into());
                            }
                        }
                        _ => {
                            return Err(
                                "Expected an array of 2-field tuples (string, address).".into()
                            );
                        }
                    }
                }
            }
            other => {
                return Err(format!("Expected Token::Array, got {:?}", other).into());
            }
        }

        Ok(reserves)
    }

    async fn fetch_reserve_data(&self, block_number: u64, symbol: &str, asset: Address) -> Result<()> {
        #[allow(non_snake_case)]
        let (
            unbacked,
            accruedToTreasuryScaled,
            totalAToken,
            totalStableDebt,
            totalVariableDebt,
            liquidityRate,
            variableBorrowRate,
            stableBorrowRate,
            averageStableBorrowRate,
            liquidityIndex,
            variableBorrowIndex,
            lastUpdateTimestamp
        ): (U256, U256, U256, U256, U256, U256, U256, U256, U256, U256, U256, u64) = self
            .data_provider_contract
            .query(
                "getReserveData",
                (asset,),
                None, // or Some(block_number.into())
                Options::default(),
                None
            )
            .await?;

        let supply_rate_percent = ray_to_percent(liquidityRate.as_u128());
        let borrow_rate_percent = ray_to_percent(variableBorrowRate.as_u128());

        let total_supply = totalAToken;
        let total_borrow = totalStableDebt + totalVariableDebt;

        let token_data = TokenData {
            chain: "Base Mainnet".to_string(),
            market: "Aave".to_string(),
            version: "3".to_string(),
            token_symbol: symbol.to_string(),
            token_address: format!("{:?}", asset),
            block: block_number,
            total_supply: total_supply.to_string(),
            total_borrow: total_borrow.to_string(),
            utilization_rate: calculate_utilization(
                total_borrow.as_u128(),
                total_supply.as_u128(),
            ),
            supply_rate: supply_rate_percent,
            borrow_rate: borrow_rate_percent,
        };

        let mut writer = self.csv_writer.lock().await;
        writer.serialize(&token_data)?;
        writer.flush()?;

        println!(
            "[DataProcessor::fetch_reserve_data] block={} symbol={} => unbacked={}, accruedToTreasuryScaled={}, stableBorrowRate={}, averageStableBorrowRate={}, lastUpdateTimestamp={}",
            block_number, symbol, unbacked, accruedToTreasuryScaled, stableBorrowRate, averageStableBorrowRate, lastUpdateTimestamp
        );

        Ok(())
    }
}
