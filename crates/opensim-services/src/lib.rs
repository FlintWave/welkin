// SPDX-License-Identifier: GPL-3.0-or-later

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

pub mod assets;
pub mod inventory;
pub mod grid;
pub mod presence;
pub mod griduser;
pub mod accounts;
pub mod auth;
pub mod authorization;
pub mod friends;
pub mod avatar;
pub mod mute;
pub mod profiles;
pub mod agentprefs;
pub mod map;
pub mod estate;
pub mod simulation;
pub mod login;
