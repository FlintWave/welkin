// SPDX-License-Identifier: GPL-3.0-or-later

//! Gloebit currency module.
//!
//! Native Rust HTTP client calling the Gloebit REST API. Requires
//! `[Gloebit] GLBKey` and `GLBSecret` in region config. Supports sandbox
//! and production endpoints.

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

use super::{MoneyModule, MoneyTransactionType};

const SANDBOX_URL: &str = "https://sandbox.gloebit.com";
const PRODUCTION_URL: &str = "https://www.gloebit.com";

pub struct GloebitModule {
    glb_key: String,
    glb_secret: String,
    api_url: String,
    sandbox: bool,
    client: reqwest::Client,
    scenes: Vec<String>,
    upload_charge_amount: i32,
    group_creation_charge_amount: i32,
}

impl GloebitModule {
    pub fn new() -> Self {
        Self {
            glb_key: String::new(),
            glb_secret: String::new(),
            api_url: SANDBOX_URL.into(),
            sandbox: true,
            client: reqwest::Client::new(),
            scenes: Vec::new(),
            upload_charge_amount: 0,
            group_creation_charge_amount: 0,
        }
    }

    fn auth_header(&self) -> String {
        use std::io::Write;
        let mut buf = Vec::new();
        write!(buf, "{}:{}", self.glb_key, self.glb_secret).ok();
        let encoded = base64_encode(&buf);
        format!("Basic {encoded}")
    }

    fn api_get(&self, path: &str) -> Option<serde_json::Value> {
        let url = format!("{}{path}", self.api_url);
        let auth = self.auth_header();
        let client = self.client.clone();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .get(&url)
                    .header("Authorization", &auth)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await
            })
        });

        match result {
            Ok(val) => Some(val),
            Err(e) => {
                tracing::warn!(path, error = %e, "GloebitModule: API GET failed");
                None
            }
        }
    }

    fn api_post(&self, path: &str, body: &serde_json::Value) -> Option<serde_json::Value> {
        let url = format!("{}{path}", self.api_url);
        let auth = self.auth_header();
        let client = self.client.clone();
        let body = body.clone();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                client
                    .post(&url)
                    .header("Authorization", &auth)
                    .json(&body)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await
            })
        });

        match result {
            Ok(val) => Some(val),
            Err(e) => {
                tracing::warn!(path, error = %e, "GloebitModule: API POST failed");
                None
            }
        }
    }

    fn is_success(resp: &serde_json::Value) -> bool {
        resp.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

impl Default for GloebitModule {
    fn default() -> Self {
        Self::new()
    }
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < input.len() {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

impl MoneyModule for GloebitModule {
    fn object_give_money(
        &self,
        _object_id: Uuid,
        from: Uuid,
        to: Uuid,
        amount: i32,
        txn: Uuid,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "amount": amount,
            "transaction_id": txn.to_string(),
        });
        match self.api_post("/transact", &body) {
            Some(resp) if Self::is_success(&resp) => Ok(()),
            Some(_) => Err("Gloebit transact rejected".into()),
            None => Err("Gloebit transact call failed".into()),
        }
    }

    fn get_balance(&self, agent_id: Uuid) -> i32 {
        self.api_get(&format!("/balance/{agent_id}"))
            .and_then(|v| v.get("balance")?.as_i64())
            .unwrap_or(0) as i32
    }

    fn upload_covered(&self, agent_id: Uuid, amount: i32) -> bool {
        self.get_balance(agent_id) >= amount
    }

    fn amount_covered(&self, agent_id: Uuid, amount: i32) -> bool {
        self.get_balance(agent_id) >= amount
    }

    fn apply_charge(
        &self,
        agent_id: Uuid,
        amount: i32,
        tx_type: MoneyTransactionType,
        extra: &str,
    ) {
        let body = serde_json::json!({
            "agent_id": agent_id.to_string(),
            "amount": amount,
            "transaction_type": tx_type as i32,
            "description": extra,
        });
        let _ = self.api_post("/charge", &body);
    }

    fn apply_upload_charge(&self, agent_id: Uuid, amount: i32, text: &str) {
        self.apply_charge(agent_id, amount, MoneyTransactionType::UploadCharge, text);
    }

    fn move_money(&self, from: Uuid, to: Uuid, amount: i32, text: &str) {
        let body = serde_json::json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "amount": amount,
            "description": text,
        });
        let _ = self.api_post("/transact", &body);
    }

    fn move_money_typed(
        &self,
        from: Uuid,
        to: Uuid,
        amount: i32,
        _tx_type: MoneyTransactionType,
        text: &str,
    ) -> bool {
        let body = serde_json::json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "amount": amount,
            "description": text,
        });
        self.api_post("/transact", &body)
            .map(|r| Self::is_success(&r))
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
            "GloebitModule: set_pay_price (local only)"
        );
    }
}

impl RegionModule for GloebitModule {
    fn name(&self) -> &str {
        "GloebitModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.glb_key = config.get_string("Gloebit", "GLBKey", "");
        self.glb_secret = config.get_string("Gloebit", "GLBSecret", "");
        self.sandbox = config.get_bool("Gloebit", "Sandbox", true);
        self.api_url = config.get_string(
            "Gloebit",
            "GLBApiUrl",
            if self.sandbox { SANDBOX_URL } else { PRODUCTION_URL },
        );
        self.upload_charge_amount = config.get_string("Economy", "PriceUpload", "0")
            .parse()
            .unwrap_or(0);
        self.group_creation_charge_amount = config.get_string("Economy", "PriceGroupCreate", "0")
            .parse()
            .unwrap_or(0);

        if self.glb_key.is_empty() || self.glb_secret.is_empty() {
            tracing::warn!("GloebitModule: GLBKey or GLBSecret not configured in [Gloebit]");
        } else {
            tracing::info!(
                sandbox = self.sandbox,
                api_url = %self.api_url,
                "GloebitModule initialised"
            );
        }
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "GloebitModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "GloebitModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        Ok(())
    }
}
