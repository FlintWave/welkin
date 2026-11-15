// SPDX-License-Identifier: GPL-3.0-or-later

pub mod pool;
pub mod assets;
pub mod inventory;
pub mod grid;
pub mod accounts;
pub mod auth;
pub mod presence;
pub mod grid_user;
pub mod friends;
pub mod avatar;
pub mod mute;
pub mod profiles;
pub mod offline_im;
pub mod estate;
pub mod agent_prefs;
pub mod migrations;
pub mod simulation;
pub mod groups;
pub mod hg_traveling_data;

pub use pool::DataPool;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
