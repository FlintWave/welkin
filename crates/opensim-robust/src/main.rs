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

    // --- Run schema migrations ---
    let backend = if conn_str.starts_with("mysql") {
        opensim_data::migrations::Backend::MySQL
    } else if conn_str.starts_with("postgres") {
        opensim_data::migrations::Backend::PostgreSQL
    } else {
        opensim_data::migrations::Backend::SQLite
    };
    let runner = opensim_data::migrations::MigrationRunner::new(pool.inner().clone(), backend);
    if backend == opensim_data::migrations::Backend::SQLite {
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
