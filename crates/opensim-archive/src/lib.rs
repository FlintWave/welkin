// SPDX-License-Identifier: GPL-3.0-or-later

pub mod constants;
pub mod control_file;
pub mod land_data;
pub mod region_settings;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("XML error: {0}")]
    Xml(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid archive: {0}")]
    Invalid(String),
    #[error("UUID parse error: {0}")]
    Uuid(#[from] uuid::Error),
}
