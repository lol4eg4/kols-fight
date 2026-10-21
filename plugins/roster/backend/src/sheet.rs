//! Reads the network sheet's CSV export.
//!
//! Columns are found by header name, not by position. Required: `Name`, `FOMO`. Optional:
//! `Group`, `Low $`, `High $`, `Deliverable / note`, `Wallet`, `Pump.fun`. Unknown columns are
//! ignored. Unnamed columns after `Wallet` are read as extra wallets (base58 addresses only).
//! A missing required header fails the whole read: the roster stays as it was.
//!
//! Rows are keyed by the lowercase FOMO handle. A blank row is skipped silently; a row without a
//! FOMO handle or a name, or a second row with a FOMO handle already seen, is skipped and
//! listed with its reason.

use api_roster::{FORMATS, is_solana_address};
use serde::{Deserialize, Serialize};

/// One usable sheet row.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetRow {
    /// 1-based line (record) number in the CSV, header = 1.
    pub line: usize,
    pub name: String,
    pub group: Option<String>,
    pub low_usd: Option<f64>,
    pub high_usd: Option<f64>,
    /// The whole «Deliverable / note» cell. Team-only.
    pub note: Option<String>,
    /// FOMO handle as written (no `@`, no URL).
    pub fomo: String,
    /// Lowercase `fomo`: the row key.
    pub fomo_key: String,
    pub pumpfun: Option<String>,
    /// Base58 addresses from `Wallet` and the unnamed columns after it, in sheet order.
    pub wallets: Vec<String>,
    /// A `Wallet` cell that is not an address (a note, an EVM address). Team-only.
    pub wallet_note: Option<String>,
    /// Known format keywords found in the note.
    pub formats: Vec<String>,
}

/// A row that was not used, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub line: usize,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub rows: Vec<SheetRow>,
    pub skipped: Vec<Skipped>,
}

/// The sheet could not be read as a whole.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SheetError {
    #[error("the sheet is empty")]
    Empty,
    #[error("sheet columns changed: missing column {0}")]
    MissingColumn(&'static str),
    #[error("sheet is not valid CSV: {0}")]
    Csv(String),
}

/// Splits CSV text into records. Handles quotes, `""`, CRLF and newlines inside quotes.
fn records(text: &str) -> Result<Vec<Vec<String>>, SheetError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut cell_started = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' if !cell_started => {
                quoted = true;
                cell_started = true;
            }
            ',' => {
                row.push(std::mem::take(&mut cell));
                cell_started = false;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                cell_started = false;
                out.push(std::mem::take(&mut row));
            }
            _ => {
                cell.push(c);
                cell_started = true;
            }
        }
    }
    if quoted {
        return Err(SheetError::Csv("a quoted cell is not closed".into()));
    }
    if cell_started || !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        out.push(row);
    }
    Ok(out)
}

fn norm(h: &str) -> String {
    h.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Parses `text`. See the module docs for the rules.
pub fn parse(text: &str) -> Result<Parsed, SheetError> {
    let recs = records(text)?;
    let Some(header) = recs.first() else {
        return Err(SheetError::Empty);
    };
    let find = |name: &str| header.iter().position(|h| norm(h) == name);
    let name_col = find("name").ok_or(SheetError::MissingColumn("Name"))?;
    let fomo_col = find("fomo").ok_or(SheetError::MissingColumn("FOMO"))?;
    let group_col = find("group");
    let low_col = find("low $");
    let high_col = find("high $");
    let note_col = find("deliverable / note");
    let wallet_col = find("wallet");
    let pump_col = find("pump.fun");
    let extra_cols: Vec<usize> = match wallet_col {
        Some(w) => header
            .iter()
            .enumerate()
            .filter(|(i, h)| *i > w && h.trim().is_empty())
            .map(|(i, _)| i)
            .collect(),
        None => Vec::new(),
    };

    let mut rows: Vec<SheetRow> = Vec::new();
    let mut skipped = Vec::new();
    for (i, rec) in recs.iter().enumerate().skip(1) {
        let line = i + 1;
        if rec.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let cell = |col: Option<usize>| -> Option<String> {
            col.and_then(|c| rec.get(c))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let Some(fomo) = cell(Some(fomo_col))
            .map(|f| handle_of(&f))
            .filter(|f| !f.is_empty())
        else {
            skipped.push(Skipped {
                line,
                reason: "no FOMO handle".into(),
            });
            continue;
        };
        let Some(name) = cell(Some(name_col)) else {
            skipped.push(Skipped {
                line,
                reason: "no name".into(),
            });
            continue;
        };
        let fomo_key = fomo.to_lowercase();
        if rows.iter().any(|r| r.fomo_key == fomo_key) {
            skipped.push(Skipped {
                line,
                reason: "FOMO handle already used by an earlier row".into(),
            });
            continue;
        }
        let mut wallets = Vec::new();
        let mut wallet_note = None;
        if let Some(w) = cell(wallet_col) {
            if is_solana_address(&w) {
                wallets.push(w);
            } else {
                wallet_note = Some(w);
            }
        }
        for c in &extra_cols {
            if let Some(w) = rec.get(*c).map(|s| s.trim())
                && is_solana_address(w)
                && !wallets.iter().any(|x| x == w)
            {
                wallets.push(w.to_string());
            }
        }
        let note = cell(note_col);
        rows.push(SheetRow {
            line,
            name,
            group: cell(group_col),
            low_usd: cell(low_col).and_then(|v| money(&v)),
            high_usd: cell(high_col).and_then(|v| money(&v)),
            formats: note.as_deref().map(formats_of).unwrap_or_default(),
            note,
            fomo,
            fomo_key,
            pumpfun: cell(pump_col)
                .map(|p| handle_of(&p))
                .filter(|p| !p.is_empty()),
            wallets,
            wallet_note,
        });
    }
    Ok(Parsed { rows, skipped })
}

/// `@name`, `https://host/path/name?x` -> `name`.
fn handle_of(cell: &str) -> String {
    let s = cell.trim();
    let s = s.split(['?', '#']).next().unwrap_or(s);
    let s = s.trim_end_matches('/');
    let s = s.rsplit('/').next().unwrap_or(s);
    s.trim().trim_start_matches('@').to_string()
}

/// `$1,500`, `1500`, `1.5k` -> USD. Anything else -> `None`.
fn money(cell: &str) -> Option<f64> {
    let t: String = cell
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '$' && *c != ',')
        .collect();
    let (digits, mult) = match t.strip_suffix(['k', 'K']) {
        Some(d) => (d, 1000.0),
        None => (t.as_str(), 1.0),
    };
    let v: f64 = digits.parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v * mult)
}

/// Known format keywords in `note` (whole words, singular or plural), in `FORMATS` order.
pub fn formats_of(note: &str) -> Vec<String> {
    let words: Vec<String> = note
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    FORMATS
        .iter()
        .filter(|f| words.iter().any(|w| w == **f || *w == format!("{f}s")))
        .map(|f| f.to_string())
        .collect()
}
