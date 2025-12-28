use std::error::Error;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;
use std::thread;

// Struct to hold result
#[derive(Debug, Clone)]
pub struct AddressResult {
    pub address: String,
    pub final_balance: u64,
    pub total_received: u64,
}

#[derive(Clone)]
pub enum ProviderType {
    BlockchainInfo,
    Esplora, // Mempool.space, Blockstream.info
}

pub struct RpcChecker {
    client: Client,
    url: String,
    provider_type: ProviderType,
}

#[derive(Deserialize, Debug)]
struct BlockchainInfoMultiAddr {
    addresses: Vec<BlockchainInfoAddress>,
}

#[derive(Deserialize, Debug)]
struct BlockchainInfoAddress {
    address: String,
    final_balance: u64,
    total_received: u64,
}

#[derive(Deserialize, Debug)]
struct EsploraStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

#[derive(Deserialize, Debug)]
struct EsploraAddress {
    address: String,
    chain_stats: EsploraStats,
    mempool_stats: EsploraStats,
}

impl RpcChecker {
    pub fn new(url: Option<String>, _user: Option<String>, _pass: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10)) 
            .build()
            .unwrap_or_default();
        
        let (actual_url, p_type) = match url {
            Some(u) => {
                if u.contains("blockchain.info") {
                    (u, ProviderType::BlockchainInfo)
                } else {
                    (u, ProviderType::Esplora)
                }
            },
            None => ("https://blockchain.info".to_string(), ProviderType::BlockchainInfo),
        };

        RpcChecker {
            client,
            url: actual_url,
            provider_type: p_type,
        }
    }

    pub fn check_batch(&self, addresses: &[String]) -> Result<Vec<AddressResult>, Box<dyn Error>> {
        match self.provider_type {
            ProviderType::BlockchainInfo => self.check_batch_blockchain_info(addresses),
            ProviderType::Esplora => self.check_batch_esplora(addresses),
        }
    }

    fn check_batch_blockchain_info(&self, addresses: &[String]) -> Result<Vec<AddressResult>, Box<dyn Error>> {
        let joined_addrs = addresses.join("|");
        let base = self.url.trim_end_matches('/');
        let url = format!("{}/multiaddr?active={}", base, joined_addrs);

        thread::sleep(Duration::from_millis(500));

        // Propagate Error if request fails (Offline)
        let resp = self.client.get(&url).send()?;
        
        if !resp.status().is_success() {
             return Err(format!("Blockchain.info Error: {}", resp.status()).into());
        }

        let data: BlockchainInfoMultiAddr = resp.json()?;
        let mut results = Vec::new();

        for addr_data in data.addresses {
            if addr_data.total_received > 0 {
                results.push(AddressResult {
                    address: addr_data.address,
                    final_balance: addr_data.final_balance,
                    total_received: addr_data.total_received,
                });
            }
        }
        Ok(results)
    }

    fn check_batch_esplora(&self, addresses: &[String]) -> Result<Vec<AddressResult>, Box<dyn Error>> {
        let mut results = Vec::new();
        let base = self.url.trim_end_matches('/');
        let mut consecutive_failures = 0;

        for addr in addresses {
            let url = format!("{}/api/address/{}", base, addr);
            
            thread::sleep(Duration::from_millis(200)); 

            // Try to make the request
            match self.client.get(&url).send() {
                Ok(resp) => {
                    consecutive_failures = 0; // Reset on success
                    
                    if resp.status().is_success() {
                        if let Ok(data) = resp.json::<EsploraAddress>() {
                             let total_received = data.chain_stats.funded_txo_sum + data.mempool_stats.funded_txo_sum;
                             let total_spent = data.chain_stats.spent_txo_sum + data.mempool_stats.spent_txo_sum;
                             let final_balance = total_received.saturating_sub(total_spent);
                             
                             if total_received > 0 {
                                 results.push(AddressResult {
                                     address: data.address,
                                     final_balance,
                                     total_received,
                                 });
                             }
                        }
                    } else if resp.status().as_u16() == 429 {
                        // Rate limit - wait longer
                        thread::sleep(Duration::from_secs(2));
                    }
                    // Other HTTP errors: just skip this address
                },
                Err(_) => {
                    consecutive_failures += 1;
                    
                    // If we get 10 consecutive failures, assume full network down
                    if consecutive_failures >= 10 {
                        return Err("Network appears down (10 consecutive failures)".into());
                    }
                    
                    // Otherwise, just skip this address and continue
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        
        Ok(results)
    }
}
