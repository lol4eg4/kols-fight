//! Fake roster, Solana Tracker and KOL Explorer for the wallets tests.
#![allow(dead_code)]

fn handle_key(s: &str) -> String {
    s.trim().trim_start_matches('@').to_lowercase()
}

use api_kolexplorer::{KeError, KeRun, KolexplorerApi};
use api_roster::{NetworkMember, Rates, RosterApi, RosterError, XHandle};
use api_solanatracker::{
    KolList, SolanatrackerApi, StError, StFailure, StRun, StWallet, WalletSummary,
};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
pub struct FakeRoster {
    pub members: Mutex<Vec<NetworkMember>>,
    /// kol -> (handle, verified)
    pub handles: Mutex<BTreeMap<String, (String, bool)>>,
}

impl FakeRoster {
    pub fn add(&self, kol: &str, name: &str, fomo: &str, sheet_wallets: &[&str]) {
        self.members.lock().unwrap().push(NetworkMember {
            kol: kol.to_string(),
            name: name.to_string(),
            x_handle: Some(fomo.to_string()),
            x_verified: false,
            fomo_handle: Some(fomo.to_string()),
            pumpfun: None,
            sheet_wallets: sheet_wallets.iter().map(|s| s.to_string()).collect(),
            formats: Vec::new(),
            keep: false,
        });
        self.handles
            .lock()
            .unwrap()
            .insert(kol.to_string(), (fomo.to_string(), false));
    }

    pub fn set_sheet(&self, kol: &str, wallets: &[&str]) {
        let mut ms = self.members.lock().unwrap();
        ms.iter_mut().find(|m| m.kol == kol).unwrap().sheet_wallets =
            wallets.iter().map(|s| s.to_string()).collect();
    }

    pub fn verify(&self, kol: &str, handle: &str) {
        self.handles
            .lock()
            .unwrap()
            .insert(kol.to_string(), (handle.to_string(), true));
    }

    pub fn remove(&self, kol: &str) {
        self.members.lock().unwrap().retain(|m| m.kol != kol);
    }
}

#[async_trait]
impl RosterApi for FakeRoster {
    async fn members(&self, person: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        let mut ms = self.members.lock().unwrap().clone();
        if !person.is_staff() {
            ms.iter_mut().for_each(|m| m.sheet_wallets.clear());
        }
        Ok(ms)
    }

    async fn is_member(&self, _: &Person, kol: &str) -> Result<bool, RosterError> {
        Ok(self.members.lock().unwrap().iter().any(|m| m.kol == kol))
    }

    async fn verified_handle(&self, _: &Person, kol: &str) -> Result<Option<XHandle>, RosterError> {
        Ok(self.handles.lock().unwrap().get(kol).map(|(h, v)| XHandle {
            handle: h.clone(),
            verified: *v,
        }))
    }

    async fn rates(&self, _: &Person, _: &[String]) -> Result<Vec<Rates>, RosterError> {
        Err(RosterError::Forbidden)
    }
}

// ----- Solana Tracker and KOL Explorer ---------------------------------------------------

#[derive(Default)]
pub struct FakeSolanaTracker {
    /// address -> last trade (None: the wallet has no trades)
    pub wallets: Mutex<BTreeMap<String, Option<DateTime<Utc>>>>,
    /// handle (lowercase) -> identity
    pub identities: Mutex<BTreeMap<String, StWallet>>,
    pub failing: AtomicBool,
    pub summary_calls: Mutex<Vec<String>>,
}

impl FakeSolanaTracker {
    pub fn trades(&self, address: &str) {
        self.wallets
            .lock()
            .unwrap()
            .insert(address.to_string(), Some(Utc::now()));
    }

    pub fn no_trades(&self, address: &str) {
        self.wallets
            .lock()
            .unwrap()
            .insert(address.to_string(), None);
    }

    pub fn identity(&self, handle: &str, address: &str, fomo: Option<&str>) {
        self.identities.lock().unwrap().insert(
            handle_key(handle),
            StWallet {
                address: address.to_string(),
                name: None,
                x_handle: Some(handle.to_string()),
                fomo_handle: fomo.map(str::to_string),
            },
        );
    }
}

#[async_trait]
impl SolanatrackerApi for FakeSolanaTracker {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<StRun, StError> {
        Err(StError::Off)
    }

    async fn kol_list(&self, _: &Person, _: NaiveDate) -> Result<Option<KolList>, StError> {
        Err(StError::Off)
    }

    async fn wallet_by_handle(&self, _: &Person, x: &str) -> Result<Option<StWallet>, StError> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(StError::Source {
                failure: StFailure::RateLimited,
            });
        }
        Ok(self.identities.lock().unwrap().get(&handle_key(x)).cloned())
    }

    async fn wallet_summary(&self, _: &Person, address: &str) -> Result<WalletSummary, StError> {
        self.summary_calls.lock().unwrap().push(address.to_string());
        if self.failing.load(Ordering::SeqCst) {
            return Err(StError::Source {
                failure: StFailure::Upstream { status: 503 },
            });
        }
        let last = self.wallets.lock().unwrap().get(address).copied().flatten();
        Ok(WalletSummary {
            address: address.to_string(),
            has_trades: last.is_some(),
            last_trade_at: last,
        })
    }
}

#[derive(Default)]
pub struct FakeKolexplorer {
    pub labels: Mutex<BTreeMap<String, String>>,
}

impl FakeKolexplorer {
    pub fn label(&self, handle: &str, address: &str) {
        self.labels
            .lock()
            .unwrap()
            .insert(handle_key(handle), address.to_string());
    }
}

#[async_trait]
impl KolexplorerApi for FakeKolexplorer {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<KeRun, KeError> {
        Err(KeError::Off)
    }

    async fn wallet_by_handle(&self, _: &Person, x: &str) -> Result<Option<String>, KeError> {
        Ok(self.labels.lock().unwrap().get(&handle_key(x)).cloned())
    }
}
