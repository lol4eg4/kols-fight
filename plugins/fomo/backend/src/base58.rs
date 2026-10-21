//! Base58 (Bitcoin alphabet), enough for 32-byte Solana keys.

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encodes bytes as base58.
pub fn encode(bytes: &[u8]) -> String {
    let zeros = bytes.iter().take_while(|&&b| b == 0).count();
    // little-endian base-58 digits
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 138 / 100 + 1);
    for &b in &bytes[zeros..] {
        let mut carry = b as u32;
        for d in digits.iter_mut() {
            carry += (*d as u32) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(zeros + digits.len());
    out.extend(std::iter::repeat_n('1', zeros));
    out.extend(digits.iter().rev().map(|&d| ALPHABET[d as usize] as char));
    out
}

/// Decodes base58; `None` on a character outside the alphabet.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let zeros = text.bytes().take_while(|&c| c == b'1').count();
    let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
    for c in text.bytes().skip(zeros) {
        let mut carry = ALPHABET.iter().position(|&a| a == c)? as u32;
        for b in bytes.iter_mut() {
            carry += (*b as u32) * 58;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let mut out = vec![0u8; zeros];
    out.extend(bytes.iter().rev());
    Some(out)
}

/// A 32-byte key (a mint or a wallet address).
pub fn is_key(text: &str) -> bool {
    (32..=44).contains(&text.len()) && decode(text).is_some_and(|b| b.len() == 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_known_keys() {
        // the SPL Token program id is the base58 of these well-known bytes
        let token = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        let bytes = decode(token).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(encode(&bytes), token);
        assert_eq!(encode(&[0u8; 32]), "11111111111111111111111111111111");
        assert_eq!(
            decode("11111111111111111111111111111111").unwrap(),
            vec![0u8; 32]
        );
        for seed in 0u8..=255 {
            let key: Vec<u8> = (0..32)
                .map(|i| seed.wrapping_mul(31).wrapping_add(i))
                .collect();
            assert_eq!(decode(&encode(&key)).unwrap(), key);
        }
        assert!(is_key(token));
        assert!(!is_key("0OIl"));
        assert!(!is_key("abc"));
    }
}
