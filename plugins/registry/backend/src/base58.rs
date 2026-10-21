//! Solana address check: base58 (Bitcoin alphabet) that decodes to exactly 32 bytes.

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Base58 decode; `None` on a character outside the alphabet.
fn decode(s: &str) -> Option<Vec<u8>> {
    // Big-endian base-256 digits of the number, built digit by digit.
    let mut bytes: Vec<u8> = Vec::new();
    for c in s.bytes() {
        let mut carry = ALPHABET.iter().position(|&a| a == c)? as u32;
        for b in bytes.iter_mut().rev() {
            carry += u32::from(*b) * 58;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    // Each leading '1' is a leading zero byte.
    let zeros = s.bytes().take_while(|&c| c == b'1').count();
    let mut out = vec![0u8; zeros];
    out.extend(bytes);
    Some(out)
}

/// A base58 string of a 32-byte public key.
pub fn is_solana_address(s: &str) -> bool {
    (32..=44).contains(&s.len()) && decode(s).is_some_and(|b| b.len() == 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert!(is_solana_address(
            "7UBjMaYsE3tKcanfgGrtso6aHAMhVNSVHkkX5j3t7f5y"
        ));
        assert!(is_solana_address("11111111111111111111111111111111"));
        assert!(is_solana_address(
            "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9"
        ));
        // 0, O, I, l are not base58.
        assert!(!is_solana_address(
            "0UBjMaYsE3tKcanfgGrtso6aHAMhVNSVHkkX5j3t7f5y"
        ));
        assert!(!is_solana_address("short"));
        assert!(!is_solana_address(
            "0x52908400098527886E0F7030069857D2E4169EE7"
        ));
    }
}
