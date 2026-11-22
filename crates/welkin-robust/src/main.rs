// SPDX-License-Identifier: LGPL-3.0-or-later

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

use welkin_config::Config;
use welkin_modules::hg::connector_in::HypergridServiceInConnectorModule;
use welkin_modules::module::ModuleConfig as ModuleConfigTrait;
use welkin_modules::module::RegionModule;
use welkin_modules::registry::SceneRef;
use welkin_data::DataPool;

mod gatekeeper_create_agent;

#[derive(Parser, Debug)]
#[command(name = "robust", about = "OpenSimulator ROBUST grid services server")]
struct Cli {
    #[arg(long, default_value = "robust.toml")]
    config: String,

    #[arg(long, default_value_t = 8002)]
    public_port: u16,

    #[arg(long, default_value_t = 8003)]
    private_port: u16,

    /// Address the private (trusted) service port binds to. It serves
    /// unauthenticated account and auth endpoints, so it defaults to loopback.
    /// Container images set WELKIN_PRIVATE_BIND=0.0.0.0 for the container network.
    #[arg(long, env = "WELKIN_PRIVATE_BIND", default_value = "127.0.0.1")]
    private_bind: String,
}

async fn get_only_filter(req: Request, next: Next) -> axum::response::Response {
    if req.method() != Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    next.run(req).await
}

fn redact_connection_string(url: &str) -> String {
    if let Some(at) = url.find('@')
        && let Some(scheme_end) = url.find("://")
    {
        return format!("{}://***@{}", &url[..scheme_end], &url[at + 1..]);
    }
    url.to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    welkin_runtime::init_tracing(false);

    let cli = Cli::parse();
    info!("Starting ROBUST grid services server");

    let mut config = Config::from_toml_file(std::path::Path::new(&cli.config))?;
    config.apply_env_overrides();
    config.resolve_substitutions()?;

    let conn_str = config.get_string_or("DatabaseService", "RobustConnectionString",
        &config.get_string_or("DatabaseService", "ConnectionString", "sqlite:robust.db"));
    info!(url = %redact_connection_string(&conn_str), "Connecting to database");
    let pool = DataPool::connect(&conn_str).await?;
    info!("Database connection established");

    // --- Run schema migrations ---
    let backend = if conn_str.starts_with("mysql") {
        welkin_data::migrations::Backend::MySQL
    } else if conn_str.starts_with("postgres") {
        welkin_data::migrations::Backend::PostgreSQL
    } else {
        welkin_data::migrations::Backend::SQLite
    };
    let runner = welkin_data::migrations::MigrationRunner::new(pool.inner().clone(), backend);
    if backend == welkin_data::migrations::Backend::SQLite {
        runner.ensure_migrations_table().await?;
        for ddl in &[
            "CREATE TABLE IF NOT EXISTS users (\
             UUID VARCHAR(36) PRIMARY KEY, username VARCHAR(32), surname VARCHAR(32), \
             passwordHash VARCHAR(32), passwordSalt VARCHAR(32), \
             webLoginKey VARCHAR(255), created INT DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS userfriends (\
             ownerID VARCHAR(36), friendID VARCHAR(36), friendPerms INT DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS inventoryfolders(\
             folderName varchar(64), type integer, version integer, \
             folderID varchar(36) primary key, \
             agentID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000')",
            "CREATE TABLE IF NOT EXISTS inventoryitems(\
             assetID varchar(36), assetType integer, inventoryName varchar(64), \
             inventoryDescription varchar(128), inventoryNextPermissions integer, \
             inventoryCurrentPermissions integer, invType integer, creatorID varchar(128), \
             inventoryBasePermissions integer, inventoryEveryOnePermissions integer, \
             salePrice integer default 99, saleType integer default 0, \
             creationDate integer default 2000, \
             groupID varchar(36) default '00000000-0000-0000-0000-000000000000', \
             groupOwned integer default 0, flags integer default 0, \
             inventoryID varchar(36) primary key, \
             parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             avatarID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             inventoryGroupPermissions integer not null default 0)",
            "CREATE INDEX IF NOT EXISTS inventoryfolders_agentid ON inventoryfolders(agentID)",
            "CREATE INDEX IF NOT EXISTS inventoryfolders_parentid ON inventoryfolders(parentFolderID)",
            "CREATE INDEX IF NOT EXISTS inventoryitems_parentfolderid ON inventoryitems(parentFolderID)",
            "CREATE INDEX IF NOT EXISTS inventoryitems_avatarid ON inventoryitems(avatarID)",
            "CREATE TABLE IF NOT EXISTS regions (\
             uuid VARCHAR(36) PRIMARY KEY, regionHandle BIGINT, \
             regionName VARCHAR(128), regionSecret VARCHAR(128), \
             serverIP VARCHAR(64), serverPort INTEGER, serverURI VARCHAR(255), \
             locX INTEGER, locY INTEGER, locZ INTEGER, \
             regionMapTexture VARCHAR(36), serverHttpPort INTEGER, \
             owner_uuid VARCHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             access INTEGER DEFAULT 1, \
             ScopeID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             sizeX INTEGER NOT NULL DEFAULT 256, sizeY INTEGER NOT NULL DEFAULT 256, \
             flags INTEGER NOT NULL DEFAULT 0, last_seen INTEGER NOT NULL DEFAULT 0, \
             PrincipalID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             Token VARCHAR(255) NOT NULL DEFAULT '', \
             parcelMapTexture VARCHAR(36) DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS Presence (\
             UserID VARCHAR(255) NOT NULL, \
             RegionID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             SessionID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             SecureSessionID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             LastSeen TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE UNIQUE INDEX IF NOT EXISTS Presence_SessionID ON Presence(SessionID)",
            "CREATE TABLE IF NOT EXISTS GridUser (\
             UserID VARCHAR(255) NOT NULL PRIMARY KEY, \
             HomeRegionID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             HomePosition CHAR(64) NOT NULL DEFAULT '<0,0,0>', \
             HomeLookAt CHAR(64) NOT NULL DEFAULT '<0,0,0>', \
             LastRegionID CHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000', \
             LastPosition CHAR(64) NOT NULL DEFAULT '<0,0,0>', \
             LastLookAt CHAR(64) NOT NULL DEFAULT '<0,0,0>', \
             Online CHAR(5) NOT NULL DEFAULT 'false', \
             Login CHAR(16) NOT NULL DEFAULT '0', \
             Logout CHAR(16) NOT NULL DEFAULT '0')",
            "CREATE TABLE IF NOT EXISTS im_offline (\
             PrincipalID VARCHAR(36) NOT NULL, \
             FromID VARCHAR(36) NOT NULL, \
             Message TEXT NOT NULL, \
             TMStamp INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS os_groups_groups (\
             GroupID CHAR(36) PRIMARY KEY, \
             Location VARCHAR(255) NOT NULL DEFAULT '', \
             Name VARCHAR(255) NOT NULL DEFAULT '', \
             Charter TEXT NOT NULL DEFAULT '', \
             InsigniaID CHAR(36) NOT NULL DEFAULT '', \
             FounderID CHAR(36) NOT NULL DEFAULT '', \
             MembershipFee INTEGER NOT NULL DEFAULT 0, \
             OpenEnrollment VARCHAR(255) NOT NULL DEFAULT '', \
             ShowInList INTEGER NOT NULL DEFAULT 0, \
             AllowPublish INTEGER NOT NULL DEFAULT 0, \
             MaturePublish INTEGER NOT NULL DEFAULT 0, \
             OwnerRoleID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_membership (\
             GroupID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             SelectedRoleID CHAR(36) NOT NULL DEFAULT '', \
             Contribution INTEGER NOT NULL DEFAULT 0, \
             ListInProfile INTEGER NOT NULL DEFAULT 1, \
             AcceptNotices INTEGER NOT NULL DEFAULT 1, \
             AccessToken CHAR(36) NOT NULL DEFAULT '', \
             PRIMARY KEY (GroupID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_roles (\
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             Name VARCHAR(255) NOT NULL DEFAULT '', \
             Description VARCHAR(255) NOT NULL DEFAULT '', \
             Title VARCHAR(255) NOT NULL DEFAULT '', \
             Powers BIGINT NOT NULL DEFAULT 0, \
             PRIMARY KEY (GroupID, RoleID))",
            "CREATE TABLE IF NOT EXISTS os_groups_rolemembership (\
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             PRIMARY KEY (GroupID, RoleID, PrincipalID))",
            "CREATE TABLE IF NOT EXISTS os_groups_invites (\
             InviteID CHAR(36) PRIMARY KEY, \
             GroupID CHAR(36) NOT NULL, \
             RoleID CHAR(36) NOT NULL, \
             PrincipalID VARCHAR(255) NOT NULL, \
             TMStamp INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE IF NOT EXISTS os_groups_notices (\
             GroupID CHAR(36) NOT NULL, \
             NoticeID CHAR(36) PRIMARY KEY, \
             TMStamp INTEGER NOT NULL DEFAULT 0, \
             FromName VARCHAR(255) NOT NULL DEFAULT '', \
             Subject VARCHAR(255) NOT NULL DEFAULT '', \
             Message TEXT NOT NULL DEFAULT '', \
             HasAttachment INTEGER NOT NULL DEFAULT 0, \
             AttachmentType INTEGER NOT NULL DEFAULT 0, \
             AttachmentName VARCHAR(128) NOT NULL DEFAULT '', \
             AttachmentItemID CHAR(36) NOT NULL DEFAULT '', \
             AttachmentOwnerID CHAR(36) NOT NULL DEFAULT '')",
            "CREATE TABLE IF NOT EXISTS os_groups_principals (\
             PrincipalID VARCHAR(255) PRIMARY KEY, \
             ActiveGroupID CHAR(36) NOT NULL DEFAULT '')",
        ] {
            sqlx::query(ddl).execute(pool.inner()).await?;
        }
        runner.set_version("InventoryStore", 2).await?;
        runner.set_version("GridStore", 10).await?;
        runner.set_version("Presence", 4).await?;
        runner.set_version("GridUserStore", 2).await?;
    }
    runner.run_all().await?;
    info!("Database migrations complete");

    let p = pool.inner().clone();

    let auth_store = Arc::new(welkin_data::auth::AuthStore::new(p.clone()));
    let accounts_store = Arc::new(welkin_data::accounts::UserAccountStore::new(p.clone()));
    let inventory_store = Arc::new(welkin_data::inventory::InventoryStore::new(p.clone()));
    let grid_store = Arc::new(welkin_data::grid::GridStore::new(p.clone()));
    let presence_store = Arc::new(welkin_data::presence::PresenceStore::new(p.clone()));
    let griduser_store = Arc::new(welkin_data::grid_user::GridUserStore::new(p.clone()));
    let friends_store = Arc::new(welkin_data::friends::FriendsStore::new(p.clone()));
    let avatar_store = Arc::new(welkin_data::avatar::AvatarStore::new(p.clone()));
    let agentprefs_store = Arc::new(welkin_data::agent_prefs::AgentPrefsStore::new(p.clone()));
    let mute_store = Arc::new(welkin_data::mute::MuteStore::new(p.clone()));
    let offline_im_store = Arc::new(welkin_data::offline_im::OfflineImStore::new(p.clone()));
    let groups_store = Arc::new(welkin_data::groups::GroupsStore::new(p.clone()));
    let estate_store = Arc::new(welkin_data::estate::EstateStore::new(p.clone()));
    let profiles_store = Arc::new(welkin_data::profiles::ProfilesStore::new(p.clone()));
    let asset_store = Arc::new(welkin_data::assets::AssetStore::new(p.clone()));
    let traveling_data_store = Arc::new(welkin_data::hg_traveling_data::HgTravelingDataStore::new(p.clone()));
    let _ = traveling_data_store.create_table().await;

    // Seed the default asset sets (default avatar body parts, clothing, animations...).
    if config.get_bool_or("AssetService", "AssetLoaderEnabled", true) {
        let loader_args = config.get_string_or("AssetService", "AssetLoaderArgs", "assets/AssetSets.xml");
        let control = PathBuf::from(&loader_args);
        if control.is_file() {
            let report = welkin_services::asset_loader::load_asset_sets(&asset_store, &control).await;
            tracing::info!(?report, "default asset sets loaded");
        } else {
            tracing::warn!(path = %loader_args, "AssetSets.xml not found; default avatar assets are not seeded");
        }
    }

    // --- Hypergrid gatekeeper / user-agent (optional, config-gated) ---
    let hypergrid_enabled = config.get_bool_or("Hypergrid", "Enabled", true);
    let gatekeeper_service = if hypergrid_enabled {
        let gk_url = config.get_string_or(
            "Hypergrid",
            "GatekeeperURL",
            &format!("http://127.0.0.1:{}/", cli.public_port),
        );
        Some(Arc::new(welkin_services::gatekeeper::GatekeeperServiceImpl::new(
            welkin_services::gatekeeper::GatekeeperConfig {
                gatekeeper_url: gk_url,
                gatekeeper_aliases: vec![],
                scope_id: uuid::Uuid::nil(),
                allow_teleports_to_any_region: true,
                foreign_agents_allowed: true,
                foreigns_allowed_exceptions: Vec::new(),
                foreigns_disallowed_exceptions: Vec::new(),
                allowed_clients: None,
                denied_clients: None,
                denied_macs: String::new(),
                denied_id0s: String::new(),
            },
            None,
        )))
    } else {
        None
    };

    let hg_user_management_module = Arc::new(std::sync::RwLock::new(welkin_modules::hg::user_management::HGUserManagementModule::new()));
    let user_agent_service = if hypergrid_enabled {
        let grid_name = config.get_string_or(
            "Hypergrid",
            "GridName",
            &format!("http://127.0.0.1:{}/", cli.public_port),
        );
        Some(Arc::new(welkin_services::user_agent::UserAgentServiceImpl::new(
            welkin_services::user_agent::UserAgentConfig {
                grid_name,
                my_external_ip: config
                    .get_string_or("Hypergrid", "ExternalIP", "127.0.0.1")
                    .to_string(),
                bypass_client_verification: config
                    .get_bool_or("Hypergrid", "BypassClientVerification", false),
                level_outside_contacts: 0,
                show_details: true,
            },
        )))
    } else {
        None
    };

    // Mount HG module registration for HypergridServiceInConnector (config-gated).
    struct ConfigAdapter<'a> { config: &'a Config }
    impl<'a> ModuleConfigTrait for ConfigAdapter<'a> {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.config.get_string_or(section, key, default)
        }
        fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
            self.config
                .get_string(section, key)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.config.get_bool_or(section, key, default)
        }
        fn get_float(&self, section: &str, key: &str, default: f64) -> f64 {
            self.config
                .get_string(section, key)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
    }
    let adapter = ConfigAdapter { config: &config };
    let mut hg_connector_tmp = HypergridServiceInConnectorModule::new();
    let _ = hg_connector_tmp.initialise(&adapter);
    if hypergrid_enabled {
        let scene = SceneRef::new(uuid::Uuid::nil(), "ROBUST".to_string());
        let _ = hg_connector_tmp.add_region(&scene);
    }
    tracing::info!(
        enabled = hypergrid_enabled,
        hg_routes = hg_connector_tmp.route_count(),
        "Hypergrid module registration mounted"
    );
    let hg_connector = std::sync::Arc::new(std::sync::RwLock::new(hg_connector_tmp));

    let tile_dir = Arc::new(PathBuf::from(config.get_string_or(
        "MapService",
        "TileDir",
        "./maptiles",
    )));

    let allowed_delete_types = config
        .get_string_or("AssetService", "AllowedDeleteTypes", "None")
        .parse()
        .unwrap_or(welkin_services::assets::DeleteAllowed::None);

    let gridname = config.get_string_or("GridInfoService", "gridname", "OpenSimulator-Rust Grid");
    let gridnick = config.get_string_or("GridInfoService", "gridnick", "opensimrust");
    let login_uri = format!("http://127.0.0.1:{}/", cli.public_port);

    info!(
        public_port = cli.public_port,
        private_port = cli.private_port,
        "Port configuration"
    );

    // --- PUBLIC router: login, map (GET-only), grid info ---

    let login_state = welkin_services::login::LoginServiceState {
        auth_store: auth_store.clone(),
        accounts_store: accounts_store.clone(),
        inventory_store: inventory_store.clone(),
        grid_store: grid_store.clone(),
        presence_store: presence_store.clone(),
        griduser_store: griduser_store.clone(),
        friends_store: friends_store.clone(),
    };

    let public_map = welkin_services::map::router(welkin_services::map::MapImageServiceState {
        tile_dir: tile_dir.clone(),
    })
    .layer(middleware::from_fn(get_only_filter));

    let mut create_agent_state = gatekeeper_create_agent::CreateAgentState::default();
    if let Some(accounts) = Some(accounts_store.clone()) {
        create_agent_state = create_agent_state.with_accounts_store(accounts);
    }
    if let Some(ua) = &user_agent_service {
        create_agent_state = create_agent_state.with_user_agent(ua.clone());
    }
    create_agent_state = create_agent_state.with_traveling_data_store(traveling_data_store.clone());
    create_agent_state = create_agent_state.with_hg_user_management(hg_user_management_module.clone());
    create_agent_state = create_agent_state.with_hypergrid_connector(hg_connector.clone());
    let create_agent_router = if hypergrid_enabled {
        gatekeeper_create_agent::router(create_agent_state)
    } else {
        Router::new()
    };

    let mut public_router = Router::new()
        .merge(welkin_services::login::router(login_state))
        .merge(public_map)
        .merge(create_agent_router)
        ;
    if hypergrid_enabled {
        if let Some(gk) = &gatekeeper_service {
            public_router = public_router.merge(welkin_services::gatekeeper::router(
                welkin_services::gatekeeper::GatekeeperHandlerState {
                    service: gk.clone(),
                },
            ));
        }
        if let Some(ua) = &user_agent_service {
            public_router = public_router.merge(welkin_services::user_agent::router(
                welkin_services::user_agent::UserAgentHandlerState {
                    service: ua.clone(),
                },
            ));
        }
    }
    let public_router = public_router
        .route("/health", get(welkin_runtime::health))
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
        .merge(welkin_services::auth::router(
            welkin_services::auth::AuthServiceState {
                store: auth_store.clone(),
            },
        ))
        .merge(welkin_services::assets::router(
            welkin_services::assets::AssetServiceState {
                store: asset_store.clone(),
                allowed_delete_types,
            },
        ))
        .merge(welkin_services::inventory::router(
            welkin_services::inventory::InventoryServiceState {
                store: inventory_store.clone(),
            },
        ))
        .merge(welkin_services::grid::router(
            welkin_services::grid::GridServiceState {
                store: grid_store.clone(),
            },
        ))
        .merge(welkin_services::avatar::router(
            welkin_services::avatar::AvatarServiceState {
                store: avatar_store.clone(),
            },
        ))
        .merge(welkin_services::presence::router(
            welkin_services::presence::PresenceServiceState {
                store: presence_store.clone(),
            },
        ))
        .merge(welkin_services::accounts::router(
            welkin_services::accounts::AccountsServiceState {
                store: accounts_store.clone(),
                auth_store: auth_store.clone(),
                inventory_store: inventory_store.clone(),
                allow_create_user: config.get_bool_or("UserAccountService", "AllowCreateUser", false),
            },
        ))
        .merge(welkin_services::griduser::router(
            welkin_services::griduser::GridUserServiceState {
                store: griduser_store.clone(),
            },
        ))
        .merge(welkin_services::friends::router(
            welkin_services::friends::FriendsServiceState {
                store: friends_store.clone(),
            },
        ))
        .merge(welkin_services::agentprefs::router(
            welkin_services::agentprefs::AgentPrefsServiceState {
                store: agentprefs_store.clone(),
            },
        ))
        .merge(welkin_services::mute::router(
            welkin_services::mute::MuteServiceState {
                store: mute_store.clone(),
            },
        ))
        .merge(welkin_services::offline_im::router(
            welkin_services::offline_im::OfflineIMServiceState {
                store: offline_im_store.clone(),
            },
        ))
        .merge(welkin_services::groups::router(
            welkin_services::groups::GroupsServiceState {
                store: groups_store.clone(),
            },
        ))
        .merge(welkin_services::estate::router(
            welkin_services::estate::EstateServiceState {
                store: estate_store.clone(),
            },
        ))
        .merge(welkin_services::profiles::router(
            welkin_services::profiles::ProfilesServiceState {
                store: profiles_store.clone(),
            },
        ))
        .merge(welkin_services::authorization::router(
            welkin_services::authorization::AuthorizationServiceState,
        ))
        .merge(welkin_services::map::router(
            welkin_services::map::MapImageServiceState {
                tile_dir: tile_dir.clone(),
            },
        ));

    // --- Start both listeners ---

    let public_listener = TcpListener::bind(("0.0.0.0", cli.public_port)).await?;
    let private_listener = TcpListener::bind((cli.private_bind.as_str(), cli.private_port)).await?;

    info!(
        port = cli.public_port,
        "Public server listening (login, gridinfo, map)"
    );
    info!(
        port = cli.private_port,
        "Private server listening (all grid services)"
    );

    let public_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(public_listener, public_router)
            .with_graceful_shutdown(welkin_runtime::shutdown_signal())
            .await
        {
            error!("Public server error: {e}");
        }
    });

    if let Err(e) = axum::serve(private_listener, private_router)
        .with_graceful_shutdown(welkin_runtime::shutdown_signal())
        .await
    {
        error!("Private server error: {e}");
    }

    let _ = public_handle.await;
    info!("ROBUST server shutdown complete");
    Ok(())
}
