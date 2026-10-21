//! The real sheet reader: one HTTPS GET of the CSV export.
//!
//! The URL is a secret (it opens the sheet with rates). It is never logged and never part
//! of an error text: every reqwest error is stripped of its URL before it is turned into
//! text.

use crate::roster::SheetFetch;
use async_trait::async_trait;
use std::time::Duration;

/// Names the product; the sheet host sees it.
pub const USER_AGENT: &str = "KOLArena/0.1 (daily read of the network sheet)";

/// Whole request, body included.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// Largest CSV accepted. The real sheet is a few dozen rows, far below this.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;

/// Limits of one read. Tests shorten them.
#[derive(Clone, Debug)]
pub struct Limits {
    pub timeout: Duration,
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            timeout: TIMEOUT,
            max_bytes: MAX_BYTES,
        }
    }
}

/// Reads the sheet over HTTPS with rustls. Plain `http://` is refused, redirects included.
pub struct HttpSheetFetch {
    client: reqwest::Client,
    limits: Limits,
    https_only: bool,
}

impl HttpSheetFetch {
    /// The production reader: HTTPS only, 30 s, 5 MB.
    pub fn new() -> Result<Self, String> {
        Self::build(Limits::default(), true)
    }

    /// Test-only: a reader that also takes plain `http://`, and only to a loopback host
    /// (`127.0.0.1`, `[::1]`, `localhost`): the fake local sheet of the tests. Production
    /// code never calls it; [`HttpSheetFetch::new`] is the only reader the plugin builds.
    #[doc(hidden)]
    pub fn plain_http_for_tests(limits: Limits) -> Result<Self, String> {
        Self::build(limits, false)
    }

    fn build(limits: Limits, https_only: bool) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(limits.timeout)
            .https_only(https_only);
        if !https_only {
            // the test reader never leaves the loopback host
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        let client = builder
            .build()
            .map_err(|e| format!("cannot build the HTTP client: {}", e.without_url()))?;
        Ok(HttpSheetFetch {
            client,
            limits,
            https_only,
        })
    }

    async fn read(&self, url: &str) -> Result<String, String> {
        if !url.starts_with("https://") && (self.https_only || !is_loopback_http(url)) {
            return Err("the sheet URL must start with https://".into());
        }
        let mut resp = self
            .client
            .get(url)
            .header("accept", "text/csv")
            .send()
            .await
            .map_err(|e| self.describe(e))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("the sheet answered HTTP {}", status.as_u16()));
        }
        let max = self.limits.max_bytes;
        if resp.content_length().is_some_and(|n| n > max as u64) {
            return Err(too_big(max));
        }
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| self.describe(e))? {
            if body.len() + chunk.len() > max {
                return Err(too_big(max));
            }
            body.extend_from_slice(&chunk);
        }
        String::from_utf8(body).map_err(|_| "the sheet is not UTF-8 text".to_string())
    }

    /// Error text without the URL.
    fn describe(&self, e: reqwest::Error) -> String {
        if e.is_timeout() {
            format!(
                "the sheet did not answer within {} s",
                self.limits.timeout.as_secs_f32()
            )
        } else if e.is_connect() {
            "cannot connect to the sheet host".into()
        } else if e.is_redirect() {
            "the sheet redirected too often or to a non-HTTPS address".into()
        } else {
            format!("the sheet read failed: {}", e.without_url())
        }
    }
}

/// `http://` to a loopback host, the only plain address the test reader takes.
fn is_loopback_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = match authority.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => authority,
    };
    matches!(host, "127.0.0.1" | "[::1]" | "localhost")
}

fn too_big(max: usize) -> String {
    format!("the sheet is larger than {} bytes", max)
}

#[async_trait]
impl SheetFetch for HttpSheetFetch {
    async fn fetch(&self, url: &str) -> Result<String, String> {
        self.read(url).await
    }
}
