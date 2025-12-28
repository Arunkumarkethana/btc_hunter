use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use bloomfilter::Bloom;



pub enum CheckerType {
    Memory(HashSet<[u8; 20]>),
    Bloom(Bloom<[u8; 20]>),
}

pub struct MemoryChecker {
    inner: CheckerType,
}

// Helper to extract Hash160 from string (Base58 Only - Legacy)
fn decode_address_to_hash160(addr: &str) -> Option<[u8; 20]> {
    let trimmed = addr.trim();
    if trimmed.is_empty() { return None; }

    // 1. Try Base58 (Legacy 1...)
    if let Ok(decoded) = bs58::decode(trimmed).into_vec() {
         // Version(1) + Hash(20) + Checksum(4) = 25 bytes
         if decoded.len() == 25 {
             // We can check version byte. 0x00 is Mainnet P2PKH.
             // But we allow others if user puts them in file (e.g. uncompressed legacy)
             // just extracting the 20 byte hash.
             let mut hash = [0u8; 20];
             hash.copy_from_slice(&decoded[1..21]);
             return Some(hash);
         }
    }
    
    // Segwit (Bech32) support temporarily disabled to ensure build stability.
    // Most puzzles (66, 67, 68...) are using Legacy addresses anyway.
    
    None
}

impl MemoryChecker {
    pub fn new(target_list: Vec<&str>) -> Self {
        let mut set = HashSet::new();
        for t in target_list {
            if let Some(hash) = decode_address_to_hash160(t) {
                set.insert(hash);
            }
        }
        MemoryChecker { inner: CheckerType::Memory(set) }
    }

    pub fn from_file(path: &str) -> io::Result<Self> {
        let count_estimate = count_lines(path)?;
        println!("Detected {} addresses in file.", count_estimate);
        
        if count_estimate > 2_000_000 {
            println!("Large dataset detected. Using Bloom Filter (False Positive Rate: 1 in 100M).");
            let mut bloom = Bloom::new_for_fp_rate(count_estimate, 0.00000001).unwrap();
            
            let file = File::open(path)?;
            let reader = io::BufReader::new(file);
            let mut loaded = 0;
            
            for addr in reader.lines().flatten() {
                if let Some(hash) = decode_address_to_hash160(&addr) {
                    bloom.set(&hash);
                    loaded += 1;
                }
            }
             println!("Loaded {} addresses into Bloom Filter.", loaded);
             return Ok(MemoryChecker { inner: CheckerType::Bloom(bloom) });
        }

        let path = Path::new(path);
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);
        
        let mut set = HashSet::new();
        let mut count = 0;
        
        println!("Loading addresses from file & decoding to Hash160 (Exact Set)...");
        for addr in reader.lines().flatten() {
            if let Some(hash) = decode_address_to_hash160(&addr) {
                set.insert(hash);
                count += 1;
            }
        }
        println!("Loaded {} unique addresses into RAM (Optimized Mode).", count);
        
        Ok(MemoryChecker { inner: CheckerType::Memory(set) })
    }

    pub fn contains(&self, hash160: &[u8; 20]) -> bool {
        match &self.inner {
            CheckerType::Memory(set) => set.contains(hash160),
            CheckerType::Bloom(bloom) => bloom.check(hash160),
        }
    }
}

fn count_lines(path: &str) -> io::Result<usize> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    Ok(reader.lines().count())
}
