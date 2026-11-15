## Why

The port must contain nothing proprietary and nothing whose licence is
unknown. Most of the C# tree is BSD, but several parts originate with Linden
Lab or depend on closed services, and the choice of project licence decides
whether code can be shared with Aurora Viewer.

## What Changes

A written audit, kept in the repository, with a disposition for every item.
Findings so far from this tree:

| Item | Finding | Disposition |
|---|---|---|
| OpenSimulator source | BSD-3-Clause | Port; keep the notice |
| ODE, Bullet | BSD, zlib | Open, but replaced (see `physics-backend`) |
| Havok | Proprietary; used by Second Life, absent here | Nothing to remove |
| `Voice/VivoxVoice` | Client for a proprietary service | Drop |
| `Voice/FreeSwitchVoice`, `FreeswitchService` | Open, legacy; Aurora is WebRTC only | Drop; see `webrtc-voice` |
| `MySql.Data.dll` | Oracle connector, GPL with exception | Replace with `sqlx` |
| `OpenMetaverse*.dll` | libopenmetaverse, BSD | Replace with Aurora crates plus new code |
| Aurora crates | GPL-3.0-or-later, unpublished | Pinned git dependency |
| Protocol message template | Linden Lab, LGPL 2.1 via the viewer | Confirm header |
| `bin/openmetaverse_data` (`avatar_lad.xml`, skeleton, textures) | Linden Lab origin | Confirm licence or replace |
| `bin/assets` default library | Mixed origin | Audit each asset set |
| CSJ2K, OpenJPEG | BSD | Choose a Rust JPEG 2000 decoder and encoder |
| Warp3D (map tiles) | To check | Reimplement |
| "Second Life", "LSL" names | Trademarks | Naming rules for docs and binaries |

Two decisions are recorded here:

- **D1, project licence: decided 2026-11-13, GPL-3.0-or-later**, so that
  Aurora Viewer's crates can be reused. Consequences:
  - Ported OpenSimulator code keeps its BSD-3-Clause notice inside the GPL
    project.
  - Every dependency must be GPLv3-compatible. Apache-2.0 (Rapier) is.
  - Aurora's `NOTICE.md` entries travel with the reused crates, including
    the per-file credits to the Second Life viewer and Firestorm (LGPL 2.1).
  - The Aurora name and logo are trademarks and are not used.
  - Closed-source region modules can no longer be linked into the server.
  - GPLv3 does not oblige a grid operator to publish changes to a server
    that users only reach over the network. AGPLv3 would. Not adopted.
- **D2, repository name and home.**

Aurora's `aurora-msg` is generated from the viewer's message template, which
Linden Lab publishes under LGPL 2.1. That settles the template's provenance
for a GPLv3 project, subject to confirming the file's header.

## Impact

- Produces `docs/licence-audit.md` and the `cargo deny` allow-list.
- Risk: LOW to implement, but an unresolved provenance item blocks shipping
  the affected data.
