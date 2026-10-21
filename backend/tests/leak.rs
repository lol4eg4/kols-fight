//! Every public route on the seeded database, with private canaries planted in the network
//! tables: no canary and no key outside the public allowlist reaches a response.

use arena_platform::{Config, Ctx, Tag, testing::TestDb};
use axum::{Router, body::Body, extract::Request, http::StatusCode};
use serde_json::Value;
use server::{APP, PLUGINS};
use std::collections::BTreeSet;
use tower::ServiceExt;

const DATE: &str = "2026-10-18";
/// Top 8 on day 0, for the tournament.
const EIGHT: &str = "decu,cooker,trunoest,tekkerrss,peter,levis,iced,laseasy10";
const CANARIES: [&str; 7] = [
    "CANARY_GROUP",
    "CANARY_NOTE",
    "CANARY_WALLET_NOTE",
    "987654",
    "CanaryHypothesisAddress11111111111111111111",
    "CANARY_EVIDENCE",
    "CANARY_SENT_BY",
];

/// Public routes, pinned: a new public route must be added here (and so looked at).
const PUBLIC_ROUTES: [&str; 12] = [
    "/api/auth/login",
    "/api/board",
    "/api/duel",
    "/api/fight",
    "/api/fight/image",
    "/api/kol/{id}",
    "/api/moves",
    "/api/network",
    "/api/radar",
    "/api/streaks",
    "/api/tournament",
    "/api/tournament/lineups",
];

/// Every JSON key a public response may hold.
const PUBLIC_KEYS: [&str; 93] = [
    "address",
    "all_time",
    "avg_pos",
    "awards",
    "badges",
    "card",
    "changes",
    "closed_at",
    "code",
    "counted_line",
    "counted_rounds",
    "d1",
    "d30",
    "d7",
    "damaged",
    "date",
    "dates",
    "day_climb",
    "days30",
    "days_in_market",
    "decision",
    "error",
    "fetched_at",
    "fighters",
    "fomo",
    "formats",
    "hold_s",
    "hp",
    "in_network",
    "kind",
    "kol",
    "kolexplorer",
    "kols",
    "kolscan",
    "last_min_at_fetch",
    "links",
    "list_date",
    "main",
    "members",
    "message",
    "metrics",
    "missing",
    "name",
    "network",
    "number",
    "outcome",
    "parts",
    "pnl_30d",
    "points",
    "pumpfun",
    "rank",
    "ranked",
    "raw",
    "reason",
    "rounds",
    "rounds_won",
    "rows",
    "score",
    "share",
    "snapshot_closed_at",
    "solanatracker",
    "solscan_url",
    "source",
    "sources",
    "stale",
    "stats",
    "status",
    "streaks",
    "summary",
    "telegram",
    "ticker",
    "tier",
    "top",
    "top100",
    "total",
    "total_d30",
    "tracked",
    "traded_last_hour",
    "type",
    "unreal",
    "up_30d",
    "value",
    "values",
    "vol30",
    "volume_30d",
    "waiting",
    "wallets",
    "week_climb",
    "winner",
    "wr30",
    "wr7",
    "wr_all",
    "x_handle",
];

fn keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                out.insert(k.clone());
                keys(v, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
        _ => {}
    }
}

async fn plant(pool: &sqlx::PgPool) {
    sqlx::raw_sql(
        "INSERT INTO roster.terms (kol, grp, low_usd, high_usd, note, wallet_note)
           SELECT kol, 'CANARY_GROUP', 987654.25, 987655.75, 'CANARY_NOTE', 'CANARY_WALLET_NOTE'
           FROM roster.member
         ON CONFLICT (kol) DO UPDATE SET grp = EXCLUDED.grp, low_usd = EXCLUDED.low_usd,
           high_usd = EXCLUDED.high_usd, note = EXCLUDED.note, wallet_note = EXCLUDED.wallet_note;
         INSERT INTO wallets.candidate (kol, address, status, source, evidence, sent_by)
           SELECT kol, 'CanaryHypothesisAddress11111111111111111111', s, 'fomo',
                  'CANARY_EVIDENCE', 'CANARY_SENT_BY'
           FROM roster.member, unnest(ARRAY['hypothesis', 'not_found', 'conflict']) s
           WHERE kol = 'tekkerrss' AND s = 'hypothesis'
              OR kol = 'peter' AND s = 'not_found'
              OR kol = 'naka' AND s = 'conflict';",
    )
    .execute(pool)
    .await
    .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM wallets.candidate WHERE source = 'fomo'")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 3, "canary candidates planted");
}

#[tokio::test(flavor = "multi_thread")]
async fn public_routes_leak_nothing_private() {
    let db = TestDb::new().await;
    let pool = db.pool().clone();
    let ctx = Ctx::new(pool.clone(), Config::default(), APP).unwrap();
    let plugins = server::build_plugins(&ctx, &PLUGINS).unwrap();
    server::migrate(&pool, &plugins).await.unwrap();
    server::install(&ctx, &plugins).await.unwrap();
    plant(&pool).await;
    let (app, mounted): (Router, _) =
        server::plugin_routes(&ctx, &plugins, &[Tag::Public]).unwrap();

    let paths: BTreeSet<&str> = mounted.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(
        paths,
        PUBLIC_ROUTES.into_iter().collect(),
        "public routes changed"
    );

    let queries = [
        String::new(),
        format!("?date={DATE}"),
        format!("?date={DATE}&ids=decu,cooker,tekkerrss"),
        format!("?ids={EIGHT}&date={DATE}"),
    ];
    let mut seen = BTreeSet::new();
    let mut ok = 0;
    for path in &paths {
        for kol in ["tekkerrss", "peter", "naka", "decu"] {
            let path = path.replace("{id}", kol).replace("{kol}", kol);
            for q in &queries {
                let uri = format!("{path}{q}");
                let req = Request::get(&uri).body(Body::empty()).unwrap();
                let res = app.clone().oneshot(req).await.unwrap();
                let status = res.status();
                let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let text = String::from_utf8_lossy(&body);
                for c in CANARIES {
                    assert!(!text.contains(c), "{uri}: holds canary {c}");
                }
                if status == StatusCode::OK {
                    ok += 1;
                }
                if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                    keys(&v, &mut seen);
                }
            }
        }
    }
    assert!(ok >= paths.len(), "only {ok} answers were 200");
    let allowed: BTreeSet<String> = PUBLIC_KEYS.iter().map(|k| k.to_string()).collect();
    let extra: Vec<_> = seen.difference(&allowed).collect();
    assert!(
        extra.is_empty(),
        "keys outside the public allowlist: {extra:?}"
    );
    db.close().await;
}
