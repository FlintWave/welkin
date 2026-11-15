// SPDX-License-Identifier: GPL-3.0-or-later

use sqlx::any::AnyPoolOptions;
use sqlx::AnyPool;
use tracing::info;

use crate::DataError;

#[derive(Clone)]
pub struct DataPool {
    pool: AnyPool,
}

impl DataPool {
    pub async fn connect(url: &str) -> Result<Self, DataError> {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(25)
            .connect(url)
            .await?;
        info!("Connected to database: {}", redact_url(url));
        Ok(Self { pool })
    }

    pub fn inner(&self) -> &AnyPool {
        &self.pool
    }
}

fn redact_url(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
        }
    }
    url.to_string()
}
