<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# Test grid deployment

[<- Back to docs](index.md) | [Architecture](architecture.md) | [Operator guide](operator-guide.md) | [Migration guide](migration-guide.md)

Deploy a self-contained test grid to verify that the Rust port works end to
end: login, object creation, scripting, voice, and hypergrid. Everything runs
in containers; nothing touches your host beyond the mapped ports.

## Prerequisites

| Requirement | Minimum |
|---|---|
| Docker or Podman (with docker-compose / podman-compose) | Docker 24+ or Podman 4+ |
| RAM | 4 GB free |
| Disk | 2 GB for images and database |
| CPU | 2 cores |

Ports used by the test grid:

| Port | Protocol | Service |
|---|---|---|
| 8002 | TCP | ROBUST public (login, grid info, map) |
| 8003 | TCP | ROBUST private (grid services) |
| 9000 | UDP | Region LLUDP (viewer traffic) |
| 9001 | TCP | Region HTTP (capabilities, health) |

If you already have a C# OpenSimulator listening on any of these ports, stop
it first or remap the Welkin ports in your configuration.

## Quick start (native build)

Build the binaries and run them directly:

```sh
cd welkin
cargo build --release

# Start ROBUST (grid services)
./target/release/welkin-robust &

# Start the region
./target/release/welkin-region &
```

ROBUST listens on two ports: 8002 (public, handles login, grid info, and map
requests) and 8003 (private, handles inter-service calls). The region listens
on 9000 UDP (LLUDP viewer traffic) and 9001 TCP (capabilities HTTP). Once
ROBUST logs `Public server listening` on 8002 and the region logs
`region ready`, the grid is up.

## Quick start (containers)

```sh
cd welkin
podman-compose up -d          # or: docker compose up -d

# Watch startup
podman-compose logs -f robust region
```

The first build compiles static musl-linked binaries inside the container
(multi-stage `Containerfile`). Subsequent starts reuse cached layers unless
`Cargo.toml` or `Cargo.lock` changes.

## Create an account

Create the first account from the region console. Start `welkin-robust`, then
`welkin-region`, and at the `welkin>` prompt run:

```text
create-user <first> <last>
```

For example, `create-user Alice Smith`. The console then asks for the password
twice (`Password:` and `Confirm password:`) and does not echo what you type.
It then asks `Email (optional, Enter to skip):`. The email is not a secret, so
it is echoed and not kept in history.
The console refuses a password typed on the command line, such as
`create-user Alice Smith secret123`, and tells you to use the prompt instead.

- The first and last name must be non-empty and contain no whitespace.
- The password must be at least 8 characters and must not contain a double
  quote (`"`).
- The email is optional and must not contain spaces or double quotes.
- The grid rejects a name or ID that already exists. Names are compared without
  regard to case or Unicode form, so `Alice Smith` and `alice smith` are the
  same account, and two simultaneous requests cannot both create it.

The grid refuses the request unless `welkin-robust` has `AllowCreateUser = true`
under `[UserAccountService]` in `robust.toml`. It defaults to false. The
development `robust.toml` in the repository turns it on; for a grid you build
yourself, add it while you create accounts and remove it afterwards. See the
[configuration reference](configuration.md#welkin-robust-grid-services).

The command sends the request to the grid service at `GridServerURI` in the
`[GridService]` section of `welkin.toml`, so ROBUST must be running and
reachable from the region. On success the console prints
`created user First Last (<uuid>)`. Failures start with `error:`.

The console does not save `create-user` lines in its command history, and the
password is not written to the logs. The admin REST API runs the same command
with the password inline in the request body, and the console's refusal does
not apply there; see the [admin API note](configuration.md#admin-api) before
using it.

The web portal and its first-run wizard are a scaffold and do not create
accounts.

## Importing OAR content

Load a pre-existing OAR archive into the test region:

```sh
# Copy the OAR into the region container
podman cp MyRegion.oar region:/home/opensim/

# Run the OAR load (via the region binary's console command interface)
podman exec region welkin-region load-oar /home/opensim/MyRegion.oar
```

Multi-region OARs are supported. The loader creates missing regions
automatically using the coordinates stored in the archive.

> [!NOTE]
> OAR archives from C# OpenSimulator 0.9.x load without modification. The
> XML2 serialisation format is wire-compatible. See the
> [archives summary](archives-summary.md) for details.

## Connecting Aurora Viewer

1. Download [Aurora Viewer](https://github.com/Aurora-Viewer/Aurora-Viewer)
   (pinned commit `3d523b14` for tested compatibility).
2. Add a grid entry:
   - **Login URI:** `http://<host>:8002`
   - **Grid name:** any name you like
3. Log in with an account that exists in the grid database.

The viewer should connect, rez your avatar, and display the region terrain.

### Other viewers

Second Life-compatible viewers (Firestorm, Alchemy, etc.) should work for
basic operations since the LLUDP and capabilities protocols are
wire-compatible. WebRTC voice requires Aurora Viewer.

## Verifying the deployment

Work through this checklist after first login:

### Basic operations

- [x] Avatar moves, rotates, sits, stands (Aurora verified, Firestorm code-verified)
- [ ] Right-click build: create a prim, resize, texture, move, copy, delete
- [x] Chat (Say, Shout, Whisper) between two avatars or with an NPC (bot-verified, ChatFromViewer routing fixed in 9d82a90)
- [ ] Inventory: create folder, upload texture, drag item to/from prim
- [ ] Terrain: raise, lower, smooth, flatten with the edit tools
- [x] Map and minimap render region data (MapBlockRequest fixed in 9d82a90)

### Scripting

- [x] Create a prim, upload a script via two-step LLSD protocol (UpdateScriptAgent cap)
- [x] "ALIVE" appears in chat when the script loads (state_entry fires llSay, bot-verified)
- [x] Touch the prim; "touched" appears in chat (touch_start fires llSay, bot-verified)
- [ ] Script state survives region restart (checkpoint/restore)

### Voice

Voice requires the wolfvoice sidecar. Add it to the compose stack:

```yaml
  wolfvoice:
    image: wolfvoice:latest
    ports:
      - "7880:7880"
    environment:
      TURN_SERVER: "stun:stun.l.google.com:19302"
```

Then set the region environment variable:

```
OPENSIM__Voice__WolfvoiceURI=http://wolfvoice:7880
```

- [ ] Voice icon appears in Aurora Viewer
- [ ] Spatial audio works between two avatars in the same parcel

### Hypergrid

If the grid has a public IP and ports 8002 and 9000 are reachable:

- [ ] Enable hypergrid in the profile (`[Hypergrid] Enabled = true`)
- [ ] Teleport to a remote grid via `hop://grid.example.com:8002/RegionName`
- [ ] A remote avatar can teleport in

> [!WARNING]
> Hypergrid exposes your grid to untrusted remote grids. The security review
> carries a CONDITIONAL PASS with open findings. Do not enable hypergrid on a
> production grid until the tracked fixes are resolved. See
> [security-review-hypergrid.md](security-review-hypergrid.md).

## Monitoring

Both `welkin-robust` and `welkin-region` expose Prometheus-compatible
endpoints:

| Endpoint | URL |
|---|---|
| Health check | `http://localhost:9000/health` |
| Prometheus metrics | `http://localhost:9000/metrics` |
| Robust health | `http://localhost:8003/health` |
| Robust metrics | `http://localhost:8003/metrics` |

Key metrics to watch:

- `opensim_region_agents_total` -- currently connected avatars
- `opensim_region_prims_total` -- objects in the scene
- `opensim_physics_step_duration_seconds` -- physics tick time (should stay under 11ms for 45 FPS)
- `opensim_lludp_packets_sent_total` / `opensim_lludp_packets_received_total`
- `opensim_script_events_processed_total` -- script throughput

Scrape both `:8003/metrics` and `:9000/metrics` from your Prometheus
instance. The health endpoints return `ok` with HTTP 200 when the service is
running.

## Stopping and resetting

```sh
# Stop the grid (data persists in the db-data volume)
podman-compose down

# Stop and delete all data (fresh start)
podman-compose down -v
```

## Known limitations

- **osim-robust is a skeleton binary.** The grid services are implemented in
  the `welkin-services` crate and the axum routers are functional, but the
  `main.rs` wiring is minimal. Production hardening is a cutover task.
- **All security reviews carry a CONDITIONAL PASS.** The tracked findings
  must be resolved before any deployment that faces untrusted users or grids.
- **Pathfinding is not implemented.** Decision R9 deferred it.
- **LSL coverage is partial.** The LSL-to-Luau compiler handles the core
  language (types, control flow, states, events, casts) and 68% of real-world
  scripts parse and compile. 206 ll\* builtins and 231 os\* functions are
  registered and functional. Remaining SL-spec ll\* functions are not yet
  bound.
- **osim-conform live attach is deferred.** Conformance replay from recorded
  fixtures works; live-attach to a running C# server is a Phase 7 task.
- **Single architecture.** Container images target `x86_64-unknown-linux-musl`
  only. ARM64 builds are not yet provided.

## Reporting issues

The repository is local-only (decision D2). To report an issue:

1. Capture the output of `podman-compose logs robust region`.
2. Note the failing step from the verification checklist above.
3. If the issue is a protocol divergence, run `osim-conform` replay against
   the failing service and include the diff.
4. File the report in the project's issue tracker or raise it in the
   development session.

See also: [operator guide](operator-guide.md) for production deployment,
[migration guide](migration-guide.md) for migrating from C# OpenSimulator.
