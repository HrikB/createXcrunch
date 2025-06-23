use alloy_primitives::hex;
use sha3::{Digest, Keccak256};

/// Convert an Ethereum address to EIP-55 checksum format
/// 
/// EIP-55 specifies that we hash the lowercase hex address (without 0x prefix)
/// and use the hash to determine which characters should be uppercase.
/// If the ith character is a letter (a-f) and the 4*ith bit of the hash is 1,
/// then capitalize it.
pub fn to_checksum_address(address: &[u8]) -> String {
    // Convert address bytes to lowercase hex string (without 0x)
    let hex_address = hex::encode(address);
    
    // Hash the lowercase hex address
    let mut hasher = Keccak256::new();
    hasher.update(hex_address.as_bytes());
    let hash = hasher.finalize();
    
    // Build the checksum address
    let mut checksum = String::with_capacity(42);
    checksum.push_str("0x");
    
    for (i, ch) in hex_address.chars().enumerate() {
        if ch.is_alphabetic() {
            // Get the corresponding nibble from the hash
            let hash_byte = hash[i / 2];
            let nibble = if i % 2 == 0 {
                (hash_byte >> 4) & 0xf
            } else {
                hash_byte & 0xf
            };
            
            // If the corresponding bit is 1, uppercase the character
            if nibble >= 8 {
                checksum.push(ch.to_ascii_uppercase());
            } else {
                checksum.push(ch);
            }
        } else {
            checksum.push(ch);
        }
    }
    
    checksum
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    #[test]
    fn test_checksum_addresses() {
        // Test vectors from EIP-55
        let test_cases = vec![
            ("52908400098527886e0f7030069857d2e4169ee7", "0x52908400098527886E0F7030069857D2E4169EE7"),
            ("8617e340b3d01fa5f11f306f4090fd50e238070d", "0x8617E340B3D01FA5F11F306F4090FD50E238070D"),
            ("de709f2102306220921060314715629080e2fb77", "0xde709f2102306220921060314715629080e2fb77"),
            ("27b1fdb04752bbc536007a920d24acb045561c26", "0x27b1fdb04752bbc536007a920d24acb045561c26"),
            ("5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed", "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"),
            ("fB6916095ca1df60bB79Ce92cE3Ea74c37c5d359", "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359"),
            ("dbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB", "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB"),
            ("D1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb", "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb"),
        ];
        
        for (input, expected) in test_cases {
            let addr_bytes = hex::decode(input).unwrap();
            let checksum = to_checksum_address(&addr_bytes);
            assert_eq!(checksum, expected, "Failed for input: {}", input);
        }
    }
}