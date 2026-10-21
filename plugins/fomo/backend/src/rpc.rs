//! Solana JSON-RPC: token holders and decimals.
//!
//! The node URL is a setting and may hold a key: it never appears in an error, a log or
//! the journal (`reqwest::Error::without_url`).

use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

/// SPL Token program: token accounts are exactly 165 bytes.
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// Token-2022: accounts carry extensions, so no size filter.
pub const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
/// Size of a plain SPL token account.
pub const TOKEN_ACCOUNT_SIZE: u64 = 165;

/// Limits of one call.
#[derive(Clone, Debug)]
pub struct Options {
    /// Whole request, body included.
    pub timeout: Duration,
    /// A bigger answer means too many holders to list.
    pub max_body: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            timeout: Duration::from_secs(60),
            max_body: 64 << 20,
        }
    }
}

/// A call gave no usable answer.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RpcFail {
    /// No answer, a timeout, an HTTP error, an RPC error or an answer that is not JSON-RPC.
    #[error("node did not answer: {0}")]
    NodeDown(String),
    /// The answer is over the size cap, or the node refused to scan that many accounts.
    #[error("too many holders: {0}")]
    TooMany(String),
}

/// One token account: who owns it and the raw amount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holding {
    pub owner: [u8; 32],
    pub amount: u64,
}

pub struct Rpc {
    client: reqwest::Client,
    max_body: usize,
}

#[derive(Deserialize)]
struct Reply<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct Supply {
    value: SupplyValue,
}

#[derive(Deserialize)]
struct SupplyValue {
    decimals: u8,
}

#[derive(Deserialize)]
struct Keyed {
    account: Account,
}

#[derive(Deserialize)]
struct Account {
    data: (String, String),
}

/// Messages nodes give when a program scan is too big for them.
fn means_too_many(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    [
        "too large",
        "too many",
        "exceeded the limit",
        "scan aborted",
        "secondary index",
        "response size",
    ]
    .iter()
    .any(|p| m.contains(p))
}

impl Rpc {
    pub fn new(opts: &Options) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(opts.timeout)
            .build()
            .map_err(|e| e.without_url().to_string())?;
        Ok(Rpc {
            client,
            max_body: opts.max_body,
        })
    }

    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        method: &str,
        params: Value,
    ) -> Result<T, RpcFail> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let down = |e: reqwest::Error| {
            let what = if e.is_timeout() {
                "timed out".to_string()
            } else {
                e.without_url().to_string()
            };
            RpcFail::NodeDown(what)
        };
        let mut resp = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(down)?;
        let status = resp.status();
        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(down)? {
            if bytes.len() + chunk.len() > self.max_body {
                return Err(RpcFail::TooMany(format!(
                    "{method} answer is over {} MB",
                    self.max_body >> 20
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        let reply: Option<Reply<T>> = serde_json::from_slice(&bytes).ok();
        if let Some(Reply {
            error: Some(err), ..
        }) = &reply
        {
            let message = err.message.clone().unwrap_or_default();
            let text = match err.code {
                Some(code) => format!("{method}: RPC error {code}: {message}"),
                None => format!("{method}: RPC error: {message}"),
            };
            return Err(if means_too_many(&message) {
                RpcFail::TooMany(text)
            } else {
                RpcFail::NodeDown(text)
            });
        }
        if !status.is_success() {
            return Err(RpcFail::NodeDown(format!(
                "{method}: HTTP {}",
                status.as_u16()
            )));
        }
        match reply {
            Some(Reply {
                result: Some(r), ..
            }) => Ok(r),
            _ => Err(RpcFail::NodeDown(format!(
                "{method}: not a JSON-RPC answer"
            ))),
        }
    }

    /// Decimals of `mint` (`getTokenSupply`).
    pub async fn decimals(&self, url: &str, mint: &str) -> Result<u8, RpcFail> {
        let s: Supply = self.call(url, "getTokenSupply", json!([mint])).await?;
        Ok(s.value.decimals)
    }

    /// Every account of `program` holding `mint`: owner and raw amount, read through a
    /// 40-byte data slice (owner at 32, amount at 64).
    pub async fn holders(
        &self,
        url: &str,
        program: &str,
        mint: &str,
        data_size: Option<u64>,
    ) -> Result<Vec<Holding>, RpcFail> {
        let mut filters = Vec::new();
        if let Some(size) = data_size {
            filters.push(json!({ "dataSize": size }));
        }
        filters.push(json!({ "memcmp": { "offset": 0, "bytes": mint } }));
        let params = json!([program, {
            "encoding": "base64",
            "dataSlice": { "offset": 32, "length": 40 },
            "filters": filters,
        }]);
        let accounts: Vec<Keyed> = self.call(url, "getProgramAccounts", params).await?;
        let b64 = base64::engine::general_purpose::STANDARD;
        accounts
            .into_iter()
            .map(|k| {
                let raw = b64
                    .decode(k.account.data.0.as_bytes())
                    .ok()
                    .filter(|d| d.len() == 40)
                    .ok_or_else(|| {
                        RpcFail::NodeDown("getProgramAccounts: account data is not 40 bytes".into())
                    })?;
                let mut owner = [0u8; 32];
                owner.copy_from_slice(&raw[..32]);
                let amount = u64::from_le_bytes(raw[32..40].try_into().expect("8 bytes"));
                Ok(Holding { owner, amount })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_a_minute_and_64_mb() {
        let o = Options::default();
        assert_eq!(o.timeout, Duration::from_secs(60));
        assert_eq!(o.max_body, 64 * 1024 * 1024);
    }

    #[test]
    fn scan_limit_messages_mean_too_many_holders() {
        assert!(means_too_many(
            "scan aborted: The accumulated scan results exceeded the limit"
        ));
        assert!(means_too_many("Response too large"));
        assert!(!means_too_many("Internal error"));
    }
}
