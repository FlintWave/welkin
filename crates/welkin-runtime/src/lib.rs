// SPDX-License-Identifier: LGPL-3.0-or-later
use axum::Router;
use axum::routing::get;
use metrics_exporter_prometheus::PrometheusHandle;
use tracing_subscriber::EnvFilter;

pub fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .init();
    }
}

pub fn init_metrics() -> PrometheusHandle {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

pub fn health_router(metrics_handle: PrometheusHandle) -> Router {
    let metrics_handle = metrics_handle.clone();

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/metrics",
            get(move || async move { metrics_handle.render() }),
        )
}

pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    }

    tracing::info!("shutdown signal received");
}
