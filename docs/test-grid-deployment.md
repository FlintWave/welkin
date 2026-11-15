<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Test grid deployment

[<- Back to docs](README.md) | [Architecture](architecture.md) | [Operator guide](operator-guide.md) | [Migration guide](migration-guide.md)

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
| 8003 | TCP | Robust grid services (HTTP) |
| 9000 | TCP | Region HTTP (capabilities, health) |
| 9000 | UDP | Region LLUDP (viewer traffic) |

If you already have a C# OpenSimulator listening on any of these ports, stop
it first or remap the Rust ports in `compose.yaml`.

## Quick start

```sh
# Clone and build
cd welkin
podman-compose up -d          # or: docker compose up -d

# Watch startup
podman-compose logs -f robust region
```

The first build compiles static musl-linked binaries inside the container
(multi-stage `Containerfile`). Subsequent starts reuse cached layers unless
`Cargo.toml` or `Cargo.lock` changes.

Once `robust` logs `listening on 0.0.0.0:8003` and `region` logs
`region ready`, the grid is up.

## Create an account

Use the portal's first-run wizard to create the initial admin account:

1. Open `http://localhost:9000/portal` in a browser.
2. Follow the wizard: grid name, admin credentials, first region name and
   coordinates.
3. The wizard creates the estate, the default region, and an admin user with
   the password you choose.

If the portal is not reachable, create the account by connecting `osim-region`
to the console (see the [operator guide](operator-guide.md)).

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
   - **Login URI:** `http://<host>:8003`
   - **Grid name:** whatever you chose in the wizard
3. Log in with the credentials from the wizard.

The viewer should connect, rez your avatar, and display the region terrain.

### Other viewers

Second Life-compatible viewers (Firestorm, Alchemy, etc.) should work for
basic operations since the LLUDP and capabilities protocols are
wire-compatible. WebRTC voice requires Aurora Viewer.

## Verifying the deployment

Work through this checklist after first login:

### Basic operations

- [ ] Avatar moves, rotates, sits, stands
- [ ] Right-click build: create a prim, resize, texture, move, copy, delete
- [ ] Chat (Say, Shout, Whisper) between two avatars or with an NPC
- [ ] Inventory: create folder, upload texture, drag item to/from prim
- [ ] Terrain: raise, lower, smooth, flatten with the edit tools

### Scripting

- [ ] Create a prim, add a script with `default { state_entry() { llSay(0, "ALIVE"); } touch_start(integer n) { llSay(0, "touched"); } }`
- [ ] "ALIVE" appears in chat when the script loads (state_entry fires llSay)
- [ ] Touch the prim; "touched" appears in chat (touch_start fires llSay)
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

If the grid has a public IP and ports 8003/9000 are reachable:

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
