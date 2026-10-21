//! HTTP side of the plugin: the public KOL Explorer pages, politely.

use std::time::Duration;
use tokio::{sync::Mutex, time::Instant};

/// Public site.
pub const BASE_URL: &str = "https://kolexplorer.com";

/// Names the product and how often it reads. Add a contact page URL once the product has a
/// public domain.
pub const USER_AGENT: &str = "KOLArena/0.1 (daily read of public KOL data)";

/// One profile request every 3 seconds.
pub const PROFILE_GAP: Duration = Duration::from_secs(3);

/// Settings of the HTTP client; tests point `base_url` at a fake local server.
#[derive(Clone, Debug)]
pub struct Options {
    pub base_url: String,
    pub user_agent: String,
    /// Minimum time between two profile requests.
    pub profile_gap: Duration,
    pub timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            base_url: BASE_URL.to_string(),
            user_agent: USER_AGENT.to_string(),
            profile_gap: PROFILE_GAP,
            timeout: Duration::from_secs(30),
        }
    }
}

/// A page could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP {0}")]
    Status(u16),
    #[error("{0}")]
    Network(String),
}

pub struct Http {
    client: reqwest::Client,
    opts: Options,
    /// Start of the last profile request.
    last_profile: Mutex<Option<Instant>>,
}

impl Http {
    pub fn new(opts: Options) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent(opts.user_agent.clone())
            .timeout(opts.timeout)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Http {
            client,
            opts,
            last_profile: Mutex::new(None),
        })
    }

    /// The front page with the leaderboard.
    pub async fn leaderboard(&self) -> Result<String, FetchError> {
        self.get("/").await
    }

    /// `/kol/<slug>`, at most one request per `profile_gap`.
    pub async fn profile(&self, slug: &str) -> Result<String, FetchError> {
        {
            let mut last = self.last_profile.lock().await;
            if let Some(t) = *last {
                let due = t + self.opts.profile_gap;
                if due > Instant::now() {
                    tokio::time::sleep_until(due).await;
                }
            }
            *last = Some(Instant::now());
        }
        self.get(&format!("/kol/{}", urlencode(slug))).await
    }

    async fn get(&self, path: &str) -> Result<String, FetchError> {
        let url = format!("{}{}", self.opts.base_url.trim_end_matches('/'), path);
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| FetchError::Network(e.without_url().to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::Status(status.as_u16()));
        }
        resp.text()
            .await
            .map_err(|e| FetchError::Network(e.without_url().to_string()))
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
