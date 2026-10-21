//! Backend of the `registry` plugin (layer 1): KOLs and their published wallets.
//!
//! A plain store with no routes. Tables `registry.kol` and `registry.wallet`; the wallet
//! address is the key, so one wallet belongs to one KOL. Day-0 KOLs come from the seed
//! migration written by `scripts/gen-seed.ts`.

use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, PublishedWallet, Reach, ReachRecord, RegistryApi,
    RegistryError, WalletSource,
};
use arena_platform::{Ctx, Person, Plugin, PluginEntry, StartError};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, migrate::Migrator};
use std::{collections::HashMap, sync::Arc};

mod base58;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "registry";

/// Migrations from `plugins/registry/migrations`, run in schema `registry`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct RegistryPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.apis.provide::<dyn RegistryApi>(
        NAME,
        vec![("default", Arc::new(Registry::new(ctx.pool.clone())))],
        Arc::new(api_registry::stub::Stub),
    )?;
    Ok(Arc::new(RegistryPlugin))
}

impl Plugin for RegistryPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}

/// The registry over its Postgres tables.
pub struct Registry {
    pool: PgPool,
}

impl Registry {
    pub fn new(pool: PgPool) -> Self {
        Registry { pool }
    }
}

fn internal(e: sqlx::Error) -> RegistryError {
    RegistryError::Internal {
        message: e.to_string(),
    }
}

/// Slug from a name: lowercase ASCII letters and digits, other runs become `-`.
pub fn slug_of(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "kol".to_string()
    } else {
        slug.to_string()
    }
}

fn handle_of(x: &str) -> &str {
    x.trim().trim_start_matches('@')
}

fn source_str(source: WalletSource) -> &'static str {
    match source {
        WalletSource::Prototype => "prototype",
        WalletSource::Kolexplorer => "kolexplorer",
        WalletSource::Network => "network",
    }
}

fn source_of(s: &str) -> Result<WalletSource, RegistryError> {
    match s {
        "prototype" => Ok(WalletSource::Prototype),
        "kolexplorer" => Ok(WalletSource::Kolexplorer),
        "network" => Ok(WalletSource::Network),
        other => Err(RegistryError::Internal {
            message: format!("unknown wallet source {other}"),
        }),
    }
}

#[derive(sqlx::FromRow)]
struct KolRow {
    id: String,
    name: String,
    x_handle: Option<String>,
    kolexplorer: Option<String>,
    solanatracker: bool,
    kolscan: bool,
    fomo: Option<String>,
    pumpfun: Option<String>,
    telegram: Option<String>,
    x_followers: Option<i64>,
    telegram_subscribers: Option<i64>,
    pumpfun_followers: Option<i64>,
    reach_source: Option<String>,
    reach_date: Option<NaiveDate>,
    formats: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct WalletRow {
    address: String,
    kol: String,
    main: bool,
    source: String,
    published_at: DateTime<Utc>,
}

impl Registry {
    async fn profiles(&self, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        let kols: Vec<KolRow> = sqlx::query_as(
            "SELECT id, name, x_handle, kolexplorer, solanatracker, kolscan, fomo, pumpfun, \
             telegram, x_followers, telegram_subscribers, pumpfun_followers, reach_source, \
             reach_date, formats FROM registry.kol WHERE id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        let wallets: Vec<WalletRow> = sqlx::query_as(
            "SELECT address, kol, main, source, published_at FROM registry.wallet \
             WHERE kol = ANY($1) ORDER BY kol, pos, address",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        let mut by_id: HashMap<String, KolProfile> = HashMap::new();
        for k in kols {
            let reach = match (k.reach_source, k.reach_date) {
                (Some(source), Some(date)) => Some(ReachRecord {
                    reach: Reach {
                        x_followers: k.x_followers,
                        telegram_subscribers: k.telegram_subscribers,
                        pumpfun_followers: k.pumpfun_followers,
                    },
                    source,
                    date,
                }),
                _ => None,
            };
            let profile = KolProfile {
                id: k.id.clone(),
                name: k.name,
                x_handle: k.x_handle,
                links: KolLinks {
                    kolexplorer: k.kolexplorer,
                    solanatracker: k.solanatracker,
                    kolscan: k.kolscan,
                    fomo: k.fomo,
                    pumpfun: k.pumpfun,
                    telegram: k.telegram,
                },
                main_wallet: None,
                other_wallets: Vec::new(),
                reach,
                formats: k.formats,
            };
            by_id.insert(k.id, profile);
        }
        for w in wallets {
            let Some(p) = by_id.get_mut(&w.kol) else {
                continue;
            };
            let wallet = PublishedWallet {
                address: w.address,
                source: source_of(&w.source)?,
                published_at: w.published_at,
            };
            if w.main {
                p.main_wallet = Some(wallet);
            } else {
                p.other_wallets.push(wallet);
            }
        }
        Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
    }
}

#[async_trait]
impl RegistryApi for Registry {
    async fn upsert_kol(&self, _: &Person, profile: &KolUpsert) -> Result<KolId, RegistryError> {
        let l = &profile.links;
        let x = profile.x_handle.as_deref().map(handle_of);
        if let Some(id) = &profile.id {
            let updated = sqlx::query(
                "UPDATE registry.kol SET name = $2, x_handle = $3, kolexplorer = $4, \
                 solanatracker = $5, kolscan = $6, fomo = $7, pumpfun = $8, telegram = $9 \
                 WHERE id = $1",
            )
            .bind(id)
            .bind(&profile.name)
            .bind(x)
            .bind(&l.kolexplorer)
            .bind(l.solanatracker)
            .bind(l.kolscan)
            .bind(&l.fomo)
            .bind(&l.pumpfun)
            .bind(&l.telegram)
            .execute(&self.pool)
            .await
            .map_err(internal)?;
            if updated.rows_affected() == 0 {
                return Err(RegistryError::UnknownKol { kol: id.clone() });
            }
            return Ok(id.clone());
        }
        let base = slug_of(&profile.name);
        for n in 1u32.. {
            let id = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            let inserted = sqlx::query(
                "INSERT INTO registry.kol (id, name, x_handle, kolexplorer, solanatracker, \
                 kolscan, fomo, pumpfun, telegram) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&id)
            .bind(&profile.name)
            .bind(x)
            .bind(&l.kolexplorer)
            .bind(l.solanatracker)
            .bind(l.kolscan)
            .bind(&l.fomo)
            .bind(&l.pumpfun)
            .bind(&l.telegram)
            .execute(&self.pool)
            .await
            .map_err(internal)?;
            if inserted.rows_affected() == 1 {
                return Ok(id);
            }
        }
        unreachable!("slug numbers run out")
    }

    async fn get_kol(&self, _: &Person, id: &str) -> Result<Option<KolProfile>, RegistryError> {
        Ok(self.profiles(&[id.to_string()]).await?.pop())
    }

    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        self.profiles(ids).await
    }

    async fn find_by_handle(&self, _: &Person, x: &str) -> Result<Option<KolId>, RegistryError> {
        sqlx::query_scalar(
            "SELECT id FROM registry.kol WHERE lower(x_handle) = lower($1) ORDER BY id LIMIT 1",
        )
        .bind(handle_of(x))
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)
    }

    async fn find_by_wallet(
        &self,
        _: &Person,
        address: &str,
    ) -> Result<Option<KolId>, RegistryError> {
        sqlx::query_scalar("SELECT kol FROM registry.wallet WHERE address = $1")
            .bind(address)
            .fetch_optional(&self.pool)
            .await
            .map_err(internal)
    }

    async fn publish_wallets(
        &self,
        _: &Person,
        kol: &str,
        main: Option<(&str, WalletSource)>,
        others: &[(String, WalletSource)],
    ) -> Result<(), RegistryError> {
        // (address, main, source), main first, each address once.
        let mut wanted: Vec<(&str, bool, WalletSource)> = Vec::new();
        for (address, is_main, source) in main
            .map(|(m, s)| (m, true, s))
            .into_iter()
            .chain(others.iter().map(|(o, s)| (o.as_str(), false, *s)))
        {
            if !base58::is_solana_address(address) {
                return Err(RegistryError::InvalidAddress {
                    address: address.into(),
                });
            }
            if !wanted.iter().any(|(a, _, _)| *a == address) {
                wanted.push((address, is_main, source));
            }
        }
        let mut tx = self.pool.begin().await.map_err(internal)?;
        let found: Option<String> =
            sqlx::query_scalar("SELECT id FROM registry.kol WHERE id = $1 FOR UPDATE")
                .bind(kol)
                .fetch_optional(&mut *tx)
                .await
                .map_err(internal)?;
        if found.is_none() {
            return Err(RegistryError::UnknownKol { kol: kol.into() });
        }
        let addresses: Vec<&str> = wanted.iter().map(|(a, _, _)| *a).collect();
        let taken: Option<(String, String)> = sqlx::query_as(
            "SELECT address, kol FROM registry.wallet WHERE address = ANY($1) AND kol <> $2 \
             ORDER BY address LIMIT 1",
        )
        .bind(&addresses)
        .bind(kol)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?;
        if let Some((address, owner)) = taken {
            return Err(RegistryError::WalletTaken { address, owner });
        }
        sqlx::query("DELETE FROM registry.wallet WHERE kol = $1 AND NOT (address = ANY($2))")
            .bind(kol)
            .bind(&addresses)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        // Clear the main flag first so the one-main index holds while rows change.
        sqlx::query("UPDATE registry.wallet SET main = false WHERE kol = $1")
            .bind(kol)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        for (pos, (address, is_main, source)) in wanted.iter().enumerate() {
            // A wallet this KOL already had keeps its publish time.
            let written = sqlx::query(
                "INSERT INTO registry.wallet (address, kol, main, pos, source, published_at) \
                 VALUES ($1, $2, $3, $4, $5, now()) \
                 ON CONFLICT (address) DO UPDATE SET main = EXCLUDED.main, pos = EXCLUDED.pos, \
                 source = EXCLUDED.source WHERE registry.wallet.kol = EXCLUDED.kol",
            )
            .bind(address)
            .bind(kol)
            .bind(is_main)
            .bind(pos as i32)
            .bind(source_str(*source))
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            if written.rows_affected() == 0 {
                // Another KOL published it between the check and the write.
                let owner: String =
                    sqlx::query_scalar("SELECT kol FROM registry.wallet WHERE address = $1")
                        .bind(address)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(internal)?;
                return Err(RegistryError::WalletTaken {
                    address: address.to_string(),
                    owner,
                });
            }
        }
        tx.commit().await.map_err(internal)
    }

    async fn set_reach(
        &self,
        _: &Person,
        kol: &str,
        reach: &Reach,
        source: &str,
        date: NaiveDate,
    ) -> Result<(), RegistryError> {
        let updated = sqlx::query(
            "UPDATE registry.kol SET x_followers = $2, telegram_subscribers = $3, \
             pumpfun_followers = $4, reach_source = $5, reach_date = $6 WHERE id = $1",
        )
        .bind(kol)
        .bind(reach.x_followers)
        .bind(reach.telegram_subscribers)
        .bind(reach.pumpfun_followers)
        .bind(source)
        .bind(date)
        .execute(&self.pool)
        .await
        .map_err(internal)?;
        if updated.rows_affected() == 0 {
            return Err(RegistryError::UnknownKol { kol: kol.into() });
        }
        Ok(())
    }

    async fn set_formats(
        &self,
        _: &Person,
        kol: &str,
        formats: &[String],
    ) -> Result<(), RegistryError> {
        let updated = sqlx::query("UPDATE registry.kol SET formats = $2 WHERE id = $1")
            .bind(kol)
            .bind(formats)
            .execute(&self.pool)
            .await
            .map_err(internal)?;
        if updated.rows_affected() == 0 {
            return Err(RegistryError::UnknownKol { kol: kol.into() });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_from_names() {
        assert_eq!(slug_of("Mr. Frog"), "mr-frog");
        assert_eq!(slug_of("NDF_Sonar_trade"), "ndf-sonar-trade");
        assert_eq!(slug_of("  J777Crypto! "), "j777crypto");
        assert_eq!(slug_of("Михаил"), "kol");
    }
}
