// SPDX-License-Identifier: GPL-3.0-or-later

pub mod assets_request;
pub mod constants;
pub mod control_file;
pub mod external_representation;
pub mod iar_read;
pub mod iar_write;
pub mod land_data;
pub mod oar_read;
pub mod oar_write;
pub mod osp_resolver;
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
    #[error("tar error: {0}")]
    Tar(String),
    #[error("compression error: {0}")]
    Compression(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("permission denied: {0}")]
    Permission(String),
}
