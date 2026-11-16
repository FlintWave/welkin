<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Regions, estate and users

[← Back to docs](index.md) · [Configuration reference](configuration.md) · [Quick start](quick-start.md)

This page covers the day-to-day tasks of running a region: naming it, setting an
estate owner, creating accounts and using the region console.

## The region

A region is set up in `welkin.toml`:

```toml
[Region]
RegionName = "Test Region"
RegionLocX = 1000
RegionLocY = 1000

[Network]
ExternalHostName = "127.0.0.1"
InternalPort = 9000
```

`ExternalHostName` is the address viewers connect to, so set it to your public
name or IP when others will log in. `InternalPort` is the UDP port for viewer
traffic. The [configuration reference](configuration.md) lists every key.

### Region UUID

The region saves everything under its UUID. If you do not set `[Region]
RegionUUID`, the region generates one on its first start and stores it in the
database under the region's name and location. If you later rename the region or
move it to another location without setting `RegionUUID` to the old value, the
region gets a new UUID and starts empty. Set `RegionUUID` yourself if you need a
fixed value, for example when you move the region to a new database.

## The estate

The estate owner and managers can terraform and use the estate tools. Set them
by avatar UUID:

```toml
[Estate]
EstateOwner = "00000000-0000-0000-0000-000000000000"
EstateManagers = "uuid, uuid"
```

Without an `EstateOwner` nobody has estate rights, though avatars can still
edit, move and delete objects they own. The region logs a warning at start-up
when no owner is set. The region stores its estate in its database. It writes the
configured owner and managers there on the first start, and a changed
`EstateOwner` in the config replaces the stored one.

Permission checks are on by default. `[Permissions]
serverside_object_permissions = false` turns them off, which is meant for
development only: with them off, any avatar can edit or delete any object.

To find an avatar's UUID, the `create-user` command prints it when it creates
the account.

## Users

There is no web portal yet, so you create accounts from the region console. Start
`welkin-robust`, then `welkin-region`, and at the `welkin>` prompt run:

```text
create-user <first> <last>
```

The console asks for the password twice without showing it, then for an optional
email address. The first and last name must be non-empty and contain no
whitespace, and the password must be at least 8 characters and must not contain
a double quote. Names are compared without regard to case, so `Alice Smith` and
`alice smith` are the same account. On success the console prints `created user
First Last (<uuid>)`.

The grid refuses account creation unless `welkin-robust` has `AllowCreateUser =
true` under `[UserAccountService]`. It defaults to false. The `robust.toml` in
the repository turns it on for development. On a grid that others can reach,
turn it on only while you create accounts, because anyone who can reach the
private service port (8003, bound to `127.0.0.1` by default) can create accounts
while it is on.

## The region console

The `welkin>` prompt accepts:

| Command | What it does |
|---|---|
| `help` | List the commands |
| `version`, `uptime`, `region`, `stats` | Show server and region information |
| `users`, `user <name>` | List connected users, or show one |
| `backup` | Save changed objects to the database now |
| `alert <message>` | Broadcast an alert to every avatar |
| `kick <name> [reason]` | Disconnect a user |
| `create-user <first> <last>` | Create a grid account |
| `load oar <file>` | Load an OpenSimulator region archive (see below) |
| `shutdown`, `quit` | Shut the region down, or close the console |

The same commands are available over a small REST API (`POST /admin/console`).
It is off until you set `[Admin] Token`, uses plain HTTP, and refuses to bind a
non-loopback address unless `[Admin] AllowRemoteBind = true`. See the
[configuration reference](configuration.md#admin-api) before you turn it on.

## Loading an OAR

`load oar <file>` loads an OpenSimulator region archive into the running region.
It stores the archive's assets in the asset service, adds its objects to the
scene, and applies its terrain if the terrain is a 256 by 256 `.r32` file. Other
terrain formats are skipped with a warning. Progress goes to the log, and the
command prints a short note and returns while the load runs. Assets that already
exist are left alone, and a failed asset or object is counted, not fatal. There is
no console command for saving an OAR, and none for IAR files.

## Groups

Groups are off by default. Set `[Groups] Enabled = true` to turn them on. See the
[configuration reference](configuration.md#groups) and the
[viewer compatibility page](viewer-compatibility.md#groups) for what works.
