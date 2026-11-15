// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    extract::Request,
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::{error, info};

use opensim_config::Config;
use opensim_data::DataPool;

#[derive(Parser, Debug)]
#[command(name = "robust", about = "OpenSimulator ROBUST grid services server")]
struct Cli {
    #[arg(long, default_value = "robust.toml")]
    config: String,

    #[arg(long, default_value_t = 8002)]
    public_port: u16,

    #[arg(long, default_value_t = 8003)]
    private_port: u16,
}

async fn get_only_filter(req: Request, next: Next) -> axum::response::Response {
    if req.method() != Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    next.run(req).await
}

fn redact_connection_string(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
        }
    }
    url.to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    info!("Starting ROBUST grid services server");

    let mut config = Config::from_toml_file(std::path::Path::new(&cli.config))?;
    config.apply_env_overrides();
    config.resolve_substitutions()?;

    let conn_str = config.get_string_or(
        "DatabaseService",
        "ConnectionString",
        "sqlite:robust.db",
    );
    info!(url = %redact_connection_string(&conn_str), "Connecting to database");
    let pool = DataPool::connect(&conn_str).await?;
    info!("Database connection established");

    let p = pool.inner().clone();

    let auth_store = Arc::new(opensim_data::auth::AuthStore::new(p.clone()));
    let accounts_store = Arc::new(opensim_data::accounts::UserAccountStore::new(p.clone()));
    let inventory_store = Arc::new(opensim_data::inventory::InventoryStore::new(p.clone()));
    let grid_store = Arc::new(opensim_data::grid::GridStore::new(p.clone()));
    let presence_store = Arc::new(opensim_data::presence::PresenceStore::new(p.clone()));
    let griduser_store = Arc::new(opensim_data::grid_user::GridUserStore::new(p.clone()));
    let friends_store = Arc::new(opensim_data::friends::FriendsStore::new(p.clone()));
    let avatar_store = Arc::new(opensim_data::avatar::AvatarStore::new(p.clone()));
    let agentprefs_store = Arc::new(opensim_data::agent_prefs::AgentPrefsStore::new(p.clone()));
    let mute_store = Arc::new(opensim_data::mute::MuteStore::new(p.clone()));
    let offline_im_store = Arc::new(opensim_data::offline_im::OfflineImStore::new(p.clone()));
    let groups_store = Arc::new(opensim_data::groups::GroupsStore::new(p.clone()));
    let estate_store = Arc::new(opensim_data::estate::EstateStore::new(p.clone()));
    let profiles_store = Arc::new(opensim_data::profiles::ProfilesStore::new(p.clone()));
    let asset_store = Arc::new(opensim_data::assets::AssetStore::new(p.clone()));

    let tile_dir = Arc::new(PathBuf::from(
        config.get_string_or("MapService", "TileDir", "./maptiles"),
    ));

    let allowed_delete_types = config
        .get_string_or("AssetService", "AllowedDeleteTypes", "None")
        .parse()
        .unwrap_or(opensim_services::assets::DeleteAllowed::None);

    let gridname = config.get_string_or("GridInfoService", "gridname", "OpenSimulator-Rust Grid");
    let gridnick = config.get_string_or("GridInfoService", "gridnick", "opensimrust");
    let login_uri = format!("http://127.0.0.1:{}/", cli.public_port);

    info!(public_port = cli.public_port, private_port = cli.private_port, "Port configuration");

    // --- PUBLIC router: login, map (GET-only), grid info ---

    let login_state = opensim_services::login::LoginServiceState {
        auth_store: auth_store.clone(),
        accounts_store: accounts_store.clone(),
        inventory_store: inventory_store.clone(),
        grid_store: grid_store.clone(),
        presence_store: presence_store.clone(),
        griduser_store: griduser_store.clone(),
        friends_store: friends_store.clone(),
    };

    let public_map = opensim_services::map::router(opensim_services::map::MapImageServiceState {
        tile_dir: tile_dir.clone(),
    })
    .layer(middleware::from_fn(get_only_filter));

    let public_router = Router::new()
        .merge(opensim_services::login::router(login_state))
        .merge(public_map)
        .route(
            "/get_grid_info",
            get({
                let gridname = gridname.clone();
                let gridnick = gridnick.clone();
                let login_uri = login_uri.clone();
                move || {
                    let gridname = gridname.clone();
                    let gridnick = gridnick.clone();
                    let login_uri = login_uri.clone();
                    async move {
                        let body = format!(
                            r#"{{"platform":"OpenSimulator-Rust","gridname":"{}","gridnick":"{}","login":"{}"}}"#,
                            gridname, gridnick, login_uri
                        );
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, "application/json")],
                            body,
                        )
                    }
                }
            }),
        );

    // --- PRIVATE router: all inter-service endpoints ---

    let private_router = Router::new()
        .merge(opensim_services::auth::router(
            opensim_services::auth::AuthServiceState { store: auth_store.clone() },
        ))
        .merge(opensim_services::assets::router(
            opensim_services::assets::AssetServiceState {
                store: asset_store.clone(),
                allowed_delete_types,
            },
        ))
        .merge(opensim_services::inventory::router(
            opensim_services::inventory::InventoryServiceState { store: inventory_store.clone() },
        ))
        .merge(opensim_services::grid::router(
            opensim_services::grid::GridServiceState { store: grid_store.clone() },
        ))
        .merge(opensim_services::avatar::router(
            opensim_services::avatar::AvatarServiceState { store: avatar_store.clone() },
        ))
        .merge(opensim_services::presence::router(
            opensim_services::presence::PresenceServiceState { store: presence_store.clone() },
        ))
        .merge(opensim_services::accounts::router(
            opensim_services::accounts::AccountsServiceState {
                store: accounts_store.clone(),
                auth_store: auth_store.clone(),
                inventory_store: inventory_store.clone(),
            },
        ))
        .merge(opensim_services::griduser::router(
            opensim_services::griduser::GridUserServiceState { store: griduser_store.clone() },
        ))
        .merge(opensim_services::friends::router(
            opensim_services::friends::FriendsServiceState { store: friends_store.clone() },
        ))
        .merge(opensim_services::agentprefs::router(
            opensim_services::agentprefs::AgentPrefsServiceState { store: agentprefs_store.clone() },
        ))
        .merge(opensim_services::mute::router(
            opensim_services::mute::MuteServiceState { store: mute_store.clone() },
        ))
        .merge(opensim_services::offline_im::router(
            opensim_services::offline_im::OfflineIMServiceState { store: offline_im_store.clone() },
        ))
        .merge(opensim_services::groups::router(
            opensim_services::groups::GroupsServiceState { store: groups_store.clone() },
        ))
        .merge(opensim_services::estate::router(
            opensim_services::estate::EstateServiceState { store: estate_store.clone() },
        ))
        .merge(opensim_services::profiles::router(
            opensim_services::profiles::ProfilesServiceState { store: profiles_store.clone() },
        ))
        .merge(opensim_services::authorization::router(
            opensim_services::authorization::AuthorizationServiceState,
        ))
        .merge(opensim_services::map::router(
            opensim_services::map::MapImageServiceState { tile_dir: tile_dir.clone() },
        ));

    // --- Start both listeners ---

    let public_listener = TcpListener::bind(("0.0.0.0", cli.public_port)).await?;
    let private_listener = TcpListener::bind(("0.0.0.0", cli.private_port)).await?;

    info!(port = cli.public_port, "Public server listening (login, gridinfo, map)");
    info!(port = cli.private_port, "Private server listening (all grid services)");

    let public_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(public_listener, public_router)
            .with_graceful_shutdown(opensim_runtime::shutdown_signal())
            .await
        {
            error!("Public server error: {e}");
        }
    });

    if let Err(e) = axum::serve(private_listener, private_router)
        .with_graceful_shutdown(opensim_runtime::shutdown_signal())
        .await
    {
        error!("Private server error: {e}");
    }

    let _ = public_handle.await;
    info!("ROBUST server shutdown complete");
    Ok(())
}
