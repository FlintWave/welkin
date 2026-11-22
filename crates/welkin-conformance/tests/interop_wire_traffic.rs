// SPDX-License-Identifier: LGPL-3.0-or-later

//! Multi-step integration tests simulating viewer/simulator wire-traffic sequences.
//!
//! Covers tasks 3.2 (pCampBot login/rez/teleport) and 3.3 (Aurora Viewer login)
//! scoped as integration tests against recorded wire-traffic patterns, per PM
//! guidance. These tests exercise the same request sequences a real viewer or
//! simulator would send, but against an in-process axum router backed by SQLite.
//!
//! **Live-grid gap:** These tests do NOT verify UDP circuit establishment,
//! capability negotiation, or actual viewer rendering. A live interop test with
//! a C# OpenSimulator region and Aurora Viewer remains untested and should be
//! revisited when infrastructure is available.

use std::sync::Arc;

use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use welkin_data::{
    accounts::UserAccountStore,
    assets::AssetStore,
    auth::AuthStore,
    avatar::AvatarStore,
    friends::FriendsStore,
    grid::GridStore,
    grid_user::GridUserStore,
    inventory::InventoryStore,
    migrations::{Backend, MigrationRunner},
    mute::MuteStore,
    presence::PresenceStore,
};
use welkin_services::{
    accounts::AccountsServiceState,
    assets::{AssetServiceState, DeleteAllowed},
    auth::AuthServiceState,
    avatar::AvatarServiceState,
    friends::FriendsServiceState,
    grid::GridServiceState,
    griduser::GridUserServiceState,
    inventory::InventoryServiceState,
    login::LoginServiceState,
    mute::MuteServiceState,
    presence::PresenceServiceState,
};

const PRINCIPAL: &str = "d6a10998-7b7f-4c8a-b8e5-3e3f4c5d6e7f";
const REGION_UUID: &str = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const ASSET_UUID: &str = "aaaabbbb-cccc-dddd-eeee-ffffffffffff";
const ROOT_FOLDER: &str = "f0f0f0f0-1111-2222-3333-444455556666";
const CHILD_FOLDER: &str = "f0f0f0f0-1111-2222-3333-444455556667";
const ITEM_UUID: &str = "eeeeeeee-dddd-cccc-bbbb-aaaaaaaaaaaa";
const FRIEND_UUID: &str = "12121212-3434-5656-7878-909090909090";
const PASSWORD: &str = "$1$5f4dcc3b5aa765d61d8327deb882cf99";
const SALT: &str = "test-salt-uuid";

// ---------------------------------------------------------------------------
// DB setup (same pattern as replay_grid_services.rs)
// ---------------------------------------------------------------------------

async fn setup_db() -> sqlx::AnyPool {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        sqlx::any::install_default_drivers();
    });
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    for stmt in [
        "CREATE TABLE IF NOT EXISTS users(
            UUID CHAR(36) PRIMARY KEY, username VARCHAR(64), surname VARCHAR(64),
            passwordHash CHAR(32), passwordSalt CHAR(32), webLoginKey VARCHAR(255),
            created INT)",
        "CREATE TABLE IF NOT EXISTS userfriends(
            ownerID CHAR(36), friendID VARCHAR(255), friendPerms INT)",
    ] {
        sqlx::query(stmt).execute(&pool).await.unwrap();
    }

    sqlx::query("CREATE TABLE IF NOT EXISTS migrations(name VARCHAR(100), version INT)")
        .execute(&pool)
        .await
        .unwrap();

    for stmt in [
        "CREATE TABLE IF NOT EXISTS inventoryfolders(
            folderName varchar(64), type integer, version integer,
            folderID varchar(36) primary key,
            agentID varchar(36) not null default '00000000-0000-0000-0000-000000000000',
            parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000')",
        "CREATE TABLE IF NOT EXISTS inventoryitems(
            assetID varchar(36), assetType integer, inventoryName varchar(64),
            inventoryDescription varchar(128), inventoryNextPermissions integer,
            inventoryCurrentPermissions integer, invType integer, creatorID varchar(128),
            inventoryBasePermissions integer, inventoryEveryOnePermissions integer,
            salePrice integer default 99, saleType integer default 0,
            creationDate integer default 2000,
            groupID varchar(36) default '00000000-0000-0000-0000-000000000000',
            groupOwned integer default 0, flags integer default 0,
            inventoryID varchar(36) primary key,
            parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000',
            avatarID varchar(36) not null default '00000000-0000-0000-0000-000000000000',
            inventoryGroupPermissions integer not null default 0)",
        "CREATE INDEX IF NOT EXISTS inventoryfolders_agentid ON inventoryfolders(agentID)",
        "CREATE INDEX IF NOT EXISTS inventoryfolders_parentid ON inventoryfolders(parentFolderID)",
        "CREATE INDEX IF NOT EXISTS inventoryitems_parentfolderid ON inventoryitems(parentFolderID)",
        "CREATE INDEX IF NOT EXISTS inventoryitems_avatarid ON inventoryitems(avatarID)",
    ] {
        sqlx::query(stmt).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO migrations(name, version) VALUES ('InventoryStore', 2)")
        .execute(&pool)
        .await
        .unwrap();

    let runner = MigrationRunner::new(pool.clone(), Backend::SQLite);
    for store in ["FriendsStore", "Avatar", "MuteListStore"] {
        if let Some(content) = welkin_data::migrations::migration_content(store, Backend::SQLite) {
            runner
                .run_migrations(store, content)
                .await
                .unwrap_or_else(|e| panic!("migration failed for {store}: {e}"));
        }
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS auth (
            UUID VARCHAR(36) NOT NULL PRIMARY KEY,
            passwordHash VARCHAR(32) NOT NULL DEFAULT '',
            passwordSalt VARCHAR(32) NOT NULL DEFAULT '',
            webLoginKey VARCHAR(255) NOT NULL DEFAULT '',
            accountType VARCHAR(32) NOT NULL DEFAULT 'UserAccount'
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tokens (
            UUID VARCHAR(36) NOT NULL,
            token VARCHAR(255) NOT NULL,
            validity VARCHAR(32) NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    {
        let content =
            welkin_data::migrations::migration_content("UserAccount", Backend::SQLite).unwrap();
        runner
            .run_migrations("UserAccount", content)
            .await
            .unwrap_or_else(|e| panic!("migration failed for UserAccount: {e}"));
    }
    sqlx::query("DROP TABLE IF EXISTS UserAccounts")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE UserAccounts (
            PrincipalID CHAR(36) PRIMARY KEY,
            ScopeID CHAR(36) NOT NULL,
            FirstName VARCHAR(64) NOT NULL,
            LastName VARCHAR(64) NOT NULL,
            Email VARCHAR(64),
            ServiceURLs TEXT,
            Created INT(11),
            UserLevel INTEGER NOT NULL DEFAULT 0,
            UserFlags INTEGER NOT NULL DEFAULT 0,
            UserTitle VARCHAR(64) NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS assets(
            id VARCHAR(36) NOT NULL PRIMARY KEY,
            name VARCHAR(64), description VARCHAR(64),
            assetType INTEGER NOT NULL, local INTEGER NOT NULL,
            temporary INTEGER NOT NULL, data BLOB,
            create_time INTEGER, access_time INTEGER,
            asset_flags INTEGER NOT NULL DEFAULT 0,
            CreatorID VARCHAR(128) DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS regions(
            uuid VARCHAR(255) PRIMARY KEY, regionHandle BIGINT,
            regionName VARCHAR(255), regionSecret VARCHAR(255) DEFAULT '',
            serverIP VARCHAR(255), serverPort INT, serverURI VARCHAR(255),
            locX INT, locY INT, locZ INT DEFAULT 0,
            regionMapTexture VARCHAR(255) DEFAULT '00000000-0000-0000-0000-000000000000',
            serverHttpPort INT, owner_uuid VARCHAR(36), access INT DEFAULT 1,
            ScopeID VARCHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            sizeX INT DEFAULT 256, sizeY INT DEFAULT 256,
            flags INT DEFAULT 0, last_seen INT DEFAULT 0,
            PrincipalID VARCHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            Token VARCHAR(255) DEFAULT '',
            parcelMapTexture VARCHAR(255) DEFAULT '00000000-0000-0000-0000-000000000000'
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS GridUser(
            UserID VARCHAR(255) PRIMARY KEY,
            HomeRegionID CHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            HomePosition CHAR(64) DEFAULT '<0,0,0>',
            HomeLookAt CHAR(64) DEFAULT '<0,0,0>',
            LastRegionID CHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            LastPosition CHAR(64) DEFAULT '<0,0,0>',
            LastLookAt CHAR(64) DEFAULT '<0,0,0>',
            Online CHAR(5) DEFAULT 'false',
            Login CHAR(16) DEFAULT '0',
            Logout CHAR(16) DEFAULT '0'
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS Presence(
            UserID VARCHAR(255) NOT NULL,
            RegionID VARCHAR(36) NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
            SessionID VARCHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            SecureSessionID VARCHAR(36) DEFAULT '00000000-0000-0000-0000-000000000000',
            LastSeen VARCHAR(32)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

async fn seed_data(pool: &sqlx::AnyPool) {
    // Login handler strips $1$ prefix before hashing: md5(stripped + ":" + salt)
    let stripped = PASSWORD.strip_prefix("$1$").unwrap();
    let password_hash = format!(
        "{:x}",
        md5::compute(format!("{stripped}:{SALT}").as_bytes())
    );
    let loc_x: i64 = 256000;
    let loc_y: i64 = 256000;
    let region_handle: i64 = (loc_x << 32) | loc_y;

    sqlx::query(
        "INSERT INTO assets (id, name, description, assetType, local, temporary, data, CreatorID, asset_flags, create_time, access_time)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(ASSET_UUID).bind("Test Asset").bind("Conformance test asset")
    .bind(0i32).bind(0i32).bind(0i32).bind(b"Hello OpenSim!".to_vec())
    .bind(PRINCIPAL).bind(0i32).bind(1700000000i32).bind(1700000000i32)
    .execute(pool).await.unwrap();

    sqlx::query(
        "INSERT INTO inventoryfolders (folderID, agentID, parentFolderID, folderName, type, version)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(ROOT_FOLDER).bind(PRINCIPAL).bind(ZERO_UUID)
    .bind("My Inventory").bind(8i16).bind(1i32)
    .execute(pool).await.unwrap();

    sqlx::query(
        "INSERT INTO inventoryfolders (folderID, agentID, parentFolderID, folderName, type, version)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(CHILD_FOLDER).bind(PRINCIPAL).bind(ROOT_FOLDER)
    .bind("Textures").bind(0i16).bind(1i32)
    .execute(pool).await.unwrap();

    sqlx::query(
        "INSERT INTO inventoryitems (inventoryID, assetID, assetType, inventoryName, inventoryDescription,
         inventoryNextPermissions, inventoryCurrentPermissions, invType, creatorID,
         inventoryBasePermissions, inventoryEveryOnePermissions, salePrice, saleType,
         creationDate, groupID, groupOwned, flags, parentFolderID, avatarID, inventoryGroupPermissions)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
    )
    .bind(ITEM_UUID).bind(ASSET_UUID).bind(0i32).bind("Test Item").bind("Test item")
    .bind(2147483647i64).bind(2147483647i64).bind(0i32).bind(PRINCIPAL)
    .bind(2147483647i64).bind(0i64).bind(0i32).bind(0i32).bind(1700000000i32)
    .bind(ZERO_UUID).bind(0i32).bind(0i64).bind(ROOT_FOLDER).bind(PRINCIPAL).bind(0i64)
    .execute(pool).await.unwrap();

    sqlx::query(
        "INSERT INTO regions (uuid, regionHandle, regionName, regionSecret, serverIP,
         serverPort, serverURI, locX, locY, locZ, regionMapTexture, serverHttpPort,
         owner_uuid, access, ScopeID, sizeX, sizeY, flags, last_seen, PrincipalID,
         Token, parcelMapTexture)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)",
    )
    .bind(REGION_UUID).bind(region_handle).bind("TestRegion").bind("")
    .bind("127.0.0.1").bind(9000i32).bind("http://127.0.0.1:9000/")
    .bind(256000i32).bind(256000i32).bind(0i32).bind(ZERO_UUID).bind(9000i32)
    .bind(PRINCIPAL).bind(21i32).bind(ZERO_UUID).bind(256i32).bind(256i32)
    .bind(1i32).bind(0i32).bind(ZERO_UUID).bind("").bind(ZERO_UUID)
    .execute(pool).await.unwrap();

    sqlx::query(
        "INSERT INTO UserAccounts (PrincipalID, ScopeID, FirstName, LastName, Email,
         ServiceURLs, Created, UserLevel, UserFlags, UserTitle, active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(PRINCIPAL)
    .bind(ZERO_UUID)
    .bind("Test")
    .bind("User")
    .bind("test.user@example.com")
    .bind("")
    .bind(1700000000i32)
    .bind(0i32)
    .bind(0i32)
    .bind("")
    .bind(1i32)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO auth (UUID, passwordHash, passwordSalt, webLoginKey, accountType)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(PRINCIPAL)
    .bind(&password_hash)
    .bind(SALT)
    .bind("")
    .bind("UserAccount")
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO GridUser (UserID, HomeRegionID, HomePosition, HomeLookAt,
         LastRegionID, LastPosition, LastLookAt, Online, Login, Logout)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(PRINCIPAL)
    .bind(REGION_UUID)
    .bind("<128,128,25>")
    .bind("<0,1,0>")
    .bind(REGION_UUID)
    .bind("<128,128,25>")
    .bind("<0,1,0>")
    .bind("false")
    .bind("1700000000")
    .bind("0")
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO Friends (PrincipalID, Friend, Flags, Offered) VALUES ($1, $2, $3, $4)",
    )
    .bind(PRINCIPAL)
    .bind(FRIEND_UUID)
    .bind("1")
    .bind("0")
    .execute(pool)
    .await
    .unwrap();

    for (name, value) in [
        ("AvatarType", "1"),
        ("Height", "1.771488"),
        ("Serial", "1"),
        ("Wearables", "0"),
    ] {
        sqlx::query("INSERT INTO Avatars (PrincipalID, Name, Value) VALUES ($1, $2, $3)")
            .bind(PRINCIPAL)
            .bind(name)
            .bind(value)
            .execute(pool)
            .await
            .unwrap();
    }

    sqlx::query(
        "INSERT INTO MuteList (AgentID, MuteID, MuteName, MuteType, MuteFlags, Stamp)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(PRINCIPAL)
    .bind(FRIEND_UUID)
    .bind("Annoying Bob")
    .bind(1i32)
    .bind(0i32)
    .bind(1700000000i32)
    .execute(pool)
    .await
    .unwrap();
}

fn build_router(pool: &sqlx::AnyPool) -> axum::Router {
    let auth_store = Arc::new(AuthStore::new(pool.clone()));
    let inv_store = Arc::new(InventoryStore::new(pool.clone()));
    let grid_store = Arc::new(GridStore::new(pool.clone()));
    let presence_store = Arc::new(PresenceStore::new(pool.clone()));
    let griduser_store = Arc::new(GridUserStore::new(pool.clone()));
    let accounts_store = Arc::new(UserAccountStore::new(pool.clone()));
    let friends_store = Arc::new(FriendsStore::new(pool.clone()));
    let avatar_store = Arc::new(AvatarStore::new(pool.clone()));
    let mute_store = Arc::new(MuteStore::new(pool.clone()));

    axum::Router::new()
        .merge(welkin_services::assets::router(AssetServiceState::new(
            AssetStore::new(pool.clone()),
            DeleteAllowed::None,
        )))
        .merge(welkin_services::auth::router(AuthServiceState {
            store: auth_store.clone(),
        }))
        .merge(welkin_services::inventory::router(InventoryServiceState {
            store: inv_store.clone(),
        }))
        .merge(welkin_services::grid::router(GridServiceState {
            store: grid_store.clone(),
        }))
        .merge(welkin_services::presence::router(PresenceServiceState {
            store: presence_store.clone(),
        }))
        .merge(welkin_services::griduser::router(GridUserServiceState {
            store: griduser_store.clone(),
        }))
        .merge(welkin_services::accounts::router(AccountsServiceState {
            store: accounts_store.clone(),
            auth_store: auth_store.clone(),
            inventory_store: inv_store.clone(),
            allow_create_user: true,
        }))
        .merge(welkin_services::friends::router(FriendsServiceState {
            store: friends_store.clone(),
        }))
        .merge(welkin_services::avatar::router(AvatarServiceState {
            store: avatar_store.clone(),
        }))
        .merge(welkin_services::mute::router(MuteServiceState {
            store: mute_store.clone(),
        }))
        .merge(welkin_services::login::router(LoginServiceState {
            auth_store,
            accounts_store,
            inventory_store: inv_store,
            grid_store,
            presence_store,
            griduser_store,
            friends_store,
            throttle: std::sync::Arc::new(welkin_services::login_throttle::LoginThrottle::new(Default::default())),
        }))
}

async fn send(app: &axum::Router, method: &str, uri: &str, body: String) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

async fn send_xml_rpc(app: &axum::Router, uri: &str, body: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "text/xml")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

fn extract_xml_rpc_string(body: &str, name: &str) -> Option<String> {
    let search = format!("<name>{name}</name>");
    let idx = body.find(&search)?;
    let rest = &body[idx + search.len()..];
    let val_start = rest.find("<string>")? + 8;
    let val_end = rest.find("</string>")?;
    Some(rest[val_start..val_end].to_string())
}

fn extract_xml_rpc_i4(body: &str, name: &str) -> Option<i32> {
    let search = format!("<name>{name}</name>");
    let idx = body.find(&search)?;
    let rest = &body[idx + search.len()..];
    let val_start = rest.find("<i4>")? + 4;
    let val_end = rest.find("</i4>")?;
    rest[val_start..val_end].parse().ok()
}

// ===========================================================================
// Test 3.2: pCampBot login → rez → teleport-prep sequence
// ===========================================================================

#[tokio::test]
async fn pcambot_login_rez_teleport_sequence() {
    let pool = setup_db().await;
    seed_data(&pool).await;
    let app = build_router(&pool);

    // --- Step 1: XML-RPC login (as pCampBot would) ---
    let login_xml = format!(
        r#"<?xml version="1.0"?><methodCall><methodName>login_to_simulator</methodName>
        <params><param><value><struct>
        <member><name>first</name><value><string>Test</string></value></member>
        <member><name>last</name><value><string>User</string></value></member>
        <member><name>passwd</name><value><string>{PASSWORD}</string></value></member>
        <member><name>start</name><value><string>home</string></value></member>
        </struct></value></param></params></methodCall>"#
    );

    let (status, body) = send_xml_rpc(&app, "/", &login_xml).await;
    assert_eq!(status, StatusCode::OK, "login should return 200");
    assert!(
        body.contains("<name>login</name>"),
        "response should contain login field"
    );

    let login_val = extract_xml_rpc_string(&body, "login").unwrap();
    assert_eq!(login_val, "true", "login should succeed");

    let session_id =
        extract_xml_rpc_string(&body, "session_id").expect("response should contain session_id");
    let secure_session_id = extract_xml_rpc_string(&body, "secure_session_id")
        .expect("response should contain secure_session_id");
    let agent_id =
        extract_xml_rpc_string(&body, "agent_id").expect("response should contain agent_id");
    let _circuit_code =
        extract_xml_rpc_i4(&body, "circuit_code").expect("response should contain circuit_code");
    let seed_cap = extract_xml_rpc_string(&body, "seed_capability")
        .expect("response should contain seed_capability");

    assert_eq!(agent_id, PRINCIPAL, "agent_id should match principal");
    assert!(!session_id.is_empty(), "session_id should not be empty");
    assert!(
        !secure_session_id.is_empty(),
        "secure_session_id should not be empty"
    );
    assert!(
        seed_cap.contains("/CAPS/"),
        "seed_capability should contain /CAPS/ path"
    );

    // Verify login response contains inventory skeleton
    assert!(
        body.contains("<name>inventory-skeleton</name>"),
        "login response should include inventory-skeleton"
    );
    assert!(
        body.contains("<name>inventory-root</name>"),
        "login response should include inventory-root"
    );
    assert!(
        body.contains(ROOT_FOLDER),
        "inventory root folder ID should appear in response"
    );

    // Verify login response contains buddy list with our friend
    assert!(
        body.contains("<name>buddy-list</name>"),
        "login response should include buddy-list"
    );

    // Verify home field contains region coordinates
    assert!(
        body.contains("<name>home</name>"),
        "response should contain home"
    );
    assert!(
        body.contains("region_handle"),
        "home should contain region_handle"
    );

    // --- Step 2: Presence report (simulator reports agent location) ---
    let (status, body) = send(
        &app,
        "POST",
        "/presence",
        format!("METHOD=report&SessionID={session_id}&RegionID={REGION_UUID}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("<result>Success</result>"),
        "presence report should succeed: {body}"
    );

    // --- Step 3: Verify presence was recorded ---
    let (status, body) = send(
        &app,
        "POST",
        "/presence",
        format!("METHOD=getagent&SessionID={session_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(REGION_UUID),
        "getagent should return the region we reported to: {body}"
    );

    // --- Step 4: GridUser should be marked online (login handler called logged_in) ---
    let (status, body) = send(
        &app,
        "POST",
        "/griduser",
        format!("METHOD=getgriduserinfo&UserID={PRINCIPAL}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("<Online>True</Online>"),
        "griduser should show online after login: {body}"
    );

    // --- Step 5: Post-login inventory reads (viewer fetches root folder) ---
    let (status, body) = send(
        &app,
        "POST",
        "/xinventory",
        format!("METHOD=GETROOTFOLDER&PRINCIPAL={PRINCIPAL}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(ROOT_FOLDER),
        "root folder fetch should return our root: {body}"
    );
    assert!(
        body.contains("My Inventory"),
        "root folder should be named 'My Inventory': {body}"
    );

    // --- Step 6: Viewer fetches folder content (textures subfolder items) ---
    let (status, body) = send(
        &app,
        "POST",
        "/xinventory",
        format!("METHOD=GETFOLDERCONTENT&PRINCIPAL={PRINCIPAL}&FOLDER={ROOT_FOLDER}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(CHILD_FOLDER),
        "folder content should contain Textures subfolder: {body}"
    );
    assert!(
        body.contains(ITEM_UUID),
        "folder content should contain our test item: {body}"
    );

    // --- Step 7: Teleport prep - look up region by name ---
    let (status, body) = send(
        &app,
        "POST",
        "/grid",
        "METHOD=get_region_by_name&NAME=TestRegion".to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(REGION_UUID),
        "region lookup should find TestRegion: {body}"
    );
    assert!(
        body.contains("127.0.0.1"),
        "region should have correct server IP: {body}"
    );

    // --- Step 8: Teleport prep - get default regions ---
    let (status, body) = send(
        &app,
        "POST",
        "/grid",
        format!("METHOD=get_default_regions&SCOPEID={ZERO_UUID}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(REGION_UUID),
        "default regions should include our region: {body}"
    );
}

// ===========================================================================
// Test 3.3: Aurora Viewer login sequence
// ===========================================================================

#[tokio::test]
async fn aurora_viewer_login_sequence() {
    let pool = setup_db().await;
    seed_data(&pool).await;
    let app = build_router(&pool);

    // Aurora Viewer sends the same XML-RPC login but with start=last
    let login_xml = format!(
        r#"<?xml version="1.0"?><methodCall><methodName>login_to_simulator</methodName>
        <params><param><value><struct>
        <member><name>first</name><value><string>Test</string></value></member>
        <member><name>last</name><value><string>User</string></value></member>
        <member><name>passwd</name><value><string>{PASSWORD}</string></value></member>
        <member><name>start</name><value><string>last</string></value></member>
        </struct></value></param></params></methodCall>"#
    );

    let (status, body) = send_xml_rpc(&app, "/", &login_xml).await;
    assert_eq!(status, StatusCode::OK);
    let login_val = extract_xml_rpc_string(&body, "login").unwrap();
    assert_eq!(login_val, "true", "Aurora login should succeed");

    // Verify we got the expected region (last location = TestRegion)
    let region_x = extract_xml_rpc_i4(&body, "region_x").unwrap();
    let region_y = extract_xml_rpc_i4(&body, "region_y").unwrap();
    assert_eq!(region_x, 256000, "region_x should match TestRegion");
    assert_eq!(region_y, 256000, "region_y should match TestRegion");

    let session_id = extract_xml_rpc_string(&body, "session_id").unwrap();
    let first_name = extract_xml_rpc_string(&body, "first_name").unwrap();
    let last_name = extract_xml_rpc_string(&body, "last_name").unwrap();
    assert_eq!(first_name, "Test");
    assert_eq!(last_name, "User");

    // Verify start_location reflects "last"
    let start_loc = extract_xml_rpc_string(&body, "start_location").unwrap();
    assert_eq!(start_loc, "last", "start_location should be 'last'");

    // Verify classified_categories are present (viewers need these)
    assert!(
        body.contains("<name>classified_categories</name>"),
        "response should contain classified_categories for viewer UI"
    );

    // Verify global-textures (viewer needs sun/cloud/moon textures)
    assert!(
        body.contains("<name>global-textures</name>"),
        "response should contain global-textures"
    );

    // Verify login-flags
    assert!(
        body.contains("<name>login-flags</name>"),
        "response should contain login-flags"
    );

    // Aurora-specific: verify the sim connection info is present
    let sim_ip = extract_xml_rpc_string(&body, "sim_ip").unwrap();
    let sim_port = extract_xml_rpc_i4(&body, "sim_port").unwrap();
    assert_eq!(sim_ip, "127.0.0.1");
    assert_eq!(sim_port, 9000);

    // Post-login: verify presence is set up correctly
    let (status, body) = send(
        &app,
        "POST",
        "/presence",
        format!("METHOD=getagent&SessionID={session_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains(PRINCIPAL),
        "presence should show our user after login: {body}"
    );

    // Verify griduser shows online
    let (status, body) = send(
        &app,
        "POST",
        "/griduser",
        format!("METHOD=getgriduserinfo&UserID={PRINCIPAL}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("<Online>True</Online>"),
        "user should be online after Aurora login: {body}"
    );
}

// ===========================================================================
// Test: login with wrong password should fail cleanly
// ===========================================================================

#[tokio::test]
async fn login_wrong_password_fails() {
    let pool = setup_db().await;
    seed_data(&pool).await;
    let app = build_router(&pool);

    let login_xml = r#"<?xml version="1.0"?><methodCall><methodName>login_to_simulator</methodName>
        <params><param><value><struct>
        <member><name>first</name><value><string>Test</string></value></member>
        <member><name>last</name><value><string>User</string></value></member>
        <member><name>passwd</name><value><string>$1$wronghashwronghashwronghashwro</string></value></member>
        <member><name>start</name><value><string>home</string></value></member>
        </struct></value></param></params></methodCall>"#;

    let (status, body) = send_xml_rpc(&app, "/", login_xml).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "even failed login returns 200 per XML-RPC"
    );
    let login_val = extract_xml_rpc_string(&body, "login").unwrap();
    assert_eq!(login_val, "false", "wrong password should fail");

    // Error message should not reveal whether the account exists
    assert!(
        !body.contains("not found"),
        "error should not reveal account existence"
    );
    assert!(
        !body.contains(PRINCIPAL),
        "error should not leak principal ID"
    );
}

// ===========================================================================
// Test: login with URI start location
// ===========================================================================

#[tokio::test]
async fn login_uri_start_location() {
    let pool = setup_db().await;
    seed_data(&pool).await;
    let app = build_router(&pool);

    let login_xml = format!(
        r#"<?xml version="1.0"?><methodCall><methodName>login_to_simulator</methodName>
        <params><param><value><struct>
        <member><name>first</name><value><string>Test</string></value></member>
        <member><name>last</name><value><string>User</string></value></member>
        <member><name>passwd</name><value><string>{PASSWORD}</string></value></member>
        <member><name>start</name><value><string>uri:TestRegion&amp;128&amp;128&amp;25</string></value></member>
        </struct></value></param></params></methodCall>"#
    );

    let (status, body) = send_xml_rpc(&app, "/", &login_xml).await;
    assert_eq!(status, StatusCode::OK);
    let login_val = extract_xml_rpc_string(&body, "login").unwrap();
    assert_eq!(login_val, "true", "URI-based login should succeed");

    let start_loc = extract_xml_rpc_string(&body, "start_location").unwrap();
    assert_eq!(
        start_loc, "url",
        "start_location should be 'url' for URI login"
    );
}
