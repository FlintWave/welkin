// SPDX-License-Identifier: LGPL-3.0-or-later

//! Economy and money handling.
//!
//! Defines the [`MoneyModule`] trait (Rust equivalent of IMoneyModule) and a
//! [`SampleMoneyModule`] that allows all operations at zero cost, matching
//! OpenSim's default behaviour for grids without commerce.

pub mod dtl_proxy;
pub mod gloebit;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// MoneyTransactionType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum MoneyTransactionType {
    SystemGenerated = 0,
    RegionMoneyRequest = 1,
    Gift = 2,
    Purchase = 3,
    GroupCreate = 1002,
    GroupJoin = 1004,
    UploadCharge = 1101,
    LandPassSale = 5002,
    ClassifiedCharge = 5014,
}

// ---------------------------------------------------------------------------
// MoneyModule trait
// ---------------------------------------------------------------------------

pub trait MoneyModule: Send + Sync {
    fn object_give_money(
        &self,
        object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
        txn: Uuid,
    ) -> Result<(), String>;

    fn get_balance(&self, agent_id: Uuid) -> i32;
    fn upload_covered(&self, agent_id: Uuid, amount: i32) -> bool;
    fn amount_covered(&self, agent_id: Uuid, amount: i32) -> bool;

    fn apply_charge(
        &self,
        agent_id: Uuid,
        amount: i32,
        tx_type: MoneyTransactionType,
        extra: &str,
    );

    fn apply_upload_charge(&self, agent_id: Uuid, amount: i32, text: &str);
    fn move_money(&self, from: Uuid, to: Uuid, amount: i32, text: &str);

    fn move_money_typed(
        &self,
        from: Uuid,
        to: Uuid,
        amount: i32,
        tx_type: MoneyTransactionType,
        text: &str,
    ) -> bool;

    fn upload_charge(&self) -> i32;
    fn group_creation_charge(&self) -> i32;

    fn script_give_money(&self, object_id: Uuid, from: Uuid, to: Uuid, amount: i32) -> Result<(), String>;
    fn script_transfer_lindens(&self, object_id: Uuid, from: Uuid, to: Uuid, amount: i32) -> Result<Uuid, String>;
    fn set_pay_price(&self, object_id: Uuid, default_price: i32, buttons: [i32; 4]);
}

// ---------------------------------------------------------------------------
// SampleMoneyModule
// ---------------------------------------------------------------------------

pub struct SampleMoneyModule {
    scenes: Vec<String>,
}

impl SampleMoneyModule {
    pub fn new() -> Self {
        Self {
            scenes: Vec::new(),
        }
    }
}

impl Default for SampleMoneyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl MoneyModule for SampleMoneyModule {
    fn object_give_money(
        &self,
        _object_id: Uuid,
        _from: Uuid,
        _to: Uuid,
        _amount: i32,
        _txn: Uuid,
    ) -> Result<(), String> {
        tracing::trace!("SampleMoneyModule: object_give_money (no-op, always allowed)");
        Ok(())
    }

    fn get_balance(&self, _agent_id: Uuid) -> i32 {
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
        _agent_id: Uuid,
        _amount: i32,
        _tx_type: MoneyTransactionType,
        _extra: &str,
    ) {
        tracing::trace!("SampleMoneyModule: apply_charge (no-op)");
    }

    fn apply_upload_charge(&self, _agent_id: Uuid, _amount: i32, _text: &str) {
        tracing::trace!("SampleMoneyModule: apply_upload_charge (no-op)");
    }

    fn move_money(&self, _from: Uuid, _to: Uuid, _amount: i32, _text: &str) {
        tracing::trace!("SampleMoneyModule: move_money (no-op)");
    }

    fn move_money_typed(
        &self,
        _from: Uuid,
        _to: Uuid,
        _amount: i32,
        _tx_type: MoneyTransactionType,
        _text: &str,
    ) -> bool {
        tracing::trace!("SampleMoneyModule: move_money_typed (no-op, always allowed)");
        true
    }

    fn upload_charge(&self) -> i32 {
        0
    }

    fn group_creation_charge(&self) -> i32 {
        0
    }

    fn script_give_money(&self, _object_id: Uuid, _from: Uuid, _to: Uuid, _amount: i32) -> Result<(), String> {
        tracing::trace!("SampleMoneyModule: script_give_money (no-op, always allowed)");
        Ok(())
    }

    fn script_transfer_lindens(&self, _object_id: Uuid, _from: Uuid, _to: Uuid, _amount: i32) -> Result<Uuid, String> {
        let txn_id = Uuid::new_v4();
        tracing::trace!(%txn_id, "SampleMoneyModule: script_transfer_lindens (no-op, always allowed)");
        Ok(txn_id)
    }

    fn set_pay_price(&self, _object_id: Uuid, _default_price: i32, _buttons: [i32; 4]) {
        tracing::trace!("SampleMoneyModule: set_pay_price (no-op, not persisted)");
    }
}

impl RegionModule for SampleMoneyModule {
    fn name(&self) -> &str {
        "SampleMoneyModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("SampleMoneyModule initialised (zero-cost, allow-all)");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "SampleMoneyModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "SampleMoneyModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EconomySummary — region snapshot for EconomyDataRequest
// ---------------------------------------------------------------------------

/// Lightweight, immutable snapshot of the values a region reports in response
/// to `EconomyDataRequest`. All inputs come from elsewhere (estate settings,
/// capacity config, parcel store); this type only bundles them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EconomySummary {
    /// Price per square meter sourced from `EstateSettings.price_per_meter`.
    pub price_per_meter: i32,
    /// Available script capacity sourced from region capacity config.
    pub available_script_capacity: u32,
    /// Cost per script run (Linden-style); defaults to 0 (free).
    pub cost_per_script_run: i32,
    /// Electron/energy limit reported to clients; defaults to 0.
    pub electron_limit: u32,
    /// Number of parcels in the region.
    pub parcel_count: u32,
}

impl Default for EconomySummary {
    fn default() -> Self {
        Self {
            price_per_meter: 1,
            available_script_capacity: 0,
            cost_per_script_run: 0,
            electron_limit: 0,
            parcel_count: 0,
        }
    }
}

impl EconomySummary {
    pub fn new(price_per_meter: i32, available_script_capacity: u32, parcel_count: u32) -> Self {
        Self {
            price_per_meter,
            available_script_capacity,
            cost_per_script_run: 0,
            electron_limit: 0,
            parcel_count,
        }
    }
}

/// Build an [`EconomySummary`] from estate price-per-meter and region
/// capacity/parcel counts. Cost-per-script-run and the electron limit default
/// to zero.
pub fn build_economy_summary(
    price_per_meter: i32,
    available_script_capacity: u32,
    parcel_count: u32,
) -> EconomySummary {
    EconomySummary::new(price_per_meter, available_script_capacity, parcel_count)
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn defaults_cost_and_electron_limit() {
        let s = build_economy_summary(7, 15000, 4);
        assert_eq!(s.price_per_meter, 7);
        assert_eq!(s.available_script_capacity, 15000);
        assert_eq!(s.parcel_count, 4);
        assert_eq!(s.cost_per_script_run, 0);
        assert_eq!(s.electron_limit, 0);
    }

    #[test]
    fn overrides_allowed() {
        let mut s = EconomySummary::new(1, 2, 3);
        s.cost_per_script_run = 5;
        s.electron_limit = 9;
        assert_eq!(s.cost_per_script_run, 5);
        assert_eq!(s.electron_limit, 9);
    }
}
