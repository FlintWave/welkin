## Why

Running a grid today means editing INI files, choosing map coordinates by
hand and loading content from a console. The goal is one command to
install and a browser from there on.

## What Changes

- **Installer.** One command on Linux that installs the container images
  and a service definition (Podman quadlets or a compose file), creates the
  database, opens the portal and prints its address. Upgrades and removal
  never touch world data. A non-container install from a release archive is
  the second path.
- **Portal**, served by the `robust` binary (`axum`), with GridGate's page
  set as the model (BSD; see the roadmap):
  - Home: what is running, who is in-world, sign-ups waiting, power
    controls.
  - Map: islands placed by dragging onto a grid map, with size, name,
    estate and physics engine per island; neighbours and free coordinates
    shown; on a public grid the map shows that grid's occupied coordinates.
  - People: accounts, staff roles, sign-up with approval and bot
    protection, password reset, two-factor sign-in for staff.
  - Estates, groups, hypergrid links, backups, logs and metrics.
- **First-run wizard**, a part of the portal. The portal opens on it
  automatically the first time the server is launched, and the installer
  and the desktop companion both open the browser there. Steps, in order:
  1. Mode: standalone, own grid, or join a grid (OSgrid or custom).
  2. Owner account and world name.
  3. Network check: external address, each region's TCP and UDP port
     reachable from outside, NAT detected and explained.
  4. Capacity assessment (see `capacity-assessor`), with a recommended
     number of islands and users.
  5. Place islands on the map.
  6. Choose content for each island: empty, flat or generated terrain, or
     an imported region archive.
  7. Start, then show the address to enter in Aurora Viewer.
- **Region archive import.** OAR files from disk or from a catalogue, with
  the loader's existing options (merge, displacement, rotation, bounding
  box, skip assets, default owner) set by dragging the archive's footprint
  on the map. The catalogue is a list of download addresses with a licence
  recorded for each entry; no content is bundled, an entry without a
  verified licence is not listed, and the licence is shown before import.
- **Desktop companion** (`companion` binary, optional, never needed on a
  headless server). It talks to the same control interface as the portal,
  on this machine or a remote one, and provides:
  - a tray icon with a status colour and a menu: islands running, users
    online, CPU and memory in use, any warning from the capacity assessor;
    "Open portal", "Open control window", start and stop the world, show
    logs;
  - a small control window (the "simple GUI") with the same figures and
    buttons, which also gives the application a dock or taskbar icon;
  - desktop notifications for a stopped island or a limit being reached.

  It is a Tauri 2 application (operator decision). Read from crates.io on
  2026-11-13: `tauri` 2.12.5 (Apache-2.0 or MIT) with its `tray-icon`
  feature, which covers Windows, macOS, and Linux through AppIndicator or
  StatusNotifierItem. The control window shows pages from the portal's own
  front end, so there is one set of pages to maintain. Status is polled
  every few seconds; nothing in the companion needs real-time updates, and
  anything that does stays in the portal or the viewer. On Linux Tauri
  renders with the system's WebKitGTK, which the installer must check for.
  GNOME shows tray icons only with an extension installed, so on Linux the
  control window and its launcher entry are the guaranteed path and the
  tray icon is shown where the desktop supports it.

  Nothing reusable was found. DreamGrid (Windows) and GridGate each have a
  launcher, but no OpenSimulator project with a tray icon turned up, and
  the Python tools found (`garyanthonyround-collab/opensim-tools`, the
  `Web_3d` launcher) are administration scripts, not launchers. No Python is used.
- **Region control interface**: an authenticated API the portal uses to
  create, start, stop, snapshot and remove regions.

## Impact

- Depends on `grid-services`; island control depends on
  `region-simulator-core`; archive import on `archives`. Until the Rust
  simulator exists the portal can drive C# regions through the same
  interface, so it is usable from phase 2.
- w4os (WordPress, AGPL-3.0) stays a supported alternative front for sign-up
  and search.
- Catalogue sources to audit, none yet verified: Linda Kellie's archives
  (described as public-domain equivalent in a 2013 article, since moved to
  another site), OpenSim Creations, OpenSim Worlds, and the University of
  Edinburgh's OpenVCE assets.
- Risk: HIGH for the portal's sign-in, account and region-control paths,
  which give control of the whole grid. Second review required.
