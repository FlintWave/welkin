// SPDX-License-Identifier: LGPL-3.0-or-later

pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub mod accounts;
pub mod agentprefs;
pub mod assets;
pub mod auth;
pub mod authorization;
pub mod avatar;
pub mod cache;
pub mod connectors;
pub mod estate;
pub mod friends;
pub mod gatekeeper;
pub mod grid;
pub mod griduser;
pub mod groups;
pub mod hg_assets;
pub mod hg_friends;
pub mod hg_inventory;
pub mod hg_linker;
pub mod hypergrid;
pub mod inventory;
pub mod login;
pub mod map;
pub mod mute;
pub mod offline_im;
pub mod presence;
pub mod profiles;
pub mod registration;
pub mod remote;
pub mod simulation;
pub mod user_agent;
pub mod wizard;
