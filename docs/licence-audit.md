# Licence Audit — opensim-rust

Audit of the OpenSimulator 0.9.3.0 C# source tree and its dependencies,
performed 2026-11-13 against the tree at
`/var/home/erik/Projects/opensim-0.9.3.0-source/`.

Project licence: **GPL-3.0-or-later**.

## Decisions

- **D1 (2026-11-13):** GPL-3.0-or-later, so that Aurora Viewer's crates can
  be reused. Ported OpenSimulator code keeps its BSD-3-Clause notice.
- **D2 (2026-11-13):** Repository name `opensim-rust`, local development only.

## OpenSimulator source

BSD-3-Clause (LICENSE.txt, CONTRIBUTORS.txt). All ported code retains the
notice. GPLv3 permits including BSD-3-Clause code.

## Aurora Viewer crates

| Crate | Licence | Disposition |
|---|---|---|
| aurora-llsd | GPL-3.0-or-later | Pinned git dep, wrapped |
| aurora-msg | GPL-3.0-or-later (message template: LGPL 2.1 from Linden Lab) | Pinned git dep, wrapped |
| aurora-prim | GPL-3.0-or-later (code derived from SL viewer, originally LGPL 2.1) | Pinned git dep, wrapped |
| aurora-assets | GPL-3.0-or-later; bundles openjpeg-sys (BSD-2), V-HACD (BSD-3), parry3d (Apache-2.0) | Pinned git dep, wrapped |
| aurora-net | GPL-3.0-or-later | Test bot only, not in server |

Pinned at commit `3d523b14a17bbc6484b208378c0eef291ad5ac32`.
Aurora name and logo are trademarks — not used.

## Third-party components (C# tree)

### Ported or reused

| Component | Original licence | Disposition |
|---|---|---|
| OpenSimulator source | BSD-3-Clause | Port; keep notice |
| Protocol message template | LGPL 2.1 (Linden Lab, via viewer) | Via aurora-msg; confirmed |
| OpenMetaverse (libsl) | BSD-3-Clause | Replace with Aurora crates + new code |

### Dropped (not ported, replaced by Rust equivalents)

| Component | Original licence | Replacement |
|---|---|---|
| ODE (ubODE) | BSD-3-Clause / LGPL-2.1 dual | Rapier (Apache-2.0) |
| Bullet / BulletSim | zlib (Bullet); wrapper licence unstated | Rapier (Apache-2.0) |
| CSJ2K (JJ2000) | Custom JJ2000 notice (field-of-use restriction, NOT BSD) | openjpeg-sys via aurora-assets (BSD-2) |
| OpenJPEG (.NET wrapper) | BSD-2-Clause | openjpeg-sys via aurora-assets |
| Warp3D | **UNKNOWN** (no licence in tree; Avination build) | Reimplement with tiny-skia or wgpu |
| MySql.Data | GPLv2 + FLOSS exception | sqlx (MIT/Apache-2.0) |
| Vivox voice client | Proprietary service | Drop; WebRTC via wolfvoice |
| FreeSWITCH voice | Open, legacy | Drop; WebRTC via wolfvoice |
| Nini (INI parser) | MIT | toml + custom INI importer |
| log4net | Apache-2.0 | tracing (MIT) |
| Mono.Addins | MIT | Not needed |
| SharpZipLib | GPL-2.0+ with linking exception | flate2 / zip (MIT) |
| DotNetZip | Ms-PL | flate2 / zip (MIT) |
| BclExtras | Ms-PL | Not needed |
| SmartThreadPool | Ms-PL | tokio (MIT) |
| GTCache | MPL-1.1 | Not needed |
| CSCompilerTools | "Used with permission" (no OSS licence) | Not needed (SLua replaces LSL compiler) |
| C5 collections | MIT | std collections |
| Newtonsoft.Json | MIT | serde_json (MIT/Apache-2.0) |
| XML-RPC.NET | MIT | Custom implementation |
| Prototype.js | MIT | axum web framework |
| Google Protobuf | BSD-3-Clause | Not needed |
| DotNetOpenId | BSD | openidconnect crate (MIT/Apache-2.0) |
| NUnit | MIT | Rust built-in tests |
| ConvexDecompositionDotNet | MIT (Intel) | V-HACD in parry3d (Apache-2.0) |
| RestSharp | Apache-2.0 | reqwest (MIT/Apache-2.0) |
| Npgsql | PostgreSQL licence (permissive) | sqlx |
| BouncyCastle | MIT | rustls (MIT/Apache-2.0/ISC) |
| MailKit / MimeKit | MIT | lettre (MIT/Apache-2.0) |

### Data files — provenance status

| Item | Files | Licence status | Disposition |
|---|---|---|---|
| bin/openmetaverse_data/ | 61 (avatar_lad.xml, avatar_skeleton.xml, 59 TGA) | **Unlicensed in tree**; Linden Lab viewer data, probable LGPL 2.1 or CC-BY-SA 3.0 | Use Aurora Viewer's copies (properly attributed, LGPL 2.1 → GPL-3.0-or-later) |
| bin/assets/ TexturesAssetSet | 155 | Partial: public domain, MIT, CC-BY-SA 3.0 (see licenses.txt) | Audit individually if redistributed; CC-BY-SA 3.0 is NOT GPLv3-compatible |
| bin/assets/ CollisionSounds | 57 .ogg | Freesound.org credits, no licence terms | Do not redistribute; source new sounds under compatible licences |
| bin/assets/ AnimationsAssetSet | 154 | No licence marker; likely Linden content | Do not redistribute without confirmation |
| bin/assets/ (other sets) | ~100 | No licence markers | Do not redistribute without confirmation |
| bin/data/avataranimations.xml | 1 | CC-BY-SA 2.5 | Not GPLv3-compatible; use Aurora's equivalent |
| DefaultTerrain | 1 | CC-BY-SA 3.0 (Linden) | Not GPLv3-compatible; generate or source new terrain |

### GPLv3 compatibility summary

Licences confirmed GPLv3-compatible: MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0,
ISC, zlib, PostgreSQL licence (Npgsql), SIL OFL 1.1 (fonts), CC0 1.0.

Licences NOT GPLv3-compatible or uncertain:
- **Ms-PL** (BclExtras, DotNetZip, SmartThreadPool): FSF considers GPL-incompatible. All dropped.
- **MPL-1.1** (GTCache): Not GPL-compatible on its own. Dropped.
- **CC-BY-SA 3.0** (viewer artwork, default terrain): Only 4.0 is one-way GPLv3-compatible. Do not include 3.0 content.
- **CC-BY-SA 2.5** (avataranimations.xml): Same issue. Do not include.
- **Custom JJ2000** (CSJ2K): Field-of-use restriction. Dropped; replaced by openjpeg.
- **"Used with permission"** (CSCompilerTools): Not an open-source licence. Dropped.
- **GPLv2 + FLOSS exception** (MySQL Connector): Compatible only via the exception. Dropped; replaced by sqlx.

## Naming and trademarks

- "Second Life" is a registered trademark of Linden Research, Inc. Use only
  in factual references to the platform or protocol.
- "OpenSimulator" names the upstream project from which this port derives.
  No endorsement by the OpenSimulator project is implied.
- "Aurora Viewer" and the Aurora logo are trademarks. Not used for this
  project or its binaries.
- Binary names: `opensim-robust`, `opensim-region`. The `opensim-` prefix
  identifies compatibility, not affiliation.
- LSL (Linden Scripting Language) is a descriptive term used by multiple
  projects. No trademark issue identified.

## Open items

1. **bin/assets individual texture mapping**: Textures in TexturesAssetSet are
   not individually mapped to their licence source. If we ship a default asset
   library, each texture needs a confirmed licence.
2. **CollisionSounds licences**: Freesound.org sources need individual licence
   verification (CC0, CC-BY, or CC-BY-NC — the last is NOT compatible).
3. **Default library strategy**: The C# default library has mixed provenance.
   Options: (a) ship no default library and let grids import their own,
   (b) build a new library from confirmed-licensed content,
   (c) audit each asset individually.
