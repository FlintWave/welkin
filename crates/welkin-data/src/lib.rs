// SPDX-License-Identifier: LGPL-3.0-or-later

pub mod accounts;
pub mod agent_prefs;
pub mod assets;
pub mod auth;
pub mod avatar;
pub mod estate;
pub mod friends;
pub mod grid;
pub mod grid_user;
pub mod groups;
pub mod hg_traveling_data;
pub mod inventory;
pub mod migrations;
pub mod mute;
pub mod offline_im;
pub mod pool;
pub mod presence;
pub mod profiles;
pub mod simulation;

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
