//! A fake KOL Explorer on a local port. Lives outside `backend/` because the architecture
//! test keeps `Router` and `axum::serve` out of plugin backends; this is test-only code.
#![allow(dead_code)]

use super::PROFILE_HTML;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

// ------------------------------------------------------------ fake KE site

pub struct SiteState {
    pub board: Mutex<(u16, String)>,
    /// Per slug; a slug not listed gets `default_profile`.
    pub profiles: Mutex<BTreeMap<String, (u16, String)>>,
    pub default_profile: Mutex<(u16, String)>,
    pub hits: Mutex<Vec<(String, Instant)>>,
    pub user_agents: Mutex<Vec<String>>,
}

pub struct FakeSite {
    pub url: String,
    pub state: Arc<SiteState>,
}

impl FakeSite {
    pub async fn start(board: &str) -> FakeSite {
        let state = Arc::new(SiteState {
            board: Mutex::new((200, board.to_string())),
            profiles: Mutex::default(),
            default_profile: Mutex::new((200, PROFILE_HTML.to_string())),
            hits: Mutex::default(),
            user_agents: Mutex::default(),
        });
        async fn front(
            State(s): State<Arc<SiteState>>,
            h: axum::http::HeaderMap,
        ) -> (StatusCode, String) {
            s.hits.lock().unwrap().push(("/".into(), Instant::now()));
            if let Some(ua) = h.get("user-agent") {
                s.user_agents
                    .lock()
                    .unwrap()
                    .push(ua.to_str().unwrap().to_string());
            }
            let (code, body) = s.board.lock().unwrap().clone();
            (StatusCode::from_u16(code).unwrap(), body)
        }
        async fn profile(
            State(s): State<Arc<SiteState>>,
            Path(slug): Path<String>,
        ) -> (StatusCode, String) {
            s.hits
                .lock()
                .unwrap()
                .push((format!("/kol/{slug}"), Instant::now()));
            let (code, body) = s
                .profiles
                .lock()
                .unwrap()
                .get(&slug)
                .cloned()
                .unwrap_or_else(|| s.default_profile.lock().unwrap().clone());
            (StatusCode::from_u16(code).unwrap(), body)
        }
        let app = Router::new()
            .route("/", get(front))
            .route("/kol/{slug}", get(profile))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        FakeSite { url, state }
    }

    pub fn profile_hits(&self) -> usize {
        self.state
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.starts_with("/kol/"))
            .count()
    }

    pub fn board_hits(&self) -> usize {
        self.state
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p == "/")
            .count()
    }
}
