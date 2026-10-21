//! Parsers for the two KOL Explorer pages. No network, no database.
//!
//! Leaderboard: one `<tr class="board-row" data-…>` per KOL. Everything is read from the
//! `data-*` attributes of that tag, never from cell order, so a restyled table does not
//! break the read; a renamed or removed attribute does, and that fails the run.
//!
//! Profile: the «Avg position» tile and the «Median hold» line.

use std::collections::HashMap;

/// Fewer rows than this means the page changed (the board lists 150).
pub const MIN_ROWS: usize = 50;

/// One KOL on the leaderboard. Money is USD, win rates are fractions 0..=1. A value the
/// page leaves empty is `None`.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardRow {
    pub rank: u32,
    /// Profile slug: `/kol/<slug>`.
    pub slug: String,
    pub name: String,
    /// Base58 wallet address (`data-address`, original case).
    pub address: String,
    /// X handle without `@`, from `data-twitter` (a profile URL).
    pub x_handle: Option<String>,
    pub d1: Option<f64>,
    pub d7: Option<f64>,
    pub d30: Option<f64>,
    pub unreal: Option<f64>,
    pub total: Option<f64>,
    pub wr7: Option<f64>,
    pub wr30: Option<f64>,
    /// Unix seconds of the last trade.
    pub last_tx_ts: Option<i64>,
}

/// Why the leaderboard could not be read. Any of them fails the run.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("leaderboard has {found} rows, expected at least {MIN_ROWS}")]
    TooFewRows { found: usize },
    #[error("row {row}: attribute {name} is missing")]
    MissingAttribute { row: usize, name: &'static str },
    #[error("row {row}: attribute {name} has bad value {value:?}")]
    BadValue {
        row: usize,
        name: &'static str,
        value: String,
    },
}

/// Parses the front page. Rows come back sorted by rank.
pub fn parse_leaderboard(html: &str) -> Result<Vec<BoardRow>, ParseError> {
    let mut rows = Vec::new();
    let mut from = 0;
    while let Some(at) = html[from..].find("<tr") {
        let start = from + at;
        let Some((attrs, end)) = tag_attrs(&html[start + 3..]) else {
            break;
        };
        from = start + 3 + end;
        if !attrs
            .get("class")
            .is_some_and(|c| c.split_whitespace().any(|t| t == "board-row"))
        {
            continue;
        }
        rows.push(attrs);
    }
    if rows.len() < MIN_ROWS {
        return Err(ParseError::TooFewRows { found: rows.len() });
    }
    let mut out = Vec::with_capacity(rows.len());
    for (i, attrs) in rows.iter().enumerate() {
        out.push(read_row(i + 1, attrs)?);
    }
    out.sort_by_key(|r| r.rank);
    Ok(out)
}

fn read_row(n: usize, a: &HashMap<String, String>) -> Result<BoardRow, ParseError> {
    let need = |name: &'static str| -> Result<&str, ParseError> {
        a.get(name)
            .map(String::as_str)
            .ok_or(ParseError::MissingAttribute { row: n, name })
    };
    let text = |name: &'static str| -> Result<String, ParseError> {
        let v = need(name)?.trim();
        if v.is_empty() {
            return Err(ParseError::BadValue {
                row: n,
                name,
                value: String::new(),
            });
        }
        Ok(v.to_string())
    };
    let num = |name: &'static str| -> Result<Option<f64>, ParseError> {
        let v = need(name)?.trim();
        if v.is_empty() {
            return Ok(None);
        }
        match v.parse::<f64>() {
            Ok(x) if x.is_finite() => Ok(Some(x)),
            _ => Err(ParseError::BadValue {
                row: n,
                name,
                value: v.to_string(),
            }),
        }
    };
    let rank = text("data-rank")?;
    let rank = rank.parse::<u32>().map_err(|_| ParseError::BadValue {
        row: n,
        name: "data-rank",
        value: rank.clone(),
    })?;
    let address = text("data-address")?;
    if !is_base58_address(&address) {
        return Err(ParseError::BadValue {
            row: n,
            name: "data-address",
            value: address,
        });
    }
    let last_tx_ts = match num("data-last-tx-ts")? {
        Some(x) if x >= 0.0 && x.fract() == 0.0 => Some(x as i64),
        Some(x) => {
            return Err(ParseError::BadValue {
                row: n,
                name: "data-last-tx-ts",
                value: x.to_string(),
            });
        }
        None => None,
    };
    Ok(BoardRow {
        rank,
        slug: text("data-slug")?,
        name: text("data-kol-name")?,
        address,
        x_handle: x_handle(need("data-twitter")?),
        d1: num("data-pnl-today")?,
        d7: num("data-pnl7d")?,
        d30: num("data-pnl30d")?,
        unreal: num("data-unrealized")?,
        total: num("data-totalpnl")?,
        wr7: num("data-wr7d")?.map(|p| p / 100.0),
        wr30: num("data-wr30d")?.map(|p| p / 100.0),
        last_tx_ts,
    })
}

/// 32..=44 base58 characters (a 32-byte key).
fn is_base58_address(s: &str) -> bool {
    (32..=44).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l'))
}

/// `https://x.com/notdecu?s=1` -> `notdecu`. Anything else -> `None`.
pub fn x_handle(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let host = host
        .trim_start_matches("www.")
        .trim_start_matches("mobile.");
    if !matches!(host, "x.com" | "twitter.com") {
        return None;
    }
    let seg = path.split(['/', '?', '#']).next()?.trim_start_matches('@');
    let ok = !seg.is_empty()
        && seg.len() <= 15
        && seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    ok.then(|| seg.to_string())
}

/// What the profile page tells us. Either value may be missing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// Average position (buy) size, USD.
    pub avg_pos: Option<f64>,
    /// Median hold time, seconds.
    pub hold_s: Option<f64>,
}

/// The profile page has neither value: it changed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("profile page has no average position and no median hold")]
pub struct ProfileError;

pub fn parse_profile(html: &str) -> Result<Profile, ProfileError> {
    let avg_pos = html
        .find("Avg position</span>")
        .and_then(|at| before_tag(&html[..at], "kx-dna-v\">", "</span>"))
        .and_then(money);
    let hold_s = html
        .find("</b>Median hold")
        .and_then(|at| {
            let head = &html[..at];
            head.rfind("<b>").map(|b| head[b + 3..].to_string())
        })
        .and_then(|t| duration_secs(&t));
    if avg_pos.is_none() && hold_s.is_none() {
        return Err(ProfileError);
    }
    Ok(Profile { avg_pos, hold_s })
}

/// Text between the last `open` before the end of `head` and the `close` after it.
fn before_tag(head: &str, open: &str, close: &str) -> Option<String> {
    let start = head.rfind(open)? + open.len();
    let end = head[start..].find(close)?;
    Some(head[start..start + end].trim().to_string())
}

/// `$342`, `$15,320`, `$1.2K`, `$3M` -> USD.
fn money(s: String) -> Option<f64> {
    let t = s.trim().trim_start_matches('$').replace(',', "");
    let (num, mult) = match t.chars().last()? {
        'K' | 'k' => (&t[..t.len() - 1], 1e3),
        'M' | 'm' => (&t[..t.len() - 1], 1e6),
        'B' | 'b' => (&t[..t.len() - 1], 1e9),
        _ => (t.as_str(), 1.0),
    };
    let v = num.trim().parse::<f64>().ok()? * mult;
    (v.is_finite() && v >= 0.0).then_some(v)
}

/// `15 s`, `13 min`, `2.5 h`, `1 d`, `1h 20m` -> seconds. Unknown unit -> `None`.
fn duration_secs(s: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut seen = false;
    let mut chars = s.trim().chars().peekable();
    while chars.peek().is_some() {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut num = String::new();
        while let Some(c) = chars.next_if(|c| c.is_ascii_digit() || *c == '.' || *c == ',') {
            if c != ',' {
                num.push(c);
            }
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut unit = String::new();
        while let Some(c) = chars.next_if(|c| c.is_ascii_alphabetic()) {
            unit.push(c.to_ascii_lowercase());
        }
        let mult = match unit.as_str() {
            "s" | "sec" | "secs" => 1.0,
            "m" | "min" | "mins" => 60.0,
            "h" | "hr" | "hrs" => 3600.0,
            "d" | "day" | "days" => 86400.0,
            _ => return None,
        };
        total += num.parse::<f64>().ok()? * mult;
        seen = true;
    }
    seen.then_some(total)
}

/// Attributes of the tag whose text starts at `s` (just after `<tr`), and the offset after
/// its closing `>`. Attribute names are lowercased, values decoded for the common entities.
fn tag_attrs(s: &str) -> Option<(HashMap<String, String>, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut attrs = HashMap::new();
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        if b[i] == b'>' {
            return Some((attrs, i + 1));
        }
        let name_start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let name = s[name_start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                if i >= b.len() {
                    return None;
                }
                value = decode(&s[vs..i]);
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = decode(&s[vs..i]);
            }
        }
        if !name.is_empty() {
            attrs.insert(name, value);
        }
    }
}

fn decode(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_and_duration() {
        assert_eq!(money("$342".into()), Some(342.0));
        assert_eq!(money("$15,320".into()), Some(15320.0));
        assert_eq!(money("$1.2K".into()), Some(1200.0));
        assert_eq!(money("n/a".into()), None);
        assert_eq!(duration_secs("15 s"), Some(15.0));
        assert_eq!(duration_secs("13 min"), Some(780.0));
        assert_eq!(duration_secs("2.5 h"), Some(9000.0));
        assert_eq!(duration_secs("1h 20m"), Some(4800.0));
        assert_eq!(duration_secs("3 weeks"), None);
        assert_eq!(duration_secs(""), None);
    }

    #[test]
    fn handles() {
        assert_eq!(
            x_handle("https://x.com/notdecu").as_deref(),
            Some("notdecu")
        );
        assert_eq!(
            x_handle("https://twitter.com/@Foo_1?s=20").as_deref(),
            Some("Foo_1")
        );
        assert_eq!(x_handle(""), None);
        assert_eq!(x_handle("https://example.com/foo"), None);
    }

    #[test]
    fn tag_with_odd_quoting() {
        let (a, end) =
            tag_attrs(r#" class='board-row' data-x="a &amp; b" data-y=5 hidden>rest"#).unwrap();
        assert_eq!(a["class"], "board-row");
        assert_eq!(a["data-x"], "a & b");
        assert_eq!(a["data-y"], "5");
        assert!(a.contains_key("hidden"));
        assert_eq!(
            &r#" class='board-row' data-x="a &amp; b" data-y=5 hidden>rest"#[end..],
            "rest"
        );
    }
}
