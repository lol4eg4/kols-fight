mod common;
use common::*;
use plugin_kolexplorer::parse::*;

#[test]
fn leaderboard_fixture_parses_150_rows() {
    let rows = parse_leaderboard(BOARD_HTML).unwrap();
    assert_eq!(rows.len(), 150);
    assert!(rows.windows(2).all(|w| w[0].rank < w[1].rank));
    let decu = &rows[0];
    assert_eq!(decu.rank, 1);
    assert_eq!(decu.slug, "decu");
    assert_eq!(decu.name, "Decu");
    assert_eq!(decu.address, "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9");
    assert_eq!(decu.x_handle.as_deref(), Some("notdecu"));
    assert_eq!(decu.d1, Some(15953.375041653));
    assert_eq!(decu.d7, Some(111722.12918905));
    assert_eq!(decu.d30, Some(553928.02266837));
    assert_eq!(decu.unreal, Some(-1292.8411644756));
    assert_eq!(decu.total, Some(552635.18150389));
    assert!((decu.wr7.unwrap() - 0.73164218958611).abs() < 1e-12);
    assert!((decu.wr30.unwrap() - 0.70669533169533).abs() < 1e-12);
    assert_eq!(decu.last_tx_ts, Some(1792381024));
}

#[test]
fn empty_values_are_none_not_zero() {
    let rows = parse_leaderboard(BOARD_HTML).unwrap();
    assert!(
        rows.iter().any(|r| r.d1.is_none()),
        "fixture has rows with no 24h PnL"
    );
    assert!(
        rows.iter()
            .filter(|r| r.d1.is_none())
            .all(|r| r.d1 != Some(0.0))
    );
}

#[test]
fn under_50_rows_is_an_error() {
    assert_eq!(
        parse_leaderboard(&board_with_rows(49)),
        Err(ParseError::TooFewRows { found: 49 })
    );
    assert_eq!(parse_leaderboard(&board_with_rows(50)).unwrap().len(), 50);
    assert_eq!(
        parse_leaderboard("<html></html>"),
        Err(ParseError::TooFewRows { found: 0 })
    );
}

#[test]
fn missing_attribute_is_an_error() {
    for name in [
        "data-address",
        "data-twitter",
        "data-slug",
        "data-kol-name",
        "data-rank",
        "data-pnl7d",
        "data-wr30d",
        "data-last-tx-ts",
    ] {
        // Rename the attribute on the 10th row only.
        let html = rename_in_row(BOARD_HTML, 10, name);
        match parse_leaderboard(&html) {
            Err(ParseError::MissingAttribute { row: 10, name: n }) => assert_eq!(n, name),
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[test]
fn bad_number_and_bad_address_are_errors() {
    let html = BOARD_HTML.replacen("data-pnl7d=\"111722.12918905\"", "data-pnl7d=\"abc\"", 1);
    assert!(matches!(
        parse_leaderboard(&html),
        Err(ParseError::BadValue {
            row: 1,
            name: "data-pnl7d",
            ..
        })
    ));
    let html = BOARD_HTML.replacen(
        "data-address=\"4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9\"",
        "data-address=\"nope\"",
        1,
    );
    assert!(matches!(
        parse_leaderboard(&html),
        Err(ParseError::BadValue {
            row: 1,
            name: "data-address",
            ..
        })
    ));
}

#[test]
fn profile_fixture() {
    let p = parse_profile(PROFILE_HTML).unwrap();
    assert_eq!(p.avg_pos, Some(342.0));
    assert_eq!(p.hold_s, Some(15.0));
}

#[test]
fn profile_with_one_value_or_none() {
    let only_hold = PROFILE_HTML.replace("Avg position", "Something else");
    let p = parse_profile(&only_hold).unwrap();
    assert_eq!((p.avg_pos, p.hold_s), (None, Some(15.0)));
    let minutes = PROFILE_HTML.replace("<b>15 s</b>Median hold", "<b>13 min</b>Median hold");
    assert_eq!(parse_profile(&minutes).unwrap().hold_s, Some(780.0));
    assert!(parse_profile("<html>profile moved</html>").is_err());
}

/// Renames attribute `name` to `x-renamed` in the `n`th board row (1-based).
fn rename_in_row(html: &str, n: usize, name: &str) -> String {
    let marker = "<tr class=\"board-row\"";
    let mut at = 0;
    for _ in 0..n {
        at += html[at..].find(marker).unwrap() + marker.len();
    }
    let end = at + html[at..].find('>').unwrap();
    let tag = &html[at..end];
    let needle = format!("\n    {name}=");
    assert!(tag.contains(&needle), "{name} not in row {n}");
    let tag = tag.replacen(&needle, "\n    x-renamed=", 1);
    format!("{}{}{}", &html[..at], tag, &html[end..])
}
