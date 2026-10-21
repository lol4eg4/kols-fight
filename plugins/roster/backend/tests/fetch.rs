//! The real sheet reader against a fake local sheet host: good CSV, HTTP error, oversize,
//! timeout. The real sheet is never read. No error text carries the URL.

#[path = "../../tests/support/fake_sheet.rs"]
mod fake_sheet;

use fake_sheet::FakeSheet;
use plugin_roster::{HttpSheetFetch, SheetFetch, SheetLimits, parse_sheet};
use std::time::Duration;

const SHEET: &str = include_str!("../../tests/fixtures/sheet.csv");

fn reader(timeout: Duration, max_bytes: usize) -> HttpSheetFetch {
    HttpSheetFetch::plain_http_for_tests(SheetLimits { timeout, max_bytes }).unwrap()
}

fn assert_secret_kept(err: &str, fake: &FakeSheet) {
    assert!(
        !err.contains("fake-secret-token-77"),
        "token in error: {err}"
    );
    assert!(!err.contains(&fake.base), "address in error: {err}");
    assert!(!err.contains("/export"), "path in error: {err}");
}

#[tokio::test]
async fn reads_a_good_csv() {
    let fake = FakeSheet::start(SHEET).await;
    let body = reader(Duration::from_secs(5), 1 << 20)
        .fetch(&fake.url())
        .await
        .unwrap();
    assert_eq!(body, SHEET);
    assert_eq!(parse_sheet(&body).unwrap().rows.len(), 6);
    assert_eq!(
        fake.user_agents.lock().unwrap().as_slice(),
        [plugin_roster::SHEET_USER_AGENT]
    );
}

#[tokio::test]
async fn an_http_error_is_a_visible_error_without_the_url() {
    let fake = FakeSheet::start(SHEET).await;
    fake.set(500, "boom", Duration::ZERO);
    let err = reader(Duration::from_secs(5), 1 << 20)
        .fetch(&fake.url())
        .await
        .unwrap_err();
    assert_eq!(err, "the sheet answered HTTP 500");
    fake.set(403, "", Duration::ZERO);
    let err = reader(Duration::from_secs(5), 1 << 20)
        .fetch(&fake.url())
        .await
        .unwrap_err();
    assert_eq!(err, "the sheet answered HTTP 403");
    assert_secret_kept(&err, &fake);
}

#[tokio::test]
async fn an_oversize_answer_is_refused() {
    let big = "a,b\n".repeat(4096); // 16 KiB
    let fake = FakeSheet::start(&big).await;
    let err = reader(Duration::from_secs(5), 8 * 1024)
        .fetch(&fake.url())
        .await
        .unwrap_err();
    assert_eq!(err, "the sheet is larger than 8192 bytes");
    assert_secret_kept(&err, &fake);
    // exactly at the cap is fine
    let ok = reader(Duration::from_secs(5), big.len())
        .fetch(&fake.url())
        .await
        .unwrap();
    assert_eq!(ok.len(), big.len());
}

#[tokio::test]
async fn an_oversize_answer_without_a_length_is_cut_off() {
    let url = fake_sheet::start_without_length("a,b\n".repeat(4096)).await;
    let err = reader(Duration::from_secs(5), 8 * 1024)
        .fetch(&url)
        .await
        .unwrap_err();
    assert_eq!(err, "the sheet is larger than 8192 bytes");
    let url = fake_sheet::start_without_length(SHEET.to_string()).await;
    let ok = reader(Duration::from_secs(5), 1 << 20)
        .fetch(&url)
        .await
        .unwrap();
    assert_eq!(ok, SHEET);
}

#[tokio::test]
async fn a_slow_host_times_out() {
    let fake = FakeSheet::start(SHEET).await;
    fake.set(200, SHEET, Duration::from_secs(3));
    let started = std::time::Instant::now();
    let err = reader(Duration::from_millis(300), 1 << 20)
        .fetch(&fake.url())
        .await
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(err.starts_with("the sheet did not answer within"), "{err}");
    assert_secret_kept(&err, &fake);
}

#[tokio::test]
async fn a_closed_port_is_a_connect_error_without_the_url() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("http://{addr}/export?token=fake-secret-token-77");
    let err = reader(Duration::from_secs(5), 1 << 20)
        .fetch(&url)
        .await
        .unwrap_err();
    assert_eq!(err, "cannot connect to the sheet host");
}

#[tokio::test]
async fn the_production_reader_refuses_plain_http() {
    let fake = FakeSheet::start(SHEET).await;
    let err = HttpSheetFetch::new()
        .unwrap()
        .fetch(&fake.url())
        .await
        .unwrap_err();
    assert_eq!(err, "the sheet URL must start with https://");
    assert_secret_kept(&err, &fake);
    assert!(
        fake.user_agents.lock().unwrap().is_empty(),
        "no request was sent"
    );
}

#[tokio::test]
async fn the_test_reader_takes_plain_http_only_to_a_loopback_host() {
    for url in [
        "http://sheet.example/export?token=fake-secret-token-77",
        "http://127.0.0.1@sheet.example/export",
        "http://10.0.0.1:8080/export",
        "ftp://127.0.0.1/export",
    ] {
        let err = reader(Duration::from_secs(1), 1024)
            .fetch(url)
            .await
            .unwrap_err();
        assert_eq!(err, "the sheet URL must start with https://", "{url}");
    }
}

#[test]
fn production_limits_are_30_s_and_5_mb() {
    let l = SheetLimits::default();
    assert_eq!(l.timeout, Duration::from_secs(30));
    assert_eq!(l.max_bytes, 5 * 1024 * 1024);
}
