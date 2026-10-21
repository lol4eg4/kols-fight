mod common;

use api_kolexplorer::KeRunStatus;
use api_snapshots::{MemberReason, SnapshotStatus};
use api_solanatracker::{StFailure, StRunStatus};
use arena_platform::{
    Person, RunError,
    scheduler::{DATA_LOCK, lock_key, run_job},
    testing::TestDb,
};
use common::*;
use plugin_schedule::{MIGRATOR, RunStartError, RunView, Schedule, collection};
use sqlx::{Connection, PgConnection};
use std::time::Duration;

const SECOND: Duration = Duration::from_secs(1);

async fn rig() -> (TestDb, Fakes, Schedule) {
    let db = TestDb::with_plugins(&[("schedule", &MIGRATOR)]).await;
    let fakes = Fakes::new();
    fakes.listen();
    let schedule = fakes.schedule(db.pool().clone(), SECOND);
    (db, fakes, schedule)
}

async fn latest_run(s: &Schedule) -> RunView {
    collection(s).await.unwrap().runs.remove(0)
}

fn step<'a>(run: &'a RunView, name: &str) -> &'a plugin_schedule::StepView {
    run.steps.iter().find(|s| s.step == name).unwrap()
}

/// Holds an advisory lock on its own connection until dropped.
async fn hold(db: &TestDb, name: &str) -> PgConnection {
    let mut conn = PgConnection::connect(db.url()).await.unwrap();
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(lock_key(name))
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert!(got);
    conn
}

#[tokio::test]
async fn steps_run_in_order() {
    let (db, fakes, s) = rig().await;
    let d = today();
    fakes
        .snapshots
        .seed(days_ago(1), SnapshotStatus::Closed, &[]);

    run_job(db.pool(), &s.job()).await.unwrap();

    assert_eq!(
        fakes.calls(),
        [
            format!("open {d}"),
            format!("event {d}"),
            "kolexplorer".into(),
            "solanatracker".into(),
            format!("close {d}"),
            format!("rank {d}"),
        ]
    );
    let run = latest_run(&s).await;
    assert_eq!(
        (run.status.as_str(), run.trigger.as_str(), run.date),
        ("ok", "scheduled", d)
    );
    let names: Vec<_> = run.steps.iter().map(|s| s.step.as_str()).collect();
    assert_eq!(
        names,
        [
            "open",
            "event",
            "kolexplorer",
            "solanatracker",
            "close",
            "rank"
        ]
    );
    assert_eq!(step(&run, "kolexplorer").rows, Some(100));
    assert_eq!(
        step(&run, "kolexplorer").note.as_deref(),
        Some("2 profile reads failed")
    );
    assert_eq!(step(&run, "solanatracker").rows, Some(14));
    let view = collection(&s).await.unwrap();
    assert_eq!(view.sources[0].source, "kolexplorer");
    assert_eq!(view.sources[0].last_ok.as_ref().unwrap().rows, Some(100));
    assert!(view.sources[0].last_error.is_none());
    assert!(!view.running);
    db.close().await;
}

#[tokio::test]
async fn kolexplorer_failure_copies_members_when_the_date_has_none() {
    let (db, fakes, s) = rig().await;
    let d = today();
    fakes.snapshots.seed(
        days_ago(1),
        SnapshotStatus::Closed,
        &[
            ("decu", MemberReason::Top100),
            ("ansem", MemberReason::Top100),
        ],
    );
    *fakes.ke.result.lock().unwrap() = Ok(KeRunStatus::Failed);

    run_job(db.pool(), &s.job()).await.unwrap();

    assert_eq!(fakes.snapshots.members_of(d), ["decu", "ansem"]);
    // The failed read does not stop the rest of the run.
    let calls = fakes.calls();
    assert_eq!(
        &calls[calls.len() - 3..],
        [
            "solanatracker".to_string(),
            format!("close {d}"),
            format!("rank {d}")
        ]
    );
    let run = latest_run(&s).await;
    assert_eq!(run.status, "partial");
    let ke = step(&run, "kolexplorer");
    assert_eq!(ke.status, "failed");
    assert_eq!(ke.error.as_deref(), Some("only 12 rows on the page"));
    assert_eq!(
        ke.note.as_deref(),
        Some("top-100 members copied from the previous snapshot")
    );
    let view = collection(&s).await.unwrap();
    assert!(view.sources[0].last_ok.is_none());
    assert_eq!(view.sources[0].last_error.as_ref().unwrap().date, d);
    db.close().await;
}

#[tokio::test]
async fn kolexplorer_error_keeps_members_already_on_the_date() {
    let (db, fakes, s) = rig().await;
    let d = today();
    fakes.snapshots.seed(
        days_ago(1),
        SnapshotStatus::Closed,
        &[("old", MemberReason::Top100)],
    );
    // A rerun of a date that already has members: nothing is copied over them.
    fakes
        .snapshots
        .seed(d, SnapshotStatus::Closed, &[("kept", MemberReason::Top100)]);
    *fakes.ke.result.lock().unwrap() = Err("connection refused".into());

    run_job(db.pool(), &s.job()).await.unwrap();

    assert!(!fakes.calls().iter().any(|c| c.starts_with("copy_members")));
    assert_eq!(fakes.snapshots.members_of(d), ["kept"]);
    let run = latest_run(&s).await;
    let ke = step(&run, "kolexplorer");
    assert!(ke.error.as_deref().unwrap().contains("connection refused"));
    assert_eq!(
        ke.note.as_deref(),
        Some(&*format!("kept the 1 top-100 members already on {d}"))
    );
    db.close().await;
}

#[tokio::test]
async fn solanatracker_failure_is_shown_and_the_run_still_closes_and_ranks() {
    let (db, fakes, s) = rig().await;
    let d = today();
    *fakes.st.result.lock().unwrap() = (StRunStatus::Failed, Some(StFailure::PlanLacksPnl));

    run_job(db.pool(), &s.job()).await.unwrap();

    let calls = fakes.calls();
    assert_eq!(
        &calls[calls.len() - 2..],
        [format!("close {d}"), format!("rank {d}")]
    );
    let run = latest_run(&s).await;
    assert_eq!(run.status, "partial");
    assert_eq!(
        step(&run, "solanatracker").error.as_deref(),
        Some("plan lacks PnL")
    );
    assert_eq!(run.error.as_deref(), Some("solanatracker: plan lacks PnL"));
    db.close().await;
}

#[tokio::test]
async fn close_failure_fails_the_run_and_skips_rank() {
    let (db, fakes, s) = rig().await;
    *fakes.snapshots.fail_close.lock().unwrap() = true;

    assert!(run_job(db.pool(), &s.job()).await.is_err());

    assert!(!fakes.calls().iter().any(|c| c.starts_with("rank")));
    let run = latest_run(&s).await;
    assert_eq!(run.status, "failed");
    assert!(run.error.unwrap().starts_with("close:"));
    db.close().await;
}

#[tokio::test]
async fn manual_run_while_a_run_is_going_says_already_running() {
    let (db, fakes, s) = rig().await;
    let _job = hold(&db, "job:collect").await;

    let err = s
        .start_manual(Person::admin(7), today())
        .await
        .err()
        .unwrap();

    match err {
        RunStartError::Busy(m) => assert!(m.contains("already running"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(fakes.calls().is_empty());
    assert!(collection(&s).await.unwrap().runs.is_empty());
    db.close().await;
}

#[tokio::test]
async fn manual_run_while_the_data_lock_is_held_says_try_again_later() {
    let (db, fakes, s) = rig().await;
    let _data = hold(&db, DATA_LOCK).await;

    let err = s
        .start_manual(Person::admin(7), today())
        .await
        .err()
        .unwrap();

    assert_eq!(
        err,
        RunStartError::Busy("collection running, try again later".into())
    );
    assert!(fakes.calls().is_empty());
    db.close().await;
}

#[tokio::test]
async fn scheduled_run_waits_for_the_data_lock_then_fails_visibly() {
    let (db, fakes, s) = rig().await;
    let data = hold(&db, DATA_LOCK).await;

    let err = run_job(db.pool(), &s.job()).await.unwrap_err();
    assert!(matches!(err, RunError::Failed(_)), "{err}");

    assert!(fakes.calls().is_empty());
    let run = latest_run(&s).await;
    assert_eq!(
        (run.status.as_str(), run.trigger.as_str()),
        ("failed", "scheduled")
    );
    assert_eq!(
        run.error.as_deref(),
        Some("collection running, try again later (waited 1 min)")
    );
    drop(data);
    db.close().await;
}

#[tokio::test]
async fn scheduled_run_starts_when_the_lock_frees_within_the_wait() {
    let (db, fakes, s) = rig().await;
    let data = hold(&db, DATA_LOCK).await;
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        data.close().await.unwrap();
    });

    run_job(db.pool(), &s.job()).await.unwrap();

    release.await.unwrap();
    assert_eq!(fakes.calls().len(), 6);
    assert_eq!(latest_run(&s).await.status, "ok");
    db.close().await;
}

#[tokio::test]
async fn restart_closes_and_ranks_an_old_open_snapshot() {
    let (db, fakes, s) = rig().await;
    let old = days_ago(2);
    let t = today();
    fakes
        .snapshots
        .seed(days_ago(3), SnapshotStatus::Closed, &[]);
    fakes.snapshots.seed(old, SnapshotStatus::Open, &[]);
    fakes.snapshots.seed(t, SnapshotStatus::Open, &[]);
    // The run that died with the server.
    sqlx::query(
        "INSERT INTO schedule.run (date, trigger, started_at, status) VALUES ($1, 'scheduled', now(), 'running')",
    )
    .bind(old)
    .execute(db.pool())
    .await
    .unwrap();

    s.recover().await;

    assert_eq!(
        fakes.calls(),
        [format!("close {old}"), format!("rank {old}")]
    );
    assert_eq!(fakes.snapshots.status_of(old), Some(SnapshotStatus::Closed));
    // Today's snapshot is not older than today: left alone.
    assert_eq!(fakes.snapshots.status_of(t), Some(SnapshotStatus::Open));
    let view = collection(&s).await.unwrap();
    assert!(!view.running);
    let by_trigger = |t: &str| view.runs.iter().find(|r| r.trigger == t).unwrap();
    assert_eq!(by_trigger("scheduled").status, "interrupted");
    assert_eq!(
        by_trigger("scheduled").error.as_deref(),
        Some("the server stopped during the run")
    );
    let recovery = by_trigger("recovery");
    assert_eq!((recovery.status.as_str(), recovery.date), ("ok", old));
    assert_eq!(
        recovery
            .steps
            .iter()
            .map(|s| s.step.as_str())
            .collect::<Vec<_>>(),
        ["close", "rank"]
    );

    // A second start finds nothing to do.
    fakes.log.lock().unwrap().clear();
    s.recover().await;
    assert!(fakes.calls().is_empty());
    db.close().await;
}

#[tokio::test]
async fn manual_rerun_of_an_earlier_date_only_closes_ranks_and_journals() {
    let (db, fakes, s) = rig().await;
    let d = days_ago(3);
    fakes.snapshots.seed(d, SnapshotStatus::Open, &[]);

    let started = s.start_manual(Person::admin(7), d).await.unwrap();
    started.done.await.unwrap();

    // The sources give today's numbers only: they never write into an earlier date.
    assert_eq!(
        fakes.calls(),
        [
            "journal".into(),
            format!("close {d}"),
            format!("rank {d}"),
        ]
    );
    let entries = fakes.journal.entries.lock().unwrap().clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (entries[0].0, entries[0].1.as_str(), entries[0].2.as_str()),
        (Some(7), "schedule/collection", "run_now")
    );
    let run = latest_run(&s).await;
    assert_eq!(
        (
            run.id,
            run.trigger.as_str(),
            run.person_id,
            run.status.as_str()
        ),
        (started.run_id, "manual", Some(7), "ok")
    );
    assert_eq!(run.date, d);
    db.close().await;
}

#[tokio::test]
async fn manual_run_needs_a_known_date_and_a_working_journal() {
    let (db, fakes, s) = rig().await;
    let admin = Person::admin(7);

    assert_eq!(
        s.start_manual(admin, days_ago(5)).await.err(),
        Some(RunStartError::UnknownDate(days_ago(5)))
    );
    let tomorrow = today() + chrono::Duration::days(1);
    assert_eq!(
        s.start_manual(admin, tomorrow).await.err(),
        Some(RunStartError::FutureDate(tomorrow))
    );

    *fakes.journal.fail.lock().unwrap() = true;
    let started = s.start_manual(admin, today()).await.unwrap();
    started.done.await.unwrap();
    assert!(
        fakes.calls().is_empty(),
        "no collection without a journal entry"
    );
    let run = latest_run(&s).await;
    assert_eq!(run.status, "failed");
    assert_eq!(run.error.as_deref(), Some("journal: journal: journal down"));
    db.close().await;
}
