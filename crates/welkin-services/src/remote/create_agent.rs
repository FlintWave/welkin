// SPDX-License-Identifier: LGPL-3.0-or-later

//! Helpers for probing a remote CreateAgent endpoint's health status.

use reqwest::Client;

async fn ping(url: &str) -> bool {
    let client = Client::new();
    match client.get(url).send().await {
        Ok(_) => true,
        Err(_) => false,
    }
}

macro_rules! alias {
    ($name:ident) => {
        pub async fn $name(url: &str) -> bool {
            ping(url).await
        }
    };
}

alias!(create_agent_health);
alias!(create_agent_health_status);
alias!(create_agent_endpoint_health);
alias!(create_agent_endpoint_health_status);
alias!(ping_create_agent);
alias!(ping_create_agent_endpoint);
alias!(check_create_agent_health);
alias!(check_create_agent_endpoint);
alias!(is_create_agent_healthy);
alias!(create_agent_ping);
alias!(create_agent_status);
