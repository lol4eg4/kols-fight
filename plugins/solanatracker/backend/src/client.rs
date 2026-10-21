//! HTTP client for the Solana Tracker Data API: key header, 2.5 requests per second,
//! retry only on 5xx.

use api_solanatracker::StFailure;
use serde::de::DeserializeOwned;
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};
use tokio::{sync::Mutex, time::Instant};

pub const BASE_URL: &str = "https://data.solanatracker.io";

/// Settings of the client; tests point `base_url` at a fake local server.
#[derive(Clone, Debug)]
pub struct Options {
    pub base_url: String,
    /// Minimum time between two requests. The free plan allows 3 per second; at exactly 334 ms
    /// network jitter puts a 4th request in some second and the API answers 429.
    pub min_interval: Duration,
    /// Wait before the second try after a 5xx.
    pub retry_backoff: Duration,
    pub timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            base_url: BASE_URL.to_string(),
            min_interval: Duration::from_millis(400),
            retry_backoff: Duration::from_secs(1),
            timeout: Duration::from_secs(30),
        }
    }
}

/// Tries per request on 5xx (the first one included).
pub const TRIES: u32 = 2;

pub struct Client {
    http: reqwest::Client,
    /// The API key. Never printed: this type has no `Debug`.
    key: String,
    opts: Options,
    /// Start of the last request.
    last: Mutex<Option<Instant>>,
    calls: AtomicU32,
}

impl Client {
    pub fn new(key: String, opts: Options) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(opts.timeout)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Client {
            http,
            key,
            opts,
            last: Mutex::new(None),
            calls: AtomicU32::new(0),
        })
    }

    /// HTTP requests made so far (retries count).
    pub fn calls(&self) -> u32 {
        self.calls.load(Ordering::Relaxed)
    }

    /// Waits for the next free slot.
    async fn slot(&self) {
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            let due = t + self.opts.min_interval;
            if due > Instant::now() {
                tokio::time::sleep_until(due).await;
            }
        }
        *last = Some(Instant::now());
    }

    /// GET `path` (with query) and parse the JSON body.
    ///
    /// 401 -> [`StFailure::KeyRejected`], 403 -> [`StFailure::PlanLacksPnl`], 429 ->
    /// [`StFailure::RateLimited`], other 4xx -> [`StFailure::BadResponse`], none retried.
    /// 5xx is tried [`TRIES`] times with a pause, then [`StFailure::Upstream`].
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, StFailure> {
        let url = format!("{}{}", self.opts.base_url.trim_end_matches('/'), path);
        let mut tries = 0;
        loop {
            tries += 1;
            self.slot().await;
            self.calls.fetch_add(1, Ordering::Relaxed);
            let resp = self
                .http
                .get(&url)
                .header("x-api-key", &self.key)
                .send()
                .await
                .map_err(|e| StFailure::Network {
                    message: e.without_url().to_string(),
                })?;
            let status = resp.status();
            match status.as_u16() {
                200..=299 => {
                    let body = resp.bytes().await.map_err(|e| StFailure::Network {
                        message: e.without_url().to_string(),
                    })?;
                    return serde_json::from_slice(&body).map_err(|e| StFailure::BadResponse {
                        message: e.to_string(),
                    });
                }
                401 => return Err(StFailure::KeyRejected),
                403 => return Err(StFailure::PlanLacksPnl),
                429 => return Err(StFailure::RateLimited),
                500..=599 => {
                    if tries >= TRIES {
                        return Err(StFailure::Upstream {
                            status: status.as_u16(),
                        });
                    }
                    tokio::time::sleep(self.opts.retry_backoff).await;
                }
                other => {
                    return Err(StFailure::BadResponse {
                        message: format!("HTTP {other}"),
                    });
                }
            }
        }
    }
}
