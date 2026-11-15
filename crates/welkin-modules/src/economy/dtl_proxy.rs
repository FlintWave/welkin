// SPDX-License-Identifier: GPL-3.0-or-later

//! DTL/NSL currency proxy module.
//!
//! Forwards all [`MoneyModule`] calls to an external MoneyServer via XML-RPC.
//! Configured by `[Economy] CurrencyServer = http://host:8008/`.

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

use super::{MoneyModule, MoneyTransactionType};

pub struct DTLProxyModule {
    currency_server_url: String,
    client: reqwest::Client,
    scenes: Vec<String>,
    upload_charge_amount: i32,
    group_creation_charge_amount: i32,
}

impl DTLProxyModule {
    pub fn new() -> Self {
        Self {
            currency_server_url: String::new(),
            client: reqwest::Client::new(),
            scenes: Vec::new(),
            upload_charge_amount: 0,
            group_creation_charge_amount: 0,
        }
    }

    fn xmlrpc_call(&self, method: &str, params_xml: &str) -> Option<String> {
        let body = format!(
            r#"<?xml version="1.0"?><methodCall><methodName>{method}</methodName><params>{params_xml}</params></methodCall>"#
        );

        let url = self.currency_server_url.clone();
        let client = self.client.clone();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .post(&url)
                    .header("Content-Type", "text/xml")
                    .body(body)
                    .send()
                    .await?
                    .text()
                    .await
            })
        });

        match result {
            Ok(text) => Some(text),
            Err(e) => {
                tracing::warn!(method, error = %e, "DTLProxyModule: XML-RPC call failed");
                None
            }
        }
    }

    fn parse_int_value(xml: &str) -> i32 {
        if let Some(start) = xml.find("<int>") {
            let rest = &xml[start + 5..];
            if let Some(end) = rest.find("</int>") {
                return rest[..end].trim().parse().unwrap_or(0);
            }
        }
        if let Some(start) = xml.find("<i4>") {
            let rest = &xml[start + 4..];
            if let Some(end) = rest.find("</i4>") {
                return rest[..end].trim().parse().unwrap_or(0);
            }
        }
        0
    }

    fn parse_bool_value(xml: &str) -> bool {
        if let Some(start) = xml.find("<boolean>") {
            let rest = &xml[start + 9..];
            if let Some(end) = rest.find("</boolean>") {
                return rest[..end].trim() == "1";
            }
        }
        false
    }

    fn uuid_param(id: Uuid) -> String {
        format!("<param><value><string>{id}</string></value></param>")
    }

    fn int_param(v: i32) -> String {
        format!("<param><value><int>{v}</int></value></param>")
    }

    fn string_param(s: &str) -> String {
        format!("<param><value><string>{s}</string></value></param>")
    }
}

impl Default for DTLProxyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl MoneyModule for DTLProxyModule {
    fn object_give_money(
        &self,
        object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
        txn: Uuid,
    ) -> Result<(), String> {
        let params = format!(
            "{}{}{}{}{}",
            Self::uuid_param(object_id),
            Self::uuid_param(from),
            Self::uuid_param(to),
            Self::int_param(amount),
            Self::uuid_param(txn),
        );
        match self.xmlrpc_call("ObjectGiveMoney", &params) {
            Some(resp) if Self::parse_bool_value(&resp) => Ok(()),
            Some(_) => Err("ObjectGiveMoney rejected by MoneyServer".into()),
            None => Err("ObjectGiveMoney XML-RPC call failed".into()),
        }
    }

    fn get_balance(&self, agent_id: Uuid) -> i32 {
        let params = Self::uuid_param(agent_id);
        self.xmlrpc_call("QueryBalance", &params)
            .map(|r| Self::parse_int_value(&r))
            .unwrap_or(0)
    }

    fn upload_covered(&self, agent_id: Uuid, amount: i32) -> bool {
        let params = format!("{}{}", Self::uuid_param(agent_id), Self::int_param(amount));
        self.xmlrpc_call("CheckBalance", &params)
            .map(|r| Self::parse_bool_value(&r))
            .unwrap_or(false)
    }

    fn amount_covered(&self, agent_id: Uuid, amount: i32) -> bool {
        let params = format!("{}{}", Self::uuid_param(agent_id), Self::int_param(amount));
        self.xmlrpc_call("CheckBalance", &params)
            .map(|r| Self::parse_bool_value(&r))
            .unwrap_or(false)
    }

    fn apply_charge(
        &self,
        agent_id: Uuid,
        amount: i32,
        tx_type: MoneyTransactionType,
        extra: &str,
    ) {
        let params = format!(
            "{}{}{}{}",
            Self::uuid_param(agent_id),
            Self::int_param(amount),
            Self::int_param(tx_type as i32),
            Self::string_param(extra),
        );
        let _ = self.xmlrpc_call("ApplyCharge", &params);
    }

    fn apply_upload_charge(&self, agent_id: Uuid, amount: i32, text: &str) {
        self.apply_charge(agent_id, amount, MoneyTransactionType::UploadCharge, text);
    }

    fn move_money(&self, from: Uuid, to: Uuid, amount: i32, text: &str) {
        let params = format!(
            "{}{}{}{}",
            Self::uuid_param(from),
            Self::uuid_param(to),
            Self::int_param(amount),
            Self::string_param(text),
        );
        let _ = self.xmlrpc_call("TransferMoney", &params);
    }

    fn move_money_typed(
        &self,
        from: Uuid,
        to: Uuid,
        amount: i32,
        tx_type: MoneyTransactionType,
        text: &str,
    ) -> bool {
        let params = format!(
            "{}{}{}{}{}",
            Self::uuid_param(from),
            Self::uuid_param(to),
            Self::int_param(amount),
            Self::int_param(tx_type as i32),
            Self::string_param(text),
        );
        self.xmlrpc_call("TransferMoney", &params)
            .map(|r| Self::parse_bool_value(&r))
            .unwrap_or(false)
    }

    fn upload_charge(&self) -> i32 {
        self.upload_charge_amount
    }

    fn group_creation_charge(&self) -> i32 {
        self.group_creation_charge_amount
    }

    fn script_give_money(
        &self,
        object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
    ) -> Result<(), String> {
        self.object_give_money(object_id, from, to, amount, Uuid::new_v4())
    }

    fn script_transfer_lindens(
        &self,
        object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
    ) -> Result<Uuid, String> {
        let txn_id = Uuid::new_v4();
        self.object_give_money(object_id, from, to, amount, txn_id)?;
        Ok(txn_id)
    }

    fn set_pay_price(&self, object_id: Uuid, default_price: i32, buttons: [i32; 4]) {
        tracing::trace!(
            %object_id, default_price,
            ?buttons,
            "DTLProxyModule: set_pay_price (local only, not forwarded)"
        );
    }
}

impl RegionModule for DTLProxyModule {
    fn name(&self) -> &str {
        "DTLProxyModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.currency_server_url = config.get_string("Economy", "CurrencyServer", "");
        self.upload_charge_amount = config.get_string("Economy", "PriceUpload", "0")
            .parse()
            .unwrap_or(0);
        self.group_creation_charge_amount = config.get_string("Economy", "PriceGroupCreate", "0")
            .parse()
            .unwrap_or(0);

        if self.currency_server_url.is_empty() {
            tracing::warn!("DTLProxyModule: no CurrencyServer configured in [Economy]");
        } else {
            tracing::info!(url = %self.currency_server_url, "DTLProxyModule initialised");
        }
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "DTLProxyModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "DTLProxyModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        Ok(())
    }
}
