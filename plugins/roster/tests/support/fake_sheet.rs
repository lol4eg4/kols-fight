//! A fake sheet host on a local port. Lives outside `backend/` because the architecture
//! test keeps `Router` and `axum::serve` out of plugin backends; this is test-only code.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// What the fake answers.
#[derive(Clone)]
pub struct Answer {
    pub status: u16,
    pub body: String,
    /// Wait this long before answering.
    pub delay: Duration,
}

pub struct FakeSheet {
    /// Base URL, `http://127.0.0.1:<port>`.
    pub base: String,
    pub answer: Arc<Mutex<Answer>>,
    pub user_agents: Arc<Mutex<Vec<String>>>,
}

type Shared = (Arc<Mutex<Answer>>, Arc<Mutex<Vec<String>>>);

async fn export(
    State((answer, agents)): State<Shared>,
    headers: HeaderMap,
) -> (StatusCode, [(header::HeaderName, &'static str); 1], String) {
    if let Some(ua) = headers.get(header::USER_AGENT) {
        agents
            .lock()
            .unwrap()
            .push(ua.to_str().unwrap_or("").to_string());
    }
    let a = answer.lock().unwrap().clone();
    if !a.delay.is_zero() {
        tokio::time::sleep(a.delay).await;
    }
    (
        StatusCode::from_u16(a.status).unwrap(),
        [(header::CONTENT_TYPE, "text/csv")],
        a.body,
    )
}

impl FakeSheet {
    pub async fn start(body: &str) -> FakeSheet {
        let answer = Arc::new(Mutex::new(Answer {
            status: 200,
            body: body.to_string(),
            delay: Duration::ZERO,
        }));
        let agents = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route("/export", get(export))
            .with_state((answer.clone(), agents.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        FakeSheet {
            base: format!("http://{addr}"),
            answer,
            user_agents: agents,
        }
    }

    /// The export URL with a secret token, as the real one has.
    pub fn url(&self) -> String {
        format!("{}/export?gid=0&token=fake-secret-token-77", self.base)
    }

    pub fn set(&self, status: u16, body: &str, delay: Duration) {
        *self.answer.lock().unwrap() = Answer {
            status,
            body: body.to_string(),
            delay,
        };
    }
}

/// A host that sends `body` with no Content-Length (the body ends when the connection
/// closes), so only the running byte count can stop an oversize answer. Returns the URL.
pub async fn start_without_length(body: String) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let head = "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nConnection: close\r\n\r\n";
                let _ = sock.write_all(head.as_bytes()).await;
                for part in body.as_bytes().chunks(1024) {
                    if sock.write_all(part).await.is_err() {
                        return;
                    }
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://{addr}/export?token=fake-secret-token-77")
}
