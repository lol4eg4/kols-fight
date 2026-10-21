//! The FOMO match: staff types 2–3 positions from a KOL's FOMO profile; the public node
//! lists each token's holders; one wallet holding the amounts becomes a hypothesis.
//!
//! Reads only the Solana node. FOMO, GMGN, Kolscan and Solscan are links for staff, never
//! read here.

use crate::{
    base58,
    matching::{intersect, matches, parse_amount},
    rpc::{Holding, Rpc, RpcFail, TOKEN_2022_PROGRAM, TOKEN_ACCOUNT_SIZE, TOKEN_PROGRAM},
};
use api_journal::JournalApi;
use api_wallets::{CandidateSource, CandidateStatus, WalletsApi, WalletsError};
use arena_platform::{Api, Config, Person, Setting, Tag};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Matched wallets listed per position; the count is always complete.
pub const SHOWN_MATCHES: usize = 10;

/// What the run ended with. Each one is a line on the screen saying what to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// `PUBLIC_RPC_URL` is not set.
    NotConfigured,
    /// The node gave no usable answer (down, timeout, error).
    NodeDown,
    /// A token has more holders than the node lists in one answer.
    TooManyHolders,
    /// No wallet holds two of the positions.
    NoMatch,
    /// More than one wallet holds two positions or more.
    Several,
    /// One wallet: a new hypothesis in Wallets.
    Hypothesis,
    /// One wallet, already a candidate of the KOL: nothing changed.
    Known,
    /// Wallet matching holds the shared lock: nothing saved.
    Busy,
    /// One wallet, but Wallets did not save it.
    NotSaved,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PositionInput {
    pub mint: String,
    pub amount: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MatchRequest {
    pub kol: String,
    pub positions: Vec<PositionInput>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionResult {
    pub mint: String,
    /// As typed.
    pub amount: String,
    pub decimals: Option<u8>,
    /// Wallets holding the token; `None` when the position was not read.
    pub holders: Option<usize>,
    pub match_count: usize,
    /// First [`SHOWN_MATCHES`] matched wallets.
    pub matches: Vec<String>,
    /// `node_down` or `too_many_holders` when this position failed.
    pub error: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletHit {
    pub address: String,
    /// Indexes of the positions it holds.
    pub positions: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateView {
    pub id: i64,
    pub address: String,
    pub status: CandidateStatus,
    pub source: CandidateSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchResult {
    pub kol: String,
    pub outcome: Outcome,
    /// Technical detail for the error lines (never the node URL).
    pub detail: Option<String>,
    pub positions: Vec<PositionResult>,
    /// Wallets with two positions or more (first [`SHOWN_MATCHES`]).
    pub wallets: Vec<WalletHit>,
    pub wallet_count: usize,
    pub candidate: Option<CandidateView>,
}

/// The request was not run.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Invalid(String),
    #[error("{0} is not in the network")]
    NotMember(String),
    #[error("{0}")]
    Unavailable(String),
    /// The run happened but could not be journaled.
    #[error("the run could not be journaled: {0}")]
    Journal(String),
}

pub struct Fomo {
    pub(crate) rpc: Rpc,
    pub(crate) config: std::sync::Arc<Config>,
    pub(crate) wallets: Api<dyn WalletsApi>,
    pub(crate) journal: Api<dyn JournalApi>,
}

struct Position {
    mint: String,
    typed: String,
    amount: f64,
}

fn validate(req: &MatchRequest) -> Result<(String, Vec<Position>), Error> {
    let kol = req.kol.trim().to_string();
    if kol.is_empty() {
        return Err(Error::Invalid("choose a KOL".into()));
    }
    if !(2..=3).contains(&req.positions.len()) {
        return Err(Error::Invalid("enter 2 or 3 positions".into()));
    }
    let mut out: Vec<Position> = Vec::new();
    for (i, p) in req.positions.iter().enumerate() {
        let n = i + 1;
        let mint = p.mint.trim().to_string();
        if !base58::is_key(&mint) {
            return Err(Error::Invalid(format!("position {n}: not a token address")));
        }
        if out.iter().any(|o| o.mint == mint) {
            return Err(Error::Invalid(format!(
                "position {n}: the same token twice"
            )));
        }
        let typed = p.amount.trim().to_string();
        let amount = parse_amount(&typed).ok_or_else(|| {
            Error::Invalid(format!("position {n}: the amount is not a positive number"))
        })?;
        out.push(Position {
            mint,
            typed,
            amount,
        });
    }
    Ok((kol, out))
}

/// Owners whose accounts together hold the typed amount (a wallet may hold a token in
/// more than one account).
fn matching_owners(holdings: &[Holding], decimals: u8, typed: f64) -> (usize, Vec<String>) {
    let mut per_owner: BTreeMap<[u8; 32], u64> = BTreeMap::new();
    for h in holdings {
        let total = per_owner.entry(h.owner).or_default();
        *total = total.saturating_add(h.amount);
    }
    let holders = per_owner.values().filter(|a| **a > 0).count();
    let found = per_owner
        .iter()
        .filter(|(_, raw)| matches(**raw, decimals, typed))
        .map(|(owner, _)| base58::encode(owner))
        .collect();
    (holders, found)
}

impl Fomo {
    /// Runs one match for `person`. Every run that gets past validation is journaled.
    pub async fn run(&self, person: &Person, req: &MatchRequest) -> Result<MatchResult, Error> {
        if !Tag::Staff.admits(person) {
            return Err(Error::Forbidden);
        }
        let (kol, positions) = validate(req)?;
        let wallets = self.wallets.pick(person);
        match wallets.status(person, &kol).await {
            Ok(_) => {}
            Err(WalletsError::NotMember { .. }) => return Err(Error::NotMember(kol)),
            Err(e) => return Err(Error::Unavailable(e.to_string())),
        }

        let mut result = MatchResult {
            kol: kol.clone(),
            outcome: Outcome::NoMatch,
            detail: None,
            positions: positions
                .iter()
                .map(|p| PositionResult {
                    mint: p.mint.clone(),
                    amount: p.typed.clone(),
                    decimals: None,
                    holders: None,
                    match_count: 0,
                    matches: Vec::new(),
                    error: None,
                })
                .collect(),
            wallets: Vec::new(),
            wallet_count: 0,
            candidate: None,
        };

        match self.config.get(Setting::PublicRpcUrl) {
            Err(e) => {
                result.outcome = Outcome::NotConfigured;
                result.detail = Some(e.to_string());
            }
            Ok(url) => {
                self.read_and_match(person, url, &positions, &mut result)
                    .await
            }
        }

        let after = serde_json::to_value(&result).map_err(|e| Error::Journal(e.to_string()))?;
        self.journal
            .pick(person)
            .record(person, "fomo", &kol, "match", None, Some(after))
            .await
            .map_err(|e| Error::Journal(e.to_string()))?;
        Ok(result)
    }

    async fn read_and_match(
        &self,
        person: &Person,
        url: &str,
        positions: &[Position],
        result: &mut MatchResult,
    ) {
        let mut per_position: Vec<Vec<String>> = Vec::new();
        for (i, p) in positions.iter().enumerate() {
            match self.read_position(url, p).await {
                Ok((decimals, holders, found)) => {
                    let r = &mut result.positions[i];
                    r.decimals = Some(decimals);
                    r.holders = Some(holders);
                    r.match_count = found.len();
                    r.matches = found.iter().take(SHOWN_MATCHES).cloned().collect();
                    per_position.push(found);
                }
                Err(fail) => {
                    let outcome = match fail {
                        RpcFail::NodeDown(_) => Outcome::NodeDown,
                        RpcFail::TooMany(_) => Outcome::TooManyHolders,
                    };
                    result.positions[i].error = Some(outcome);
                    result.outcome = outcome;
                    result.detail = Some(fail.to_string());
                    return;
                }
            }
        }

        let hits = intersect(&per_position);
        result.wallet_count = hits.len();
        result.wallets = hits
            .iter()
            .take(SHOWN_MATCHES)
            .map(|(address, idx)| WalletHit {
                address: address.clone(),
                positions: idx.clone(),
            })
            .collect();
        let [(address, idx)] = hits.as_slice() else {
            result.outcome = if hits.is_empty() {
                Outcome::NoMatch
            } else {
                Outcome::Several
            };
            return;
        };

        let evidence = format!(
            "FOMO match, {} of {} positions: {}",
            idx.len(),
            positions.len(),
            idx.iter()
                .map(|&i| format!("{} ≈ {}", positions[i].mint, positions[i].typed))
                .collect::<Vec<_>>()
                .join("; ")
        );
        match self
            .wallets
            .pick(person)
            .propose(person, &result.kol, address, &evidence)
            .await
        {
            Ok(c) => {
                result.outcome = if c.status == CandidateStatus::Hypothesis
                    && c.source == CandidateSource::Fomo
                {
                    Outcome::Hypothesis
                } else {
                    Outcome::Known
                };
                result.candidate = Some(CandidateView {
                    id: c.id,
                    address: c.address,
                    status: c.status,
                    source: c.source,
                });
            }
            Err(WalletsError::Busy) => result.outcome = Outcome::Busy,
            Err(e) => {
                result.outcome = Outcome::NotSaved;
                result.detail = Some(e.to_string());
            }
        }
    }

    /// Decimals, then the holders under both token programs: one call each.
    async fn read_position(
        &self,
        url: &str,
        p: &Position,
    ) -> Result<(u8, usize, Vec<String>), RpcFail> {
        let decimals = self.rpc.decimals(url, &p.mint).await?;
        let mut holdings = self
            .rpc
            .holders(url, TOKEN_PROGRAM, &p.mint, Some(TOKEN_ACCOUNT_SIZE))
            .await?;
        holdings.extend(
            self.rpc
                .holders(url, TOKEN_2022_PROGRAM, &p.mint, None)
                .await?,
        );
        let (holders, found) = matching_owners(&holdings, decimals, p.amount);
        Ok((decimals, holders, found))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(kol: &str, positions: &[(&str, &str)]) -> MatchRequest {
        MatchRequest {
            kol: kol.into(),
            positions: positions
                .iter()
                .map(|(m, a)| PositionInput {
                    mint: m.to_string(),
                    amount: a.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn requests_are_checked_before_any_call() {
        let m1 = base58::encode(&[1u8; 32]);
        let m2 = base58::encode(&[2u8; 32]);
        let m4 = base58::encode(&[4u8; 32]);
        assert!(validate(&req("k", &[(&m1, "10"), (&m2, "1.5K")])).is_ok());
        let err = |r: MatchRequest| validate(&r).err().unwrap().to_string();
        assert_eq!(err(req(" ", &[(&m1, "1"), (&m2, "1")])), "choose a KOL");
        assert_eq!(err(req("k", &[(&m1, "1")])), "enter 2 or 3 positions");
        assert_eq!(
            err(req("k", &[(&m1, "1"), (&m2, "1"), (&m4, "1"), (&m1, "2")])),
            "enter 2 or 3 positions"
        );
        assert_eq!(
            err(req("k", &[(&m1, "1"), ("not-a-mint", "1")])),
            "position 2: not a token address"
        );
        assert_eq!(
            err(req("k", &[(&m1, "1"), (&m1, "2")])),
            "position 2: the same token twice"
        );
        assert_eq!(
            err(req("k", &[(&m1, "0"), (&m2, "2")])),
            "position 1: the amount is not a positive number"
        );
    }

    #[test]
    fn a_wallet_with_two_accounts_is_counted_once_with_the_sum() {
        let a = [7u8; 32];
        let b = [8u8; 32];
        let holdings = vec![
            Holding {
                owner: a,
                amount: 600,
            },
            Holding {
                owner: a,
                amount: 400,
            },
            Holding {
                owner: b,
                amount: 1000,
            },
            Holding {
                owner: [9u8; 32],
                amount: 0,
            },
        ];
        let (holders, found) = matching_owners(&holdings, 0, 1000.0);
        assert_eq!(holders, 2);
        assert_eq!(found, vec![base58::encode(&a), base58::encode(&b)]);
    }
}
