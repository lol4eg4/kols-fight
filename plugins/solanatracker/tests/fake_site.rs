//! A fake Solana Tracker on a local port. Lives outside `backend/` because the architecture
//! test keeps `Router` and `axum::serve` out of plugin backends; this is test-only code.
#![allow(dead_code)]

use super::{HISTORY_JSON, WALLET_JSON};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

// ----------------------------------------------------------- fake ST site

/// What the fake answers for a path.
#[derive(Clone)]
pub enum Reply {
    /// Status and body.
    Fixed(u16, String),
    /// The first `n` requests get `status` (empty JSON body), then 200 with the body.
    FailFirst(u32, u16, String),
}

pub struct SiteState {
    /// Keyed by path without query (`/v2/pnl/wallets/<addr>` ...).
    pub replies: Mutex<BTreeMap<String, Reply>>,
    /// Answer for a path with no entry.
    pub fallback: Mutex<(u16, String)>,
    pub hits: Mutex<Vec<(String, Instant)>>,
    pub keys: Mutex<Vec<Option<String>>>,
}

pub struct FakeSt {
    pub url: String,
    pub state: Arc<SiteState>,
}

impl FakeSt {
    pub async fn start() -> FakeSt {
        let state = Arc::new(SiteState {
            replies: Mutex::default(),
            fallback: Mutex::new((404, "{\"error\":\"not found\"}".into())),
            hits: Mutex::default(),
            keys: Mutex::default(),
        });
        async fn answer(s: Arc<SiteState>, path: String, h: HeaderMap) -> (StatusCode, String) {
            s.hits.lock().unwrap().push((path.clone(), Instant::now()));
            s.keys
                .lock()
                .unwrap()
                .push(h.get("x-api-key").map(|v| v.to_str().unwrap().to_string()));
            let n = s
                .hits
                .lock()
                .unwrap()
                .iter()
                .filter(|(p, _)| *p == path)
                .count() as u32;
            let reply = s.replies.lock().unwrap().get(&path).cloned();
            let (code, body) = match reply {
                Some(Reply::Fixed(c, b)) => (c, b),
                Some(Reply::FailFirst(k, c, _)) if n <= k => (c, "{}".to_string()),
                Some(Reply::FailFirst(_, _, b)) => (200, b),
                None => s.fallback.lock().unwrap().clone(),
            };
            (StatusCode::from_u16(code).unwrap(), body)
        }
        async fn list(State(s): State<Arc<SiteState>>, h: HeaderMap) -> (StatusCode, String) {
            answer(s, "/v2/pnl/leaderboard/kols/period".into(), h).await
        }
        async fn wallet(
            State(s): State<Arc<SiteState>>,
            Path(a): Path<String>,
            h: HeaderMap,
        ) -> (StatusCode, String) {
            answer(s, format!("/v2/pnl/wallets/{a}"), h).await
        }
        async fn history(
            State(s): State<Arc<SiteState>>,
            Path(a): Path<String>,
            h: HeaderMap,
        ) -> (StatusCode, String) {
            answer(s, format!("/v2/pnl/wallets/{a}/history"), h).await
        }
        let app = Router::new()
            .route("/v2/pnl/leaderboard/kols/period", get(list))
            .route("/v2/pnl/wallets/{a}", get(wallet))
            .route("/v2/pnl/wallets/{a}/history", get(history))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        FakeSt { url, state }
    }

    pub fn set(&self, path: &str, code: u16, body: &str) {
        self.state
            .replies
            .lock()
            .unwrap()
            .insert(path.to_string(), Reply::Fixed(code, body.to_string()));
    }

    pub fn set_reply(&self, path: &str, r: Reply) {
        self.state
            .replies
            .lock()
            .unwrap()
            .insert(path.to_string(), r);
    }

    pub fn set_list(&self, code: u16, body: &str) {
        self.set(LIST_PATH, code, body);
    }

    pub fn set_wallet(&self, address: &str, code: u16, body: &str) {
        self.set(&format!("/v2/pnl/wallets/{address}"), code, body);
    }

    pub fn set_history(&self, address: &str, code: u16, body: &str) {
        self.set(&format!("/v2/pnl/wallets/{address}/history"), code, body);
    }

    /// Wallet and history answers for `address` from the Tekkerrss fixtures.
    pub fn set_traded(&self, address: &str) {
        self.set_wallet(address, 200, WALLET_JSON);
        self.set_history(address, 200, HISTORY_JSON);
    }

    pub fn hits_on(&self, path: &str) -> usize {
        self.state
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p == path)
            .count()
    }

    pub fn total_hits(&self) -> usize {
        self.state.hits.lock().unwrap().len()
    }
}

pub const LIST_PATH: &str = "/v2/pnl/leaderboard/kols/period";
