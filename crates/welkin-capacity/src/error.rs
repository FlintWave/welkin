// SPDX-License-Identifier: LGPL-3.0-or-later
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CapacityError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("unavailable: {0}")]
    Unavailable(String),
}
