// SPDX-License-Identifier: LGPL-3.0-or-later

//! Answers llRequestAgentData lookups for agents the region does not hold.

use welkin_script_vm::builtins::agent_data::{
    AgentDataRequest, DATA_BORN, DATA_NAME, format_born, format_name,
};
use welkin_services::remote::accounts::RemoteAccountsConnector;

/// Resolve `req` against the account service. `None` when the account cannot be reached
/// (the script then gets no dataserver event, as when OpenSim's lookup fails).
pub async fn resolve(accounts: &RemoteAccountsConnector, req: &AgentDataRequest) -> Option<String> {
    let id = req.agent_id.to_string();
    match req.data_type {
        DATA_NAME => accounts.get_name(&id).await.ok().map(|(first, last)| format_name(&first, &last)),
        DATA_BORN => accounts.get_created(&id).await.ok().flatten().map(format_born),
        _ => None,
    }
}
