//! Pure part of the match: typed amounts, the tolerance, the intersection.

use std::collections::BTreeMap;

/// A holding matches when it is within 0.5% of the amount on the FOMO profile.
pub const TOLERANCE: f64 = 0.005;

/// An amount as FOMO shows it: `12,345.6`, `12.3K`, `1.2M`, `3B`. Positive only.
pub fn parse_amount(text: &str) -> Option<f64> {
    let t: String = text
        .trim()
        .chars()
        .filter(|c| *c != ',' && *c != '_' && !c.is_whitespace())
        .collect();
    let (number, factor) = match t.chars().last()?.to_ascii_uppercase() {
        'K' => (&t[..t.len() - 1], 1e3),
        'M' => (&t[..t.len() - 1], 1e6),
        'B' => (&t[..t.len() - 1], 1e9),
        _ => (&t[..], 1.0),
    };
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let value = number.parse::<f64>().ok()? * factor;
    (value.is_finite() && value > 0.0).then_some(value)
}

/// The raw amount in tokens.
pub fn ui_amount(raw: u64, decimals: u8) -> f64 {
    raw as f64 / 10f64.powi(decimals as i32)
}

/// Within 0.5% of `typed`.
pub fn matches(raw: u64, decimals: u8, typed: f64) -> bool {
    (ui_amount(raw, decimals) - typed).abs() <= typed * TOLERANCE
}

/// Wallets that matched at least two positions, most positions first, then by address.
/// `per_position[i]` holds the wallets that matched position `i`.
pub fn intersect(per_position: &[Vec<String>]) -> Vec<(String, Vec<usize>)> {
    let mut by_wallet: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, wallets) in per_position.iter().enumerate() {
        for w in wallets {
            let hits = by_wallet.entry(w.as_str()).or_default();
            if hits.last() != Some(&i) {
                hits.push(i);
            }
        }
    }
    let mut out: Vec<(String, Vec<usize>)> = by_wallet
        .into_iter()
        .filter(|(_, hits)| hits.len() >= 2)
        .map(|(w, hits)| (w.to_string(), hits))
        .collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_parse_as_fomo_shows_them() {
        assert_eq!(parse_amount("12,345.5"), Some(12_345.5));
        assert_eq!(parse_amount(" 12.3K "), Some(12_300.0));
        assert_eq!(parse_amount("1.5m"), Some(1_500_000.0));
        assert_eq!(parse_amount("2B"), Some(2e9));
        assert_eq!(parse_amount("0"), None);
        assert_eq!(parse_amount("-5"), None);
        assert_eq!(parse_amount("K"), None);
        assert_eq!(parse_amount("1e5"), None);
        assert_eq!(parse_amount(""), None);
    }

    #[test]
    fn half_a_percent_either_way() {
        // 1000 tokens, 6 decimals
        assert!(matches(1_000_000_000, 6, 1000.0));
        assert!(matches(1_004_900_000, 6, 1000.0));
        assert!(matches(995_100_000, 6, 1000.0));
        assert!(!matches(1_005_100_000, 6, 1000.0));
        assert!(!matches(994_900_000, 6, 1000.0));
    }

    #[test]
    fn only_wallets_in_two_positions_or_more() {
        let w = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let got = intersect(&[w(&["a", "b", "c"]), w(&["b", "c"]), w(&["c", "d"])]);
        assert_eq!(
            got,
            vec![("c".into(), vec![0, 1, 2]), ("b".into(), vec![0, 1])]
        );
        assert!(intersect(&[w(&["a"]), w(&["b"])]).is_empty());
    }
}
