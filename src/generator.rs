use rand::RngCore;

// Generate a random 32-byte array that is valid for secp256k1
pub fn generate_random<R: RngCore>(rng: &mut R) -> [u8; 32] {
    let mut key = [0u8; 32];
    rng.fill_bytes(&mut key);
    // In a production app you'd retry if key is 0 or >= curve order,
    // but the probability is astronomically low (1 in 2^128+).
    // For raw speed in a "lucky" hunter, checking validity every time slows us down slightly,
    // but the secp256k1 lib might panic or error if we use an invalid key later.
    // However, generating a random 256-bit number that isn't a valid key is practically impossible.
    key
}

// Generate a random key within a 64-128 bit range (optimized for puzzles 64-128)
pub fn generate_in_range<R: RngCore>(rng: &mut R, min: u128, max: u128) -> [u8; 32] {
    // Range is [min, max)
    let range = max - min;
    let offset = rng.next_u64() as u128; // Simple fast random, using u64 and expanding
    // For full 128 bit range we might need more randomness, but for speed on search this is okay.
    // Better: use full u128 random if range is large.
    
    let mut key_val = min + (offset % range); // Note: verify bias if strict standard required.
    
    // If range > u64::MAX, we need two u64s.
    if range > (u64::MAX as u128) {
         let mut bytes = [0u8; 16];
         rng.fill_bytes(&mut bytes);
         let rand_128 = u128::from_be_bytes(bytes);
         key_val = min + (rand_128 % range);
    }
    
    // Convert u128 to [u8; 32] (padded with zeros)
    let mut key = [0u8; 32];
    let bytes = key_val.to_be_bytes();
    // Copy the 16 bytes of u128 to the end of the 32-byte key
    key[16..32].copy_from_slice(&bytes);
    
    key
}

