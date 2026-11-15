# Grid portal summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#g1) · Previous: [Capacity assessor](capacity-assessor-summary.md) · Next: [Region simulator core](region-simulator-core-summary.md)

The grid-portal change is accepted (`8b685e0`). All 26 tasks are done. It adds
a browser portal, an installer, a first-run wizard and an optional desktop
companion, so a grid can be installed with one command and run from a browser.

## What was built

The first feature commit (`ddba6d0`) added 2,974 lines in 21 files: the
`welkin-portal` crate (2,084 lines of Rust over 12 source files) and the
installer (890 lines of shell, systemd, container and compose files). The
desktop companion and the end-to-end test followed in `e8ddfd8`, and the fixes
for the security review in `59eea64`. `c2706d1` marked 26 of 26 tasks complete.

| Commit | Content |
|---|---|
| `ddba6d0` | `welkin-portal` crate and installer (sections 1 to 6) |
| `63cf86a` | Task list at 20 of 26 |
| `59eea64` | Security fixes H1 to H3 |
| `e8ddfd8` | Desktop companion, end-to-end test, security review (sections 7 and 8) |
| `c2706d1` | Task list at 26 of 26 |
| `8b685e0` | Change marked accepted; Phase 2 complete |

The portal follows GridGate's page set (BSD) as its model. The proposal says
the portal is served by the `robust` binary; the crate exposes axum routers
that `welkin-robust` mounts, and `welkin-robust` is still a skeleton.

## 1. Installer

Files are in `install/`:

- `install.sh`: detects the distribution, writes the configuration and the
  systemd unit, and prepares the first admin token. Container images and
  database creation follow the proposal.
- `upgrade.sh` and `uninstall.sh`: both leave world data in place.
- `install-archive.sh`: the second path, from a release archive with no
  containers.
- `welkin-robust.service`: the systemd unit.
- `container/Containerfile`: multi-stage image build.
- `container/compose.yaml`: database, `robust` and a region.

## 2. Portal base

- Sign-in uses argon2 password hashes (decision G2). Sessions are random
  tokens held in a session store and accepted as a bearer token or the
  `portal_session` cookie (`auth.rs`, `sessions.rs`).
- Staff roles are Admin, Operator and Viewer (`roles.rs`). Admin manages
  accounts and regions, Operator manages regions, and every role can view logs.
- The audit log (`audit.rs`) records logins, account changes and region
  operations. It is in memory only (see M4 below).
- Region control (`region_control.rs`) is an authenticated create, start, stop,
  snapshot and remove interface, plus the client the portal uses against it.
- Two-factor sign-in is a stub that rejects any account with a TOTP secret
  (fix H1). Real TOTP is not implemented.

## 3. Grid map

`map.rs` places islands with variable sizes. A placement is checked against
every occupied rectangle with an overlap test and refused on conflict.
`find_free_coords` searches outward in square rings around the requested
coordinate in 256-unit steps, up to 99 rings, skipping negative coordinates. If
nothing is free it falls back to 100 steps east. An overlay shows the occupied
coordinates of an external grid.

## 4. People

`people.rs` holds sign-up with approval, account search, suspension and
password reset. The sign-up password is hashed with argon2 at registration and
never kept as plaintext (fix H3). All staff account handlers call
`require_admin()` (fix H2). The `captcha_token` field is accepted but ignored
(M3).

## 5. First-run wizard

`wizard.rs` runs seven steps in order: mode, owner, network, capacity,
placement, content, finish. The state is `NotStarted`, `InProgress { step }` or
`Completed`; the portal opens on the wizard until it is completed. Modes are
standalone, own grid and join a grid. The capacity step is meant to call the
`welkin-capacity` estimate. The network check carries a result structure; the
outside probe depends on a reachable external service.

## 6. Content

`content.rs` provides:

- A catalogue where each entry records a licence (`LicenceInfo`). No content
  is bundled.
- OAR import with the loader options (`ImportOptions`) and a status record
  (`ImportStatus`). The actual load depends on `welkin-archive`, which is
  still a skeleton.
- Terrain presets: flat, island, archipelago, mountain and valley.
- Backup scheduling per region: interval in hours and a retention count.

## 7. Desktop companion

`crates/welkin-companion` is a Tauri 2 application (decision G1). It has a
tray icon with a status menu (`tray.rs`), status polling, service start and
stop through `systemctl`, a log viewer through `journalctl`, desktop
notifications and a remote grid connect (`commands.rs`). It ships a `.desktop`
launcher and an SVG icon. It is excluded from the Cargo workspace because it
needs the Tauri toolchain and, on Linux, WebKitGTK. On GNOME the tray icon
needs an extension; the control window is the guaranteed path.

## 8. Proof

`crates/welkin-portal/tests/e2e_portal_flow.rs` is one test with 14 stages
covering the wizard, authentication, region lifecycle, map, people, content
and access control. The module tests plus the end-to-end test passed with the
full workspace green.

An Opus security review found 3 HIGH, 5 MEDIUM, 9 LOW and 4 informational
items. The 3 HIGH items were fixed in `59eea64`:

- H1: the TOTP check returned true for everyone. It now returns false, which
  locks out accounts that have a TOTP secret until real TOTP exists.
- H2: people and account handlers lacked role checks. They now require Admin.
- H3: the sign-up password was held in plaintext. It is hashed at sign-up.

Not yet done: task 8.1 asked for a fresh machine to reach a walkable island in
Aurora Viewer. No Rust region simulator exists, so the end-to-end test drives
the portal's API, not a viewer. Treat 8.1 as proven for the portal path only.

## Security notes for deployment

The review's assessment is that the portal is safe for local development and
not ready for public exposure until the MEDIUM items are handled. Until then,
bind it to a private network or put it behind a reverse proxy.

| ID | Issue | Mitigation until fixed |
|---|---|---|
| M1 | No login rate limiting | Rate-limit at the proxy: per IP and per account |
| M2 | Session cookie is never set with `HttpOnly`, `Secure` or `SameSite`, so the cookie path is open to CSRF | Use bearer tokens only, or set the flags at the proxy |
| M3 | Sign-up bot protection is a stub | Rate-limit sign-up at the proxy, or keep sign-up closed |
| M4 | Audit log is memory-only and lost on restart | Forward logs elsewhere; persistence to SQLite is pending |
| M5 | No request body size limit | Limit bodies to about 1 MiB at the proxy |

The 9 LOW items include error text that may expose internal detail
(`PortalError::Internal`). The full report is `security_review_portal.md` in
the session scratchpad.

## Known gaps

- `welkin-robust` does not yet mount the portal.
- TOTP, captcha, persistent audit storage and rate limiting are stubs or absent.
- Region control talks to C# regions until the Rust simulator exists.
