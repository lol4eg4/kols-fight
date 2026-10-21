//! The plugin's own tables (schema `solanatracker`).

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;

/// One row of a stored list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredKol {
    pub wallet: String,
    pub name: Option<String>,
    pub x_handle: Option<String>,
    pub fomo_handle: Option<String>,
}

type KolRow = (String, Option<String>, Option<String>, Option<String>);

fn stored(r: KolRow) -> StoredKol {
    StoredKol {
        wallet: r.0,
        name: r.1,
        x_handle: r.2,
        fomo_handle: r.3,
    }
}

/// Stores the list of `date`, replacing an earlier one for that date.
pub async fn replace_list(
    pool: &PgPool,
    date: NaiveDate,
    fetched_at: DateTime<Utc>,
    kols: &[StoredKol],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM solanatracker.kol_list WHERE list_date = $1")
        .bind(date)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO solanatracker.kol_list (list_date, fetched_at) VALUES ($1, $2)")
        .bind(date)
        .bind(fetched_at)
        .execute(&mut *tx)
        .await?;
    for (i, k) in kols.iter().enumerate() {
        sqlx::query(
            "INSERT INTO solanatracker.listed_kol (list_date, position, wallet, name, x_handle, fomo_handle)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(date)
        .bind(i as i32 + 1)
        .bind(&k.wallet)
        .bind(&k.name)
        .bind(&k.x_handle)
        .bind(&k.fomo_handle)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// Latest list on or before `date`.
pub async fn list_on_or_before(
    pool: &PgPool,
    date: NaiveDate,
) -> Result<Option<(NaiveDate, DateTime<Utc>, Vec<StoredKol>)>, sqlx::Error> {
    let head: Option<(NaiveDate, DateTime<Utc>)> = sqlx::query_as(
        "SELECT list_date, fetched_at FROM solanatracker.kol_list
         WHERE list_date <= $1 ORDER BY list_date DESC LIMIT 1",
    )
    .bind(date)
    .fetch_optional(pool)
    .await?;
    let Some((list_date, fetched_at)) = head else {
        return Ok(None);
    };
    let rows: Vec<KolRow> = sqlx::query_as(
        "SELECT wallet, name, x_handle, fomo_handle FROM solanatracker.listed_kol
         WHERE list_date = $1 ORDER BY position",
    )
    .bind(list_date)
    .fetch_all(pool)
    .await?;
    Ok(Some((
        list_date,
        fetched_at,
        rows.into_iter().map(stored).collect(),
    )))
}

/// First KOL with X handle `x` in the latest stored list.
pub async fn wallet_by_handle(pool: &PgPool, x: &str) -> Result<Option<StoredKol>, sqlx::Error> {
    let x = x.trim().trim_start_matches('@');
    if x.is_empty() {
        return Ok(None);
    }
    let row: Option<KolRow> = sqlx::query_as(
        "SELECT wallet, name, x_handle, fomo_handle FROM solanatracker.listed_kol
         WHERE list_date = (SELECT max(list_date) FROM solanatracker.kol_list)
           AND lower(x_handle) = lower($1)
         ORDER BY position LIMIT 1",
    )
    .bind(x)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(stored))
}
