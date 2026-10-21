mod common;

use arena_platform::{
    Person, Route, Tag,
    scheduler::{DATA_LOCK, lock_key},
    testing::TestDb,
};
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use common::*;
use plugin_schedule::{MIGRATOR, routes};
use serde_json::Value;
use sqlx::{Connection, PgConnection};
use std::time::Duration;
use tower::ServiceExt;

async fn call(route: &Route, method: Method, uri: &str, who: Person) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(who);
    let res = route.handler.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn history_and_run_now() {
    let db = TestDb::with_plugins(&[("schedule", &MIGRATOR)]).await;
    let fakes = Fakes::new();
    let s = fakes.schedule(db.pool().clone(), Duration::from_secs(1));
    let all = routes(&s);
    let read = all
        .iter()
        .find(|r| r.path == "/api/admin/collection")
        .unwrap();
    let run = all
        .iter()
        .find(|r| r.path == "/api/admin/collection/run")
        .unwrap();
    assert_eq!((read.tag, run.tag), (Tag::Staff, Tag::Admin));

    // Nothing yet: both sources listed, no runs.
    let (status, body) = call(read, Method::GET, "/api/admin/collection", Person::staff(2)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["runs"].as_array().unwrap().len(), 0);
    assert_eq!(body["running"], false);
    let sources: Vec<_> = body["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["source"].as_str().unwrap())
        .collect();
    assert_eq!(sources, ["kolexplorer", "solanatracker"]);

    // A date that is not a date.
    let (status, body) = call(
        run,
        Method::POST,
        "/api/admin/collection/run?date=19-10-2026",
        Person::admin(1),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_date"))
    );

    // Run now for today.
    let (status, body) = call(
        run,
        Method::POST,
        "/api/admin/collection/run",
        Person::admin(1),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["trigger"], "manual");
    let id = body["id"].as_i64().unwrap();
    for _ in 0..100 {
        let (_, v) = call(read, Method::GET, "/api/admin/collection", Person::staff(2)).await;
        if v["runs"][0]["status"] != "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (_, body) = call(read, Method::GET, "/api/admin/collection", Person::staff(2)).await;
    assert_eq!(body["runs"][0]["id"].as_i64(), Some(id));
    assert_eq!(body["runs"][0]["status"], "ok");
    assert_eq!(body["sources"][0]["last_ok"]["rows"], 100);
    assert_eq!(body["sources"][1]["last_ok"]["rows"], 14);

    // While the data lock is held the answer is 409 with the text.
    let mut held = PgConnection::connect(db.url()).await.unwrap();
    let _: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(lock_key(DATA_LOCK))
        .fetch_one(&mut held)
        .await
        .unwrap();
    let (status, body) = call(
        run,
        Method::POST,
        "/api/admin/collection/run",
        Person::admin(1),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body["error"]["message"],
        "collection running, try again later"
    );
    assert_eq!(body["error"]["code"], "busy");
    drop(held);
    db.close().await;
}
