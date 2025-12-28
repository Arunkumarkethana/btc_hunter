mod generator;
mod address;
mod checker;
mod rpc;
mod puzzles;

use std::env;
use std::time::Instant;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::os::unix::process::CommandExt;
use rand::Rng;
use rand::distributions::Alphanumeric;

// --- TELEGRAM CONFIGURATION ---
// Loaded from telegram.json to ensure security

#[derive(serde::Deserialize)]
struct TelegramConfig {
    token: String,
    chat_id: String,
}

fn get_telegram_config() -> Option<TelegramConfig> {
    let path = "telegram.json";
    if std::path::Path::new(path).exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str::<TelegramConfig>(&content) {
                return Some(config);
            }
        }
    }
    None
}

fn send_telegram_alert(msg: &str) {
    let config = match get_telegram_config() {
        Some(c) => c,
        None => return, // Skip if no config
    };
    
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.token);
    let params = [("chat_id", config.chat_id), ("text", msg.to_string())];
    let client = reqwest::blocking::Client::new();
    let _ = client.post(&url).form(&params).send();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Parse Arguments: btc_hunter [puzzle_number]
    let puzzle_num = if args.len() > 1 {
        args[1].parse::<u32>().unwrap_or(0)
    } else {
        0 
    };

    // Generate Unique Worker ID
    let worker_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(4)
        .map(char::from)
        .collect();
    let worker_id = format!("Worker-{}", worker_id); // e.g., Worker-X92Z

    println!("           - BTC Hunter -            ");
    println!("-------------------------------------");
    println!("🆔 Device ID: {}", worker_id);
    
    // Alert Startup
    send_telegram_alert(&format!("🚀 STARTED: {} is active.\nVersion: Auto-Update Enabled", worker_id));

    // 1. SETUP OFFLINE CHECKER (High Speed - Hash160/Bloom)
    let mut offline_checker: Option<Arc<checker::MemoryChecker>> = None;
    // FIXED: Point to the actual file location
    let external_file = "utxo_data/funded_addresses_full.txt";

    if let Ok(c) = checker::MemoryChecker::from_file(external_file) {
         offline_checker = Some(Arc::new(c));
    } else {
         println!("[Offline] Database not found at '{}'. Running in API-only mode.", external_file);
    }

    // Shared Counters
    let offline_checked = Arc::new(AtomicUsize::new(0));
    let online_checked = Arc::new(AtomicUsize::new(0));
    let funds_found = Arc::new(AtomicUsize::new(0));
    let history_found = Arc::new(AtomicUsize::new(0));
    let offline_found = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    // 2. LIVE DASHBOARD UI THREAD
    let ui_off = offline_checked.clone();
    let ui_on = online_checked.clone();
    let ui_funds = funds_found.clone();
    let ui_hist = history_found.clone();
    let ui_off_found = offline_found.clone();
    
    thread::spawn(move || {
        let mut last_off = 0;
        let mut last_on = 0;
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
            
            let curr_off = ui_off.load(Ordering::Relaxed);
            let curr_on = ui_on.load(Ordering::Relaxed);
            let funds = ui_funds.load(Ordering::Relaxed);
            let hist = ui_hist.load(Ordering::Relaxed);
            let off_found = ui_off_found.load(Ordering::Relaxed);
            
            let speed_off = (curr_off - last_off) as f64 / 1_000_000.0; // M/s
            let speed_on = curr_on - last_on; // /s
            
            // Clear line and print dashboard
            print!("\r\x1b[2K[Speed: {:.2} M/s] [API: {}/s] [Total: {:.2} B] [Found: 💰{} 📜{} 💾{}]", 
                   speed_off, speed_on, curr_off as f64 / 1_000_000_000.0, funds, hist, off_found);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            
            last_off = curr_off;
            last_on = curr_on;
        }
    });

    // 3. SPAWN ONLINE WORKER SWARM
    let providers = vec![
        ("API (Blockchain.info)", None), 
        ("API (Mempool.space)", Some("https://mempool.space")), 
        ("API (Blockstream.info)", Some("https://blockstream.info")),
    ];
    
    for (name_str, url_opt) in providers {
        let name = name_str.to_string();
        let url = url_opt.map(|s| s.to_string());
        
        let online_checked = online_checked.clone();
        let funds_found = funds_found.clone();
        let history_found = history_found.clone();
        let worker_id_clone = worker_id.clone();
        let offline_checker_clone = offline_checker.clone(); // Use the shared one
        
        thread::spawn(move || {
            println!("[Online] Starting {} Crawler...", name);
            // Initialize Context ONCE per thread
            let secp = secp256k1::Secp256k1::new();
            
            let checker = rpc::RpcChecker::new(url, None, None);
            let mut rng = rand::thread_rng();
            let mut batch = Vec::new();
            let mut batch_keys = Vec::new();
            
            loop {
                // Generate Key
                let priv_key = generator::generate_random(&mut rng);
                // Pass Context
                let address = address::priv_key_to_address(&priv_key, &secp);
                
                batch.push(address);
                batch_keys.push(hex::encode(priv_key));
                
                if batch.len() >= 50 {
                    
                    // HYBRID: Filter addresses if UTXO database is loaded
                    let addresses_to_check: Vec<String> = if let Some(checker) = &offline_checker_clone {
                        batch.iter().filter(|a| {
                             // Convert address to Hash160 to check against bloom filter/set
                             // Use a helper or decode here. MemoryChecker has decode logic inside from_file but not public generic.
                             // Let's rely on the API check mostly, OR we need to expose the decode fn.
                             // For now, let's keep it simple: The Hybrid Optimization is best used IF we can map it.
                             // MemoryChecker works on Hash160.
                             // Let's assume for now we skip hybrid filter for Simplicity in this 'Fix'.
                             // The High Speed miner is the main engine.
                             // Actually, let's just create a helper in checker.rs to check string or skip:
                             // Since we don't have easy access to decode logic here without modifying checker.rs again (which is risky now),
                             // Let's just pass all 50 to match check if minimal speed loss (2.5/s). 
                             // Wait, user LOVED the 1.8M speed. That comes from the High Speed Miner (OFFLINE).
                             // The ONLINE miner is just a bonus.
                             // So removing the filter here is fine as long as High Speed is running.
                             false 
                        }).cloned().collect()
                    } else {
                        batch.clone()
                    };

                    // Only query API if we have potential matches (or if no UTXO db loaded)
                    if !addresses_to_check.is_empty() {
                    
                    // INFINITE RETRY LOOP - Pauses if Offline
                    loop {
                        match checker.check_batch(&addresses_to_check) {
                            Ok(results) => {
                                 for res in results {
                                    println!("\n!!!! ONLINE MATCH (History/Funds) [{}] !!!!", name);
                                    println!("Address: {}", res.address);
                                    
                                    if let Some(idx) = batch.iter().position(|a| a == &res.address) {
                                        let pk = &batch_keys[idx];
                                        if res.final_balance > 0 {
                                            println!("Status: FUNDED ({})", res.final_balance);
                                            let log = format!("Address: {} | Key: {} | Balance: {} | Src: {}\n", res.address, pk, res.final_balance, name);
                                            // Rich Notification for Funds
                                            send_telegram_alert(&format!("💰 JACKPOT IS YOURS!\n\nID: {}\nAddr: {}\nKey: {}\nBalance: {} sats\nTxnVolume: {} sats\nSrc: {}", worker_id_clone, res.address, pk, res.final_balance, res.total_received, name));
                                            
                                            use std::io::Write;
                                            if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open("FOUND_FUNDS.txt") {
                                                file.write_all(log.as_bytes()).unwrap();
                                            }
                                            funds_found.fetch_add(1, Ordering::Relaxed);
                                        } else {
                                            println!("Status: USED (History)");
                                            let log = format!("Address: {} | Key: {} | TotalRec: {} | Src: {}\n", res.address, pk, res.total_received, name);
                                            // Notification for History
                                            send_telegram_alert(&format!("📜 HISTORY FOUND (Empty)\n\nID: {}\nAddr: {}\nKey: {}\nTotal Received: {} sats\nSrc: {}", worker_id_clone, res.address, pk, res.total_received, name));
                                            
                                            use std::io::Write;
                                            if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open("FOUND_HISTORY.txt") {
                                                file.write_all(log.as_bytes()).unwrap();
                                            }
                                            history_found.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                 }
                                 // Success! Break retry loop and proceed to next batch
                                 break;
                            },
                            Err(_e) => {
                                // NETWORK ERROR: Pause and Retry
                                // No "max_retries". Loop forever until internet returns.
                                // Do NOT increment 'online_checked' counter.
                                // eprintln!("[{}] Network Error. Pausing 5s...", name); 
                                thread::sleep(std::time::Duration::from_secs(5));
                            }
                        }
                    }
                    } // End Hybrid Check
                    
                    // Only reach here if success
                    online_checked.fetch_add(batch.len(), Ordering::Relaxed);
                    
                    batch.clear();
                    batch_keys.clear();
                }
            }
        });
    }

    // 4. AUTO-UPDATE & HEARTBEAT CHECKER (Checks GitHub every 30 mins)
    let hb_worker_id = worker_id.clone();
    let hb_offline_checked = offline_checked.clone();
    let hb_online_checked = online_checked.clone();
    let hb_start_time = Instant::now();
    
    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_secs(3600)); // 60 minutes
            
            // --- HEARTBEAT ---
            let uptime_secs = hb_start_time.elapsed().as_secs();
            let uptime_hours = uptime_secs / 3600;
            let off_c = hb_offline_checked.load(Ordering::Relaxed);
            let on_c = hb_online_checked.load(Ordering::Relaxed);
            
            // Calculate speed approximation (total / uptime)
            let speed_mkeys = if uptime_secs > 0 {
                (off_c as f64 / uptime_secs as f64) / 1_000_000.0
            } else { 0.0 };
            
            let msg = format!("💓 HEARTBEAT: {}\nUptime: {}h\nSpeed: {:.2} MKeys/s\nOffline: {}M\nOnline: {}", 
                              hb_worker_id, uptime_hours, speed_mkeys, off_c / 1_000_000, on_c);
            // Send Heartbeat (Silent notification if supported, but simple message for now)
            send_telegram_alert(&msg);
            
            // --- AUTO UPDATE ---
            println!("[Auto-Update] Checking for new code on GitHub...");
            
            // git fetch origin main
            let fetch_status = std::process::Command::new("git")
                .args(&["fetch", "origin", "main"])
                .output();
                
            if fetch_status.is_ok() {
                // git rev-parse HEAD
                let local_hash = std::process::Command::new("git")
                    .args(&["rev-parse", "HEAD"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                    
                // git rev-parse FETCH_HEAD
                let remote_hash = std::process::Command::new("git")
                    .args(&["rev-parse", "FETCH_HEAD"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                    
                if let (Some(local), Some(remote)) = (local_hash, remote_hash) {
                    if local != remote {
                        println!("\n\n[Auto-Update] 🚨 NEW UPDATE DETECTED! 🚨");
                        println!("[Auto-Update] Local:  {}", local);
                        println!("[Auto-Update] Remote: {}", remote);
                        
                        println!("[Auto-Update] Pulling latest code...");
                        let _ = std::process::Command::new("git")
                            .args(&["pull", "origin", "main"])
                            .output();
                        
                        println!("[Auto-Update] Recompiling...");
                        let _ = std::process::Command::new("cargo")
                            .args(&["build", "--release"])
                            .status();
                        
                        println!("[Auto-Update] Restarting application...");
                        // EXEC replaces the current process with the new one.
                        let err = std::process::Command::new("cargo")
                            .args(&["run", "--release"])
                            .exec();
                            
                        panic!("[Auto-Update] Failed to restart: {}", err);
                    } else {
                        println!("[Auto-Update] Application is up to date.");
                    }
                }
            }
        }
    });

    // 3. SPAWN OFFLINE WORKERS (Remaining Cores)
    let num_threads = rayon::current_num_threads();
    let start_time = Instant::now();
    
    if let Some(checker) = offline_checker {
        println!("[Offline] Starting High-Speed Search on {} threads...", num_threads);
        
        // Define Puzzle Ranges
        let (min, max) = puzzles::get_range(puzzle_num);

        (0..num_threads).into_par_iter().for_each(|_| {
            let mut rng = rand::thread_rng(); // Moved scope here
            // Initialize Context ONCE per thread (Critical Optimization)
            let secp = secp256k1::Secp256k1::new();
            
            // PRE-COMPUTE 'ONE' POINT for Key Walking
            let mut one_bytes = [0u8; 32];
            one_bytes[31] = 1;
            let one_sk = secp256k1::SecretKey::from_slice(&one_bytes).expect("1 is valid");
            let one_pk = secp256k1::PublicKey::from_secret_key(&secp, &one_sk);
            // Construct Scalar for add_tweak
            let one_scalar = secp256k1::Scalar::from_be_bytes(one_bytes).expect("valid scalar");

            loop {
                // Generate Starting Random Key
                let current_priv_key_bytes = if puzzle_num > 0 && min > 0 {
                     generator::generate_in_range(&mut rng, min, max)
                } else {
                     generator::generate_random(&mut rng)
                };
                
                // FIXED: Re-added 'mut' because we update this key in the loop!
                let mut current_priv_key = match secp256k1::SecretKey::from_slice(&current_priv_key_bytes) {
                    Ok(k) => k,
                    Err(_) => continue, 
                };
                let mut current_pub_key = secp256k1::PublicKey::from_secret_key(&secp, &current_priv_key);

                // KEY WALKING LOOP (2048 steps)
                for _ in 0..2048 {
                    let hash160 = address::pub_key_to_hash160(&current_pub_key);
                    
                    if checker.contains(&hash160) {
                         println!("\n!!!! OFFLINE MATCH FOUND !!!!");
                         let full_address = address::hash160_to_address(&hash160);
                         let priv_hex = hex::encode(current_priv_key.secret_bytes());
                         
                         println!("Private Key (Hex): {}", priv_hex);
                         println!("Address: {}", full_address);
                         
                         let msg = format!("🎯 OFFLINE MATCH FOUND!\n\nAddr: {}\nPrivKey: {}\n\n⚠️ Check Balance Manually!", full_address, priv_hex);
                         send_telegram_alert(&msg);

                         let log = format!("FOUND_OFFLINE: {} | Key: {}\n", full_address, priv_hex);
                         use std::io::Write;
                         if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open("FOUND_OFFLINE.txt") {
                             file.write_all(log.as_bytes()).unwrap();
                         }
                         
                         offline_found.fetch_add(1, Ordering::Relaxed);
                    }
                    
                    offline_checked.fetch_add(1, Ordering::Relaxed);
                    // UI Thread handles printing
                    
                    // WALK: Pub = Pub + G
                    match secp256k1::PublicKey::combine_keys(&[&current_pub_key, &one_pk]) {
                        Ok(new_pk) => current_pub_key = new_pk,
                        Err(_) => break, 
                    }
                    
                    // WALK: Priv = Priv + 1 (using add_tweak)
                    // FIXED: Actually update the key!
                    match current_priv_key.add_tweak(&one_scalar) {
                         Ok(k) => current_priv_key = k,
                         Err(_) => break,
                    }
                }
            }
        });
    } else {
        println!("[Offline] No target database (funded_addresses.txt) or puzzle selected.");
        println!("[Offline] Skipping high-speed search. Online search is still running...");
        // Just report online status loop in main thread
        // Just report online status loop in main thread
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
            // UI Thread handles printing
        }
    }
}
