//! Fake registry and journal, shared by the roster and wallets tests. Invented data only.
#![allow(dead_code)]

use api_journal::{Entry, EntryId, JournalApi, JournalError};
use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, PublishedWallet, Reach, ReachRecord, RegistryApi,
    RegistryError, WalletSource,
};
use api_roster::is_solana_address;
use arena_platform::Person;
use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub fn handle_key(s: &str) -> String {
    s.trim().trim_start_matches('@').to_lowercase()
}

/// One `publish_wallets` call: kol, main, others, each address with its source.
pub type Publish = (
    String,
    Option<(String, WalletSource)>,
    Vec<(String, WalletSource)>,
);

#[derive(Default)]
pub struct FakeRegistry {
    pub kols: Mutex<BTreeMap<String, KolProfile>>,
    /// (kol, main, others) of every accepted `publish_wallets`.
    pub publishes: Mutex<Vec<Publish>>,
}

impl FakeRegistry {
    pub fn put(&self, id: &str, name: &str, x_handle: Option<&str>, fomo: Option<&str>) {
        let profile = KolProfile {
            id: id.to_string(),
            name: name.to_string(),
            x_handle: x_handle.map(str::to_string),
            links: KolLinks {
                fomo: fomo.map(str::to_string),
                ..KolLinks::default()
            },
            main_wallet: None,
            other_wallets: Vec::new(),
            reach: None,
            formats: Vec::new(),
        };
        self.kols.lock().unwrap().insert(id.to_string(), profile);
    }

    pub fn published(&self, address: &str, source: WalletSource) -> PublishedWallet {
        PublishedWallet {
            address: address.to_string(),
            source,
            published_at: Utc::now(),
        }
    }

    /// Puts a main wallet into the registry directly (as KOL Explorer or the seed would).
    pub fn set_main(&self, id: &str, address: &str, source: WalletSource) {
        let w = self.published(address, source);
        self.kols.lock().unwrap().get_mut(id).unwrap().main_wallet = Some(w);
    }

    pub fn get(&self, id: &str) -> KolProfile {
        self.kols
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or_else(|| panic!("no KOL {id}"))
    }

    pub fn main_of(&self, id: &str) -> Option<String> {
        self.get(id).main_wallet.map(|w| w.address)
    }

    pub fn others_of(&self, id: &str) -> Vec<String> {
        self.get(id)
            .other_wallets
            .into_iter()
            .map(|w| w.address)
            .collect()
    }
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    s.trim_matches('-').to_string()
}

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, p: &KolUpsert) -> Result<KolId, RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        let id = match &p.id {
            Some(id) => id.clone(),
            None => {
                let base = slug(&p.name);
                let mut id = base.clone();
                let mut n = 2;
                while kols.contains_key(&id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            }
        };
        let entry = kols.entry(id.clone()).or_insert_with(|| KolProfile {
            id: id.clone(),
            name: p.name.clone(),
            x_handle: None,
            links: KolLinks::default(),
            main_wallet: None,
            other_wallets: Vec::new(),
            reach: None,
            formats: Vec::new(),
        });
        entry.name = p.name.clone();
        entry.x_handle = p.x_handle.clone();
        entry.links = p.links.clone();
        Ok(id)
    }

    async fn get_kol(&self, _: &Person, id: &str) -> Result<Option<KolProfile>, RegistryError> {
        Ok(self.kols.lock().unwrap().get(id).cloned())
    }

    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        let kols = self.kols.lock().unwrap();
        Ok(ids.iter().filter_map(|i| kols.get(i).cloned()).collect())
    }

    async fn find_by_handle(&self, _: &Person, x: &str) -> Result<Option<KolId>, RegistryError> {
        let want = handle_key(x);
        Ok(self
            .kols
            .lock()
            .unwrap()
            .values()
            .find(|k| {
                k.x_handle.as_deref().map(handle_key).as_deref() == Some(want.as_str())
                    || k.links.fomo.as_deref().map(handle_key).as_deref() == Some(want.as_str())
            })
            .map(|k| k.id.clone()))
    }

    async fn find_by_wallet(
        &self,
        _: &Person,
        address: &str,
    ) -> Result<Option<KolId>, RegistryError> {
        Ok(self
            .kols
            .lock()
            .unwrap()
            .values()
            .find(|k| {
                k.main_wallet
                    .iter()
                    .chain(&k.other_wallets)
                    .any(|w| w.address == address)
            })
            .map(|k| k.id.clone()))
    }

    async fn publish_wallets(
        &self,
        _: &Person,
        kol: &str,
        main: Option<(&str, WalletSource)>,
        others: &[(String, WalletSource)],
    ) -> Result<(), RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        if !kols.contains_key(kol) {
            return Err(RegistryError::UnknownKol {
                kol: kol.to_string(),
            });
        }
        for (a, _) in main
            .into_iter()
            .chain(others.iter().map(|(a, s)| (a.as_str(), *s)))
        {
            if !is_solana_address(a) {
                return Err(RegistryError::InvalidAddress {
                    address: a.to_string(),
                });
            }
            if let Some(owner) = kols.values().find(|k| {
                k.id != kol
                    && k.main_wallet
                        .iter()
                        .chain(&k.other_wallets)
                        .any(|w| w.address == a)
            }) {
                return Err(RegistryError::WalletTaken {
                    address: a.to_string(),
                    owner: owner.id.clone(),
                });
            }
        }
        let now = Utc::now();
        let wallet = |a: &str, source| PublishedWallet {
            address: a.to_string(),
            source,
            published_at: now,
        };
        let k = kols.get_mut(kol).unwrap();
        k.main_wallet = main.map(|(a, s)| wallet(a, s));
        k.other_wallets = others.iter().map(|(a, s)| wallet(a, *s)).collect();
        self.publishes.lock().unwrap().push((
            kol.to_string(),
            main.map(|(a, s)| (a.to_string(), s)),
            others.to_vec(),
        ));
        Ok(())
    }

    async fn set_reach(
        &self,
        _: &Person,
        kol: &str,
        reach: &Reach,
        source: &str,
        date: NaiveDate,
    ) -> Result<(), RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        let k = kols.get_mut(kol).ok_or(RegistryError::UnknownKol {
            kol: kol.to_string(),
        })?;
        k.reach = Some(ReachRecord {
            reach: reach.clone(),
            source: source.to_string(),
            date,
        });
        Ok(())
    }

    async fn set_formats(
        &self,
        _: &Person,
        kol: &str,
        formats: &[String],
    ) -> Result<(), RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        let k = kols.get_mut(kol).ok_or(RegistryError::UnknownKol {
            kol: kol.to_string(),
        })?;
        k.formats = formats.to_vec();
        Ok(())
    }
}

// ----- journal --------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeJournal {
    pub entries: Mutex<Vec<Entry>>,
    pub fail: AtomicBool,
}

impl FakeJournal {
    pub fn all(&self) -> Vec<Entry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn of(&self, block: &str, object: &str) -> Vec<Entry> {
        self.all()
            .into_iter()
            .filter(|e| e.block == block && e.object == object)
            .collect()
    }

    pub fn actions(&self, block: &str, object: &str) -> Vec<String> {
        self.of(block, object)
            .into_iter()
            .map(|e| e.action)
            .collect()
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&self.all()).unwrap()
    }
}

#[async_trait]
impl JournalApi for FakeJournal {
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(JournalError::Internal {
                message: "journal down".into(),
            });
        }
        let mut entries = self.entries.lock().unwrap();
        let id = entries.len() as i64 + 1;
        entries.push(Entry {
            id,
            at: Utc::now(),
            person_id: person.id,
            role: person.role,
            block: block.to_string(),
            object: object.to_string(),
            action: action.to_string(),
            before,
            after,
        });
        Ok(id)
    }

    async fn history(
        &self,
        _: &Person,
        block: &str,
        object: &str,
    ) -> Result<Vec<Entry>, JournalError> {
        Ok(self.of(block, object))
    }
}

/// Invented base58 addresses (32 bytes each); none belongs to anyone.
pub const ADDR: [&str; 20] = [
    "EG6R4uXz76QUVXXnz2kXgcp7hUMacPzbnoKcemkQPCn9",
    "Dd9u2ZZG9HHgrAudVGrF8NQE7g4xQ81oZyd2mXSA3nG5",
    "3KLDxFB93wubXTT2azkvH484MAX5TZVTCxEjQLzfUnHZ",
    "GrtonqAg26GtsVNS8D1yZRCdrxxhVwQDnSgXHvALKvJv",
    "J8jD6iE5kLBLVAL2FrhGYKtu9t7pCXBFjur8vJ59JWTQ",
    "CfDyskCV37UFqkPBmtuPv6DnyYbvFdYQQLN7i1bRHjxi",
    "AE14S75ECm2DBAXeqYmE4s9JDbeST3SuJb3QwHYsjjjz",
    "6dUpBJzRDjym8UbRn6aCXaehsPKVKWw5tLgG5nZ3foPS",
    "HkmafNM7RqHL3EmQBFAtGxxnhXYSkwB7VLcBy1QEq6Ak",
    "9geh359Qg3dHRKp9WXCi2ZrR3X1Da6ofUbKPjFcJcyZe",
    "6zXfnkfGBKsZzFRJnapNyxj9PVpRq3pn5vSCcAGQ73yC",
    "7Bi1P45k6bjQUPt9mQ6XuzTPVgdRu6cq1qKbPPNZuwso",
    "FUnrABbeJfZ7b6HjexbowWHUCMXNvDxfQPLE7WapoTL6",
    "t3aR2REd98VTnV2CzdHGo3CgEce8YtBNg6qumJvugkC",
    "23YBBG1a381zESCdVKkokb8XPZVFqJb1HKVvpLqhmeYm",
    "2pxePx3ggP9DZQiWdnLvbenjKnXTjyzPEMD43XRwtqTT",
    "9CCrxx8uRazEoQB7LiMijZFiqRRE2Jp6YSEgsAea1M16",
    "DkQs4bvBzr4otwdBT76Gudyk1xFc2WjHZmC1cFHtaJVT",
    "499vptqhquCpcbWoCkjtZeBGRJjHAMgUpH8Ve8TEnHUF",
    "5dHAEeJMtsyFCsESuwTwdGKyYykAfYFnqGCkdtXWwRdy",
];
