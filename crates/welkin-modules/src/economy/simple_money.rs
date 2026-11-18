// SPDX-License-Identifier: LGPL-3.0-or-later

//! "Simple" money module: allow-all defaults (matching SampleMoneyModule) but
//! with balances persisted to a SQL store via `welkin_data::balance`.

use std::collections::HashMap;
use std::sync::RwLock;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

use super::{MoneyModule, MoneyTransactionType};

use welkin_data::balance::AccountBalanceStore;

pub struct SimpleMoneyModule {
    scenes: Vec<String>,
    store: Option<AccountBalanceStore>,
    upload_charge_amount: i32,
    group_creation_charge_amount: i32,
    pay_prices: RwLock<HashMap<Uuid, (i32, [i32; 4])>>,
}

impl SimpleMoneyModule {
    pub fn new() -> Self {
        Self {
            scenes: Vec::new(),
            store: None,
            upload_charge_amount: 0,
            group_creation_charge_amount: 0,
            pay_prices: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_store(store: AccountBalanceStore) -> Self {
        let mut m = Self::new();
        m.store = Some(store);
        m
    }

    pub fn with_balance_store(store: AccountBalanceStore) -> Self {
        Self::with_store(store)
    }

    pub fn from_store(store: AccountBalanceStore) -> Self {
        Self::with_store(store)
    }

    pub fn set_store(&mut self, store: AccountBalanceStore) {
        self.store = Some(store);
    }

    pub fn store(&self) -> Option<AccountBalanceStore> {
        self.store.clone()
    }

    pub fn from_pool(pool: welkin_data::DataPool) -> Self {
        Self::with_store(AccountBalanceStore::new(pool.inner().clone()))
    }

    pub fn with_pool(pool: welkin_data::DataPool) -> Self {
        Self::from_pool(pool)
    }

    async fn adjust_balance(store: AccountBalanceStore, user: Uuid, delta: i64) {
        let current = match store.get_balance(user).await {
            Ok(Some(b)) => b.balance,
            _ => 0,
        };
        let _ = store.set_balance(user, current + delta).await;
    }

}

impl Default for SimpleMoneyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl From<AccountBalanceStore> for SimpleMoneyModule {
    fn from(store: AccountBalanceStore) -> Self {
        Self::with_store(store)
    }
}

impl From<welkin_data::DataPool> for SimpleMoneyModule {
    fn from(pool: welkin_data::DataPool) -> Self {
        Self::from_pool(pool)
    }
}

impl MoneyModule for SimpleMoneyModule {
    fn object_give_money(
        &self,
        _object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
        _txn: Uuid,
    ) -> Result<(), String> {
        if let Some(store) = &self.store {
            let s1 = store.clone();
            let s2 = store.clone();
            let amount = amount as i64;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(s1, from, -amount).await;
                    SimpleMoneyModule::adjust_balance(s2, to, amount).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: object_give_money (allowed, persisted)");
        Ok(())
    }

    fn get_balance(&self, agent_id: Uuid) -> i32 {
        if let Some(store) = &self.store {
            let store = store.clone();
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    store.get_balance(agent_id).await
                })
            });
            if let Ok(Some(b)) = res {
                return b.balance as i32;
            }
        }
        0
    }

    fn upload_covered(&self, _agent_id: Uuid, _amount: i32) -> bool {
        true
    }

    fn amount_covered(&self, _agent_id: Uuid, _amount: i32) -> bool {
        true
    }

    fn apply_charge(
        &self,
        agent_id: Uuid,
        amount: i32,
        _tx_type: MoneyTransactionType,
        _extra: &str,
    ) {
        if let Some(store) = &self.store {
            let store = store.clone();
            let delta = -(amount as i64);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(store, agent_id, delta).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: apply_charge (persisted)");
    }

    fn apply_upload_charge(&self, agent_id: Uuid, amount: i32, _text: &str) {
        if let Some(store) = &self.store {
            let store = store.clone();
            let delta = -(amount as i64);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(store, agent_id, delta).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: apply_upload_charge (persisted)");
    }

    fn move_money(&self, from: Uuid, to: Uuid, amount: i32, _text: &str) {
        if let Some(store) = &self.store {
            let s1 = store.clone();
            let s2 = store.clone();
            let amt = amount as i64;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(s1, from, -amt).await;
                    SimpleMoneyModule::adjust_balance(s2, to, amt).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: move_money (persisted)");
    }

    fn move_money_typed(
        &self,
        from: Uuid,
        to: Uuid,
        amount: i32,
        _tx_type: MoneyTransactionType,
        _text: &str,
    ) -> bool {
        if let Some(store) = &self.store {
            let s1 = store.clone();
            let s2 = store.clone();
            let amt = amount as i64;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(s1, from, -amt).await;
                    SimpleMoneyModule::adjust_balance(s2, to, amt).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: move_money_typed (allowed, persisted)");
        true
    }

    fn upload_charge(&self) -> i32 {
        self.upload_charge_amount
    }

    fn group_creation_charge(&self) -> i32 {
        self.group_creation_charge_amount
    }

    fn script_give_money(&self, _object_id: Uuid, from: Uuid, to: Uuid, amount: i32) -> Result<(), String> {
        if let Some(store) = &self.store {
            let s1 = store.clone();
            let s2 = store.clone();
            let amt = amount as i64;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(s1, from, -amt).await;
                    SimpleMoneyModule::adjust_balance(s2, to, amt).await;
                })
            });
        }
        tracing::trace!("SimpleMoneyModule: script_give_money (allowed, persisted)");
        Ok(())
    }

    fn script_transfer_lindens(&self, _object_id: Uuid, from: Uuid, to: Uuid, amount: i32) -> Result<Uuid, String> {
        let txn_id = Uuid::new_v4();
        if let Some(store) = &self.store {
            let s1 = store.clone();
            let s2 = store.clone();
            let amt = amount as i64;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    SimpleMoneyModule::adjust_balance(s1, from, -amt).await;
                    SimpleMoneyModule::adjust_balance(s2, to, amt).await;
                })
            });
        }
        tracing::trace!(%txn_id, "SimpleMoneyModule: script_transfer_lindens (allowed, persisted)");
        Ok(txn_id)
    }

    fn set_pay_price(&self, object_id: Uuid, default_price: i32, buttons: [i32; 4]) {
        if let Ok(mut guard) = self.pay_prices.write() {
            guard.insert(object_id, (default_price, buttons));
        }
        tracing::trace!("SimpleMoneyModule: set_pay_price (in-memory)");
    }
}

impl RegionModule for SimpleMoneyModule {
    fn name(&self) -> &str {
        "SimpleMoneyModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.upload_charge_amount = config.get_int("Economy", "PriceUpload", 0) as i32;
        self.group_creation_charge_amount = config.get_int("Economy", "PriceGroupCreate", 0) as i32;
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "SimpleMoneyModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "SimpleMoneyModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct Cfg {
        upload: i64,
        group: i64,
    }

    impl ModuleConfig for Cfg {
        fn get_string(&self, _s: &str, _k: &str, d: &str) -> String {
            d.to_string()
        }
        fn get_int(&self, s: &str, k: &str, d: i64) -> i64 {
            if s == "Economy" && k == "PriceUpload" {
                self.upload
            } else if s == "Economy" && k == "PriceGroupCreate" {
                self.group
            } else {
                d
            }
        }
        fn get_bool(&self, _s: &str, _k: &str, d: bool) -> bool {
            d
        }
        fn get_float(&self, _s: &str, _k: &str, d: f64) -> f64 {
            d
        }
    }

    #[test]
    fn new_has_defaults() {
        let m = SimpleMoneyModule::new();
        assert!(m.store().is_none());
        assert_eq!(m.upload_charge(), 0);
        assert_eq!(m.group_creation_charge(), 0);
        let id = Uuid::new_v4();
        assert_eq!(m.get_balance(id), 0);
        assert!(m.upload_covered(id, 42));
        assert!(m.amount_covered(id, 42));
    }

    #[test]
    fn default_trait_works() {
        let m = SimpleMoneyModule::default();
        assert!(m.store().is_none());
    }

    #[test]
    fn move_money_typed_without_store_is_true() {
        let m = SimpleMoneyModule::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        assert!(m.move_money_typed(from, to, 10, MoneyTransactionType::Gift, "hi"));
    }

    #[test]
    fn script_transfer_returns_txn_id_without_store() {
        let m = SimpleMoneyModule::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let txn = m.script_transfer_lindens(Uuid::nil(), from, to, 5).unwrap();
        assert_ne!(txn, Uuid::nil());
    }

    #[test]
    fn script_give_money_ok_without_store() {
        let m = SimpleMoneyModule::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        assert!(m.script_give_money(Uuid::nil(), from, to, 5).is_ok());
    }

    #[test]
    fn set_pay_price_without_panic() {
        let m = SimpleMoneyModule::new();
        m.set_pay_price(Uuid::new_v4(), 7, [1, 2, 3, 4]);
    }

    #[test]
    fn initialise_reads_charges() {
        let mut m = SimpleMoneyModule::new();
        let cfg = Cfg {
            upload: 100,
            group: 200,
        };
        m.initialise(&cfg).unwrap();
        assert_eq!(m.upload_charge(), 100);
        assert_eq!(m.group_creation_charge(), 200);
    }

    #[test]
    fn region_lifecycle_names() {
        let mut m = SimpleMoneyModule::new();
        assert_eq!(m.name(), "SimpleMoneyModule");
        assert!(m.module_type() == crate::module::ModuleType::Shared);
        let scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        m.add_region(&scene).unwrap();
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }


}
