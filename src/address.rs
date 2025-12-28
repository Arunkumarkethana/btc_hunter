use secp256k1::{PublicKey, Secp256k1, SecretKey, All};
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;

// Actually, raw pointer manipulation is fastest but unsafe. 
// We will stick to the library for safety first.

pub fn pub_key_to_hash160(public_key: &PublicKey) -> [u8; 20] {
    // We want Compressed public key for modern Puzzles (Puzzle 66+)
    let serialized_pub = public_key.serialize(); // Returns [u8; 33]
    
    // 1. SHA256
    let mut sha256 = Sha256::new();
    sha256.update(serialized_pub);
    let sha256_res = sha256.finalize();

    // 2. RIPEMD160
    let mut ripemd160 = Ripemd160::new();
    ripemd160.update(sha256_res);
    let hash160_generic = ripemd160.finalize(); // GenericArray
    
    let mut hash160 = [0u8; 20];
    hash160.copy_from_slice(&hash160_generic);
    hash160
}

pub fn priv_key_to_hash160(priv_bytes: &[u8; 32], secp: &Secp256k1<All>) -> [u8; 20] {
    // Create SecretKey
    let secret_key = match SecretKey::from_slice(priv_bytes) {
        Ok(sk) => sk,
        Err(_) => return [0u8; 20], 
    };

    let public_key = PublicKey::from_secret_key(secp, &secret_key);
    pub_key_to_hash160(&public_key)
}

pub fn hash160_to_address(hash160: &[u8; 20]) -> String {
    // 3. Add Version Byte (0x00 for Mainnet)
    let mut joy_payload = Vec::with_capacity(21);
    joy_payload.push(0x00);
    joy_payload.extend_from_slice(hash160);

    // 4. Checksum (Double SHA256)
    let mut sha256_chk = Sha256::new();
    sha256_chk.update(&joy_payload);
    let chk_res1 = sha256_chk.finalize();
    
    let mut sha256_chk2 = Sha256::new();
    sha256_chk2.update(chk_res1);
    let chk_res2 = sha256_chk2.finalize();
    
    let checksum = &chk_res2[0..4];
    
    // 5. Append Checksum
    let mut final_payload = joy_payload.clone();
    final_payload.extend_from_slice(checksum);

    // 6. Base58 Encode
    bs58::encode(final_payload).into_string()
}

pub fn priv_key_to_address(priv_bytes: &[u8; 32], secp: &Secp256k1<All>) -> String {
    let hash160 = priv_key_to_hash160(priv_bytes, secp);
    hash160_to_address(&hash160)
}
