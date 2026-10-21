//! A fake Solana JSON-RPC node on a local port: `getTokenSupply` and `getProgramAccounts`
//! over invented mints. Lives outside `backend/` because the architecture test keeps
//! `Router` and `axum::serve` out of plugin backends; this is test-only code.
#![allow(dead_code)]

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use base64::Engine;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

pub const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// One invented token: its program, decimals and accounts (owner, raw amount).
#[derive(Clone)]
pub struct Token {
    pub program: &'static str,
    pub decimals: u8,
    pub accounts: Vec<([u8; 32], u64)>,
}

/// How the node answers a method for a mint.
#[derive(Clone)]
pub enum Mode {
    /// A JSON-RPC error object.
    RpcError(i64, String),
    /// Answers after this long.
    Slow(Duration),
    /// A valid answer padded with this many bytes of whitespace.
    Oversize(usize),
    /// This HTTP status and a body that is not JSON-RPC.
    Http(u16),
}

#[derive(Default)]
pub struct NodeState {
    pub tokens: Mutex<BTreeMap<String, Token>>,
    /// (mint, method) → how to answer.
    pub modes: Mutex<BTreeMap<(String, String), Mode>>,
    /// Every request body, in order.
    pub calls: Mutex<Vec<Value>>,
    /// Every request URI (path and query), in order.
    pub uris: Mutex<Vec<String>>,
}

pub struct FakeNode {
    /// Base URL, with a made-up key in the query like a paid node URL.
    pub url: String,
    pub state: Arc<NodeState>,
}

/// Bytes of a 165-byte SPL token account: mint, owner, amount, then the rest.
fn account_data(mint: &[u8], owner: &[u8; 32], amount: u64) -> Vec<u8> {
    let mut d = Vec::with_capacity(165);
    d.extend_from_slice(mint);
    d.extend_from_slice(owner);
    d.extend_from_slice(&amount.to_le_bytes());
    d.resize(165, 0);
    d
}

fn reply(result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": 1, "result": result })).into_response()
}

fn rpc_error(code: i64, message: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": code, "message": message } }))
        .into_response()
}

fn mint_of(method: &str, params: &Value) -> String {
    match method {
        "getProgramAccounts" => params[1]["filters"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|f| f["memcmp"]["bytes"].as_str())
            .unwrap_or_default()
            .to_string(),
        _ => params[0].as_str().unwrap_or_default().to_string(),
    }
}

fn answer(s: &NodeState, method: &str, params: &Value) -> Response {
    let mint = mint_of(method, params);
    let Some(token) = s.tokens.lock().unwrap().get(&mint).cloned() else {
        return rpc_error(-32602, "Invalid param: could not find mint");
    };
    match method {
        "getTokenSupply" => reply(json!({
            "context": { "slot": 1 },
            "value": { "amount": "0", "decimals": token.decimals, "uiAmount": 0.0, "uiAmountString": "0" }
        })),
        "getProgramAccounts" => {
            let program = params[0].as_str().unwrap_or_default();
            let cfg = &params[1];
            let size = cfg["filters"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|f| f["dataSize"].as_u64());
            let offset = cfg["dataSlice"]["offset"].as_u64().unwrap_or(0) as usize;
            let length = cfg["dataSlice"]["length"].as_u64().unwrap_or(165) as usize;
            let mint_bytes = bs58_decode(&mint);
            let b64 = base64::engine::general_purpose::STANDARD;
            let rows: Vec<Value> = if token.program == program && size.is_none_or(|n| n == 165) {
                token
                    .accounts
                    .iter()
                    .enumerate()
                    .map(|(i, (owner, amount))| {
                        let data = account_data(&mint_bytes, owner, *amount);
                        json!({
                            "pubkey": format!("acct{i}"),
                            "account": {
                                "data": [b64.encode(&data[offset..offset + length]), "base64"],
                                "executable": false,
                                "lamports": 2039280,
                                "owner": program,
                                "space": 165
                            }
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            reply(Value::Array(rows))
        }
        _ => rpc_error(-32601, "Method not found"),
    }
}

/// Its own decoder, so the fake does not trust the code under test.
fn bs58_decode(text: &str) -> Vec<u8> {
    const A: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let zeros = text.bytes().take_while(|&c| c == b'1').count();
    let mut bytes: Vec<u8> = Vec::new();
    for c in text.bytes().skip(zeros) {
        let mut carry = A.iter().position(|&a| a == c).unwrap() as u32;
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
    out
}

async fn handle(
    State(s): State<Arc<NodeState>>,
    uri: axum::http::Uri,
    Json(body): Json<Value>,
) -> Response {
    s.calls.lock().unwrap().push(body.clone());
    s.uris.lock().unwrap().push(uri.to_string());
    let method = body["method"].as_str().unwrap_or_default().to_string();
    let params = body["params"].clone();
    let mode = s
        .modes
        .lock()
        .unwrap()
        .get(&(mint_of(&method, &params), method.clone()))
        .cloned();
    match mode {
        None => answer(&s, &method, &params),
        Some(Mode::RpcError(code, message)) => rpc_error(code, &message),
        Some(Mode::Slow(d)) => {
            tokio::time::sleep(d).await;
            answer(&s, &method, &params)
        }
        Some(Mode::Oversize(n)) => {
            let mut text = String::with_capacity(n + 64);
            text.push_str(r#"{"jsonrpc":"2.0","id":1,"result":[]"#);
            text.extend(std::iter::repeat_n(' ', n));
            text.push('}');
            (StatusCode::OK, [("content-type", "application/json")], text).into_response()
        }
        Some(Mode::Http(code)) => (
            StatusCode::from_u16(code).unwrap(),
            "<html>bad gateway</html>",
        )
            .into_response(),
    }
}

impl FakeNode {
    pub async fn start() -> FakeNode {
        let state = Arc::new(NodeState::default());
        let app = Router::new()
            .route("/", post(handle))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        FakeNode {
            url: format!("http://{addr}/?api-key=node-secret-7f3a"),
            state,
        }
    }

    pub fn token(
        &self,
        mint: &str,
        program: &'static str,
        decimals: u8,
        accounts: &[([u8; 32], u64)],
    ) {
        self.state.tokens.lock().unwrap().insert(
            mint.to_string(),
            Token {
                program,
                decimals,
                accounts: accounts.to_vec(),
            },
        );
    }

    pub fn mode(&self, mint: &str, method: &str, mode: Mode) {
        self.state
            .modes
            .lock()
            .unwrap()
            .insert((mint.to_string(), method.to_string()), mode);
    }

    pub fn calls(&self) -> Vec<Value> {
        self.state.calls.lock().unwrap().clone()
    }
}
