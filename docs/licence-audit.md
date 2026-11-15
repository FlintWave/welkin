# Licence Audit -- welkin

Audit of the OpenSimulator 0.9.3.0 C# source tree and its dependencies,
performed 2026-11-13 against the C# reference tree
(`opensim-0.9.3.0-source`).

Project licence: **GPL-3.0-or-later**.

---

## Executive summary

welkin is a clean-room Rust port of OpenSimulator, licensed
GPL-3.0-or-later. This audit examined every third-party library, binary,
and data file shipped in the C# source tree to determine what can be
reused, what must be replaced, and what licence obligations apply.

**Key findings:**

1. **All C# third-party dependencies are dropped.** The Rust port replaces
   every C# library with a Rust equivalent. No Ms-PL, MPL-1.1, or
   custom-licensed code carries forward.
2. **Aurora Viewer crates are GPL-3.0-or-later**, compatible with the
   project licence. They transitively bring in openjpeg-sys (BSD-2-Clause),
   V-HACD (BSD-3-Clause), and parry3d (Apache-2.0) -- all compatible.
3. **The SL message template** is LGPL-2.1 (Linden Lab), which is
   GPLv3-compatible. It lives in the aurora-msg crate, not in this repo.
4. **No default asset library is shipped.** The C# tree's assets have
   mixed provenance (CC-BY-SA 3.0, unknown, CC-BY-NC). The server starts
   without them.
5. **All Rust dependency licences are GPLv3-compatible.** The dependency
   tree uses MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0,
   CDLA-Permissive-2.0, BSL-1.0, CC0-1.0, and Unlicense -- all
   compatible with GPL-3.0-or-later.
6. **Three items were flagged as GPL-incompatible** in the C# tree
   (Ms-PL libraries, CC-BY-SA 3.0 artwork, Warp3D). All are dropped.

**Disposition summary:** of 76+ third-party items in the C# tree, zero
are carried into the Rust port. The only external code comes from
crates.io dependencies and Aurora Viewer git dependencies, all with
confirmed-compatible licences. A `deny.toml` enforces the allow-list
via `cargo deny`.

---

## Decisions

- **D1 (2026-11-13):** GPL-3.0-or-later, so that Aurora Viewer's crates can
  be reused. Ported OpenSimulator code keeps its BSD-3-Clause notice.
- **D2 (2026-11-13):** Repository name `welkin`, local development only.

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
Aurora name and logo are trademarks -- not used.

---

## 2.1 ThirdPartyLicenses/ and bin/ -- full inventory

### ThirdPartyLicenses/ files

| File | Library | Licence | Disposition for Rust port |
|---|---|---|---|
| Aurora-Sim.txt | Aurora Sim (precursor) | BSD-3-Clause | Historical reference only; not used |
| BclExtras.txt | BclExtras | Ms-PL | **Dropped** (Ms-PL is GPL-incompatible) |
| Bullet for Xna (ModifiedBulletX).txt | BulletX .NET port | zlib | Dropped; replaced by Rapier |
| Bullet-XNA.txt | Bullet-XNA C#/XNA port | zlib | Dropped; replaced by Rapier |
| BulletLicense.txt | Bullet Physics | zlib | Dropped; replaced by Rapier |
| C# Webserver.txt | Gauffin HttpServer | Apache-2.0 | Dropped; replaced by axum |
| C5.txt | C5 Generic Collections | MIT | Dropped; std collections |
| CircularBuffer.txt | CircularBuffer | BSD-3-Clause | Dropped; not needed |
| ConvexDecompositionDotNet.txt | ConvexDecompositionDotNet | MIT | Dropped; V-HACD in parry3d |
| CSCompilerTools.txt | Compiler Tools (Malcolm Crowe) | **"Used with permission"** (not OSS) | Dropped; not an open-source licence |
| CSJ2K.txt | CSJ2K (JJ2000 C# port) | Custom JJ2000 notice (field-of-use clause) | Dropped; replaced by openjpeg-sys |
| DefaultTerrain.txt | SL viewer default terrain | CC-BY-SA 3.0 (Linden Lab) | **Not GPLv3-compatible**; do not include; generate new terrain |
| DotNetOpenid.txt | DotNetOpenId | BSD-3-Clause | Dropped; openidconnect crate |
| DotNetZip.txt | DotNetZip | Ms-PL | **Dropped** (Ms-PL is GPL-incompatible) |
| DotNetZip-bzip2.txt | DotNetZip bzip2 | Apache-2.0 (from Apache commons) | Dropped; flate2/bzip2 crate |
| DotNetZip-zlib.txt | DotNetZip zlib | Ms-PL + MKL | **Dropped** (Ms-PL is GPL-incompatible) |
| GoogleProtoBuffer.txt | protobuf-csharp-port | BSD-3-Clause (Google) | Dropped; not needed |
| GTCache.txt | GlynnTucker.Cache | GPL/MPL-1.1 dual | **Dropped** (MPL-1.1 alone is GPL-incompatible) |
| ICSharpCode.SharpZipLib.license.txt | SharpZipLib | GPL-2.0+ with linking exception | Dropped; flate2/zip crate |
| libsl.txt | libsecondlife / OpenMetaverse | BSD-3-Clause | Replaced by Aurora crates + new code |
| log4net | log4net | Apache-2.0 | Dropped; tracing crate |
| MonoAddins.txt | Mono.Addins | MIT | Dropped; not needed |
| Mono.Xna (MonoXnaCompactMaths).txt | MonoXna compact maths | MIT | Dropped; glam crate |
| MXP.txt | Metaverse Exchange Protocol | Apache-2.0 | Dropped; not implemented |
| MySQL.txt | MySQL Connector/NET | GPLv2 + FLOSS exception | Dropped; sqlx crate |
| Nini.txt | Nini (INI parser) | MIT | Dropped; toml + configparser crate |
| Npgsql.txt | Npgsql (PostgreSQL driver) | PostgreSQL licence (permissive) | Dropped; sqlx crate |
| ODE.txt | Open Dynamics Engine | BSD-3-Clause | Dropped; Rapier crate |
| OpenJpeg.txt | OpenJPEG | BSD-2-Clause | Via openjpeg-sys in aurora-assets |
| Protobuf-net.txt | protobuf-net | BSD (via Google protobuf) | Dropped; not needed |
| Prototype.txt | Prototype.js | MIT | Dropped; not applicable to server |
| SmartThreadPool.txt | SmartThreadPool | Ms-PL | **Dropped** (Ms-PL is GPL-incompatible) |
| XML-RPC.NET.txt | XML-RPC.NET | MIT (X11) | Dropped; custom Rust implementation |

### bin/ DLLs -- full inventory

| DLL | Library | Licence | Disposition |
|---|---|---|---|
| BouncyCastle.Cryptography.dll | BouncyCastle | MIT | Dropped; rustls + ring |
| BulletXNA.dll | Bullet XNA C# port | zlib | Dropped; Rapier |
| C5.dll | C5 Generic Collections | MIT | Dropped; std collections |
| CSJ2K.dll | CSJ2K JPEG 2000 | Custom JJ2000 | Dropped; openjpeg-sys |
| DotNetOpenId.dll | DotNetOpenId | BSD-3-Clause | Dropped; openidconnect crate |
| ICSharpCode.SharpZipLib.dll | SharpZipLib | GPL-2.0+ with linking exception | Dropped; flate2 |
| Ionic.Zip.dll | DotNetZip | Ms-PL | Dropped |
| libzstd.dll | Zstandard native | BSD-3-Clause (Facebook) | Dropped; zstd crate if needed |
| log4net.dll | log4net | Apache-2.0 | Dropped; tracing |
| LukeSkywalker.IPNetwork.dll | IPNetwork | Apache-2.0 | Dropped; std::net / ipnetwork crate |
| MailKit.dll | MailKit | MIT | Dropped; lettre crate |
| mautil.exe | Mono.Addins utility | MIT (part of Mono.Addins) | Dropped; not needed |
| Microsoft.Extensions.Logging.Abstractions.dll | .NET runtime | MIT (Microsoft) | Dropped; tracing |
| MimeKit.dll | MimeKit | MIT | Dropped; lettre crate |
| Mono.Addins.CecilReflector.dll | Mono.Addins | MIT | Dropped; not needed |
| Mono.Addins.dll | Mono.Addins | MIT | Dropped; not needed |
| Mono.Addins.Setup.dll | Mono.Addins | MIT | Dropped; not needed |
| Mono.Cecil.dll | Mono.Cecil | MIT | Dropped; not needed |
| Mono.Data.Sqlite.dll | Mono SQLite binding | MIT | Dropped; sqlx |
| Mono.Data.SqliteClient.dll | Mono SQLite legacy client | MIT | Dropped; sqlx |
| Mono.Security.dll | Mono.Security | MIT | Dropped; rustls |
| MySql.Data.dll | MySQL Connector/NET | GPLv2 + FLOSS exception | Dropped; sqlx |
| NDesk.Options.dll | NDesk.Options | MIT | Dropped; clap crate |
| netcd.dll | etcd client | Unknown (no licence file in tree) | Dropped; not needed |
| Newtonsoft.Json.dll | Json.NET | MIT | Dropped; serde_json |
| Nini.dll | Nini | MIT | Dropped; configparser |
| Npgsql.dll | Npgsql | PostgreSQL licence (permissive) | Dropped; sqlx |
| nunit.framework.dll | NUnit | MIT | Dropped; Rust built-in tests |
| OpenMetaverse.dll | OpenMetaverse (libomv) | BSD-3-Clause | Replaced by Aurora crates |
| OpenMetaverse.Rendering.Meshmerizer.dll | OpenMetaverse Meshmerizer | BSD-3-Clause | Replaced by welkin-prim |
| OpenMetaverse.StructuredData.dll | OpenMetaverse LLSD | BSD-3-Clause | Replaced by aurora-llsd |
| OpenMetaverseTypes.dll | OpenMetaverse types | BSD-3-Clause | Replaced by welkin-types |
| prebuild.dll | Prebuild build tool | BSD-3-Clause | Dropped; Cargo replaces it |
| PrimMesher.dll | PrimMesher | BSD-3-Clause (OpenSim contributor) | Replaced by welkin-prim |
| RestSharp.dll | RestSharp | Apache-2.0 | Dropped; reqwest crate |
| System.Configuration.ConfigurationManager.dll | .NET runtime | MIT (Microsoft) | Dropped; not needed |
| System.Drawing.Common.dll.linux/win | .NET runtime | MIT (Microsoft) | Dropped; image crate if needed |
| System.Runtime.Caching.dll | .NET runtime | MIT (Microsoft) | Dropped; not needed |
| System.Security.Permissions.dll | .NET runtime | MIT (Microsoft) | Dropped; not needed |
| Tools.dll | OpenSim build tools | BSD-3-Clause (OpenSim) | Dropped; Cargo replaces it |
| Warp3D.dll | Warp3D software renderer | **UNKNOWN -- see 2.4** | Not needed for server |
| XMLRPC.dll | XML-RPC.NET | MIT | Dropped; custom implementation |
| zlib.net.dll | zlib.net managed zlib | zlib/libpng | Dropped; flate2 |
| ZstdNet.dll | ZstdNet (.NET wrapper) | BSD-3-Clause | Dropped; zstd crate if needed |

### bin/lib64/ native libraries

| File | Library | Licence | Disposition |
|---|---|---|---|
| BulletSim-*.dll/.so/.dylib | BulletSim native | zlib (Bullet Physics) | Dropped; Rapier |
| libBulletSim*.so/.dylib | BulletSim native | zlib (Bullet Physics) | Dropped; Rapier |
| libopenjpeg-dotnet*.so/.dll/.dylib | OpenJPEG .NET wrapper | BSD-2-Clause | Via openjpeg-sys in aurora-assets |
| libsqlite3*.so/.dylib, sqlite3.dll | SQLite | Public domain | Via sqlx + system sqlite |
| libubode*.so/.dylib, ubode.dll | ubODE (modified ODE) | BSD-3-Clause / LGPL-2.1 dual | Dropped; Rapier |

### bin/ non-binary files of note

| File | What | Licence | Disposition |
|---|---|---|---|
| avatar-texture.dat | Avatar texture UUID map | Unclear; OpenSim data | Investigate if needed |
| defaultstripe.png | Default texture | Unclear | Generate new if needed |
| ScriptSyntax.xml | LSL function signatures | BSD-3-Clause (OpenSim) | May port for script compiler |
| OpenSimDefaults.ini | Default configuration | BSD-3-Clause (OpenSim) | Reference; rewrite in TOML |
| excuses | Humour file | N/A | Not needed |

---

## 2.2 Protocol message template licence

The message template file (`message_template.msg`) defines the Second Life
UDP protocol messages. It is found at:

    aurora-msg crate: crates/aurora-msg/message_template.msg

**Header text:** `"Linden Lab development message templates"`, version 2.0.
No explicit licence statement appears in the file itself.

**Origin:** This file originates from the Second Life viewer source code,
which Linden Lab released under **LGPL 2.1** (GNU Lesser General Public
License version 2.1). The file has been maintained by the viewer community
and updated for protocol extensions.

**Licence determination:**
- The SL viewer source (including indra/llmessage/ and the template) was
  released under LGPL 2.1 by Linden Lab.
- LGPL 2.1 is GPLv3-compatible (FSF confirmed: LGPL 2.1 code can be
  included in a GPL-3.0-or-later project).
- Aurora-msg's build.rs generates Rust structs from this template at compile
  time. The generated code is a derivative work of the template.
- **Status: CLEAR.** LGPL 2.1 -> GPL-3.0-or-later is permitted.

**Note:** The template is not shipped in welkin directly. It lives in
the aurora-msg crate (pinned git dependency) and is consumed via build.rs
code generation. No .msg file is distributed with welkin binaries.

---

## 2.3 bin/openmetaverse_data/ and bin/assets/ licences

### openmetaverse_data/ (61 files)

Contents: `avatar_lad.xml`, `avatar_skeleton.xml`, and 59 TGA texture files
(avatar baking textures: skin, hair, clothing alphas, bump maps).

**Origin:** These files come from the Linden Lab Second Life viewer. They
were bundled with the OpenMetaverse / libsecondlife library.

**Licence status:**
- The SL viewer artwork was released under **CC-BY-SA 3.0** (see
  `ThirdPartyLicenses/DefaultTerrain.txt` which contains the Linden artwork
  notice).
- The XML data files (`avatar_lad.xml`, `avatar_skeleton.xml`) are structural
  data from the viewer source, covered by the viewer's **LGPL 2.1**.
- No explicit licence file exists in the `openmetaverse_data/` directory.

**Disposition:**
- **avatar_lad.xml, avatar_skeleton.xml**: LGPL 2.1, GPLv3-compatible. Use
  Aurora Viewer's copies which are properly attributed under GPL-3.0-or-later.
- **TGA textures (baking)**: CC-BY-SA 3.0. **NOT GPLv3-compatible** (only
  CC-BY-SA 4.0 has the one-way GPL compatibility provision). Do not bundle
  in welkin. The server does not need these; they are viewer-side baking
  templates.

### bin/assets/ (default asset library)

The asset library contains multiple asset sets loaded at server startup:

| Asset set | Contents | File count | Licence status | Disposition |
|---|---|---|---|---|
| AnimationsAssetSet | Avatar animation .dat files | ~85 | **Unknown**: no licence marker; likely Linden viewer content | Do not redistribute |
| Avatar | Avatar appearance data | Small | Likely OpenSim-created | Investigate |
| BodyPartsAssetSet | Body part definitions | Small | OpenSim-created (BSD-3-Clause) | May reuse |
| ClothingAssetSet | Clothing definitions | Small | OpenSim-created (BSD-3-Clause) | May reuse |
| CollisionSoundsAssetSet | Collision .ogg sounds | ~57 | Freesound.org credits; individual licences vary (CC0, CC-BY, possibly CC-BY-NC) | **Audit individually**; CC-BY-NC is NOT compatible |
| GesturesAssetSet | Gesture definitions | Small | OpenSim-created | May reuse |
| LandmarksAssetSet | Default landmarks | Small | OpenSim-created | Not needed |
| MaterialsAssetSet | Material definitions | Small | OpenSim-created | May reuse |
| MyAssetSet | User template assets | Small | OpenSim-created | May reuse |
| NotecardsAssetSet | Notecards | Small | OpenSim-created | May reuse |
| ObjectsAssetSet | Default objects | Small | OpenSim-created | May reuse |
| PhotosAssetSet | Photos | Small | Unknown | Investigate |
| ScriptsAssetSet | LSL scripts | Small | OpenSim-created (BSD-3-Clause) | May port |
| SettingsAssetSet | Environment settings | Small | OpenSim-created | May reuse |
| SoundsAssetSet | System sounds | Small | Unknown origin; may be Linden | Investigate |
| TexturesAssetSet | Default textures (.jp2) | ~155 | Mixed: some OpenSim-created, some Linden CC-BY-SA 3.0 | **Audit individually** |

**Key finding:** `bin/data/LICENSE-README-IMPORTANT.txt` explicitly warns:
> "Not all of the files in this directory are licensed under the BSD license.
> Some of these files come with the Second Life viewer and are considered
> licensed under a Creative Commons License."
> - `avataranimations.xml`: CC-BY-SA 2.5

**Disposition for Rust port:** The server can start without a default asset
library (grids import their own). Ship no default asset library initially.
If one is needed later, build it from confirmed-licensed content only.

---

## 2.4 Warp3D and remaining unclassified DLLs

### Warp3D

| Item | Detail |
|---|---|
| Binary | `bin/Warp3D.dll` (73 KB) |
| Source in tree | **None.** Only consumer code in `OpenSim/Region/CoreModules/World/Warp3DMap/` |
| ThirdPartyLicenses entry | **None** |
| Usage | Server-side map tile rendering (software rasterizer) |
| Origin | Believed to be from the Avination/OpenSim fork; original Warp3D is a Java software renderer by Peter Walser. The Java version was released under a non-commercial restriction. |
| Licence | **UNKNOWN / PRESUMED RESTRICTIVE.** No licence file ships with the DLL, no header in the consuming C# code, and the original Java Warp3D had a non-commercial clause. |
| Disposition | **Not needed.** The Rust server does not render map tiles itself. If map tile generation is needed later, reimplement with `tiny-skia` (MIT/Apache-2.0) or delegate to the viewer. |

### Remaining unclassified DLLs

| DLL | Investigation | Licence | Disposition |
|---|---|---|---|
| netcd.dll (23 KB) | etcd client for the EtcdMonitoringModule (optional module) | No licence file in tree; likely MIT or Apache-2.0 (common for etcd clients) | Dropped; etcd monitoring not in scope |
| Tools.dll (162 KB) | OpenSimulator build/packaging tools | BSD-3-Clause (part of OpenSim) | Dropped; Cargo replaces it |
| LukeSkywalker.IPNetwork.dll (18 KB) | IP address/subnet calculation library | Apache-2.0 (NuGet page) | Dropped; ipnetwork2 or std::net |
| ZstdNet.dll (29 KB) | .NET wrapper for Zstandard | BSD-3-Clause | Dropped; zstd crate if needed |
| libzstd.dll (581 KB) | Zstandard native library | BSD-3-Clause (Facebook/Meta) | Dropped; zstd crate if needed |

**All remaining DLLs are now classified.** No unidentified binaries remain.

---

## 2.5 JPEG 2000 evaluation

### Current state

JPEG 2000 (J2K) is the mandatory texture format in the Second Life protocol.
Every texture asset is stored and transmitted as a `.jp2` file.

**In the C# tree:** Two implementations exist:
- `CSJ2K.dll`: Pure C# port of JJ2000. Custom licence with a field-of-use
  clause -- not clearly open-source.
- `libopenjpeg-dotnet` (native, in lib64/): C wrapper around OpenJPEG.
  BSD-2-Clause. This is the one actually used at runtime.

**In Aurora Viewer:** The `aurora-assets` crate depends on `openjpeg-sys`
(version 1.0.12), which is a Rust `-sys` binding to the C OpenJPEG library.
Licence: BSD-2-Clause. This is compiled from C source via `cc` build
dependency.

**In welkin currently:** `aurora-assets` is declared as a workspace
dependency in the root `Cargo.toml` but is not actually used by any crate
(no `aurora-assets` in any crate-level `Cargo.toml` `[dependencies]`).
However, it appears in `Cargo.lock`, meaning it is resolved. The `openjpeg-sys`
dependency comes transitively through `aurora-assets`.

### Pure Rust alternatives

| Crate | Status | Notes |
|---|---|---|
| `jpeg2000` (crates.io) | Also wraps OpenJPEG via -sys | Not pure Rust; same C dependency |
| `jxl-oxide` | JPEG XL, not JPEG 2000 | Wrong format |
| No pure Rust J2K decoder | Does not exist as of 2026-11 | JPEG 2000 is complex (DWT + arithmetic coding + rate-distortion); no one has written a full pure-Rust implementation |

### Recommendation

1. **Continue using openjpeg-sys** via aurora-assets. BSD-2-Clause is
   GPL-3.0-compatible. The C library is mature, fast, and well-tested.
2. **Accept the C dependency.** A pure Rust JPEG 2000 decoder does not exist
   and would be a massive undertaking (~50k lines of complex signal processing).
3. **If avoiding C FFI becomes a goal**, the only option is to transcode
   textures to PNG/KTX2 at ingest and serve a non-J2K format, which would
   break viewer compatibility.
4. When welkin actually needs J2K decode/encode (texture upload handling,
   map tile generation), add `aurora-assets` or `openjpeg-sys` directly as a
   dependency to the relevant crate.

---

## 2.6 Aurora's [patch.crates-io] section

The Aurora workspace `Cargo.toml` (at commit `3d523b14`) contains:

```toml
[patch.crates-io]
naga = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
naga-types = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
wgpu = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
wgpu-core = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
wgpu-core-deps-apple = { ... }
wgpu-core-deps-emscripten = { ... }
wgpu-core-deps-wasm = { ... }
wgpu-core-deps-windows-linux-android = { ... }
wgpu-hal = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
wgpu-naga-bridge = { ... }
wgpu-types = { git = "https://github.com/Aurora-Viewer/wgpu", rev = "3bad1916..." }
```

### What these patches do

All 11 patches replace upstream wgpu 30.0.1 crates with Aurora's fork
(`Aurora-Viewer/wgpu`, branch `aurora/v30.0.1-empty-pool`). The fork carries
a single 11-line fix for Intel 6th-9th gen iGPU Vulkan drivers that crash on
an empty descriptor pool (`gfx-rs/wgpu#10124`, Aurora `#1302`).

### Impact on welkin

**None.** These patches are irrelevant for several reasons:

1. **welkin does not depend on wgpu.** It is a server project; GPU
   rendering is not in scope. Confirmed: `wgpu` and `naga` do not appear in
   `welkin/Cargo.lock`.

2. **`[patch.crates-io]` does not propagate through git dependencies.** When
   Cargo resolves a `git` dependency (as welkin uses for Aurora crates),
   it resolves only the crates in that git repo that the consumer actually
   depends on. The workspace-level `[patch]` section of the _dependency's_
   `Cargo.toml` is **ignored** by the consumer. Only patches declared in the
   _consumer's_ own workspace `Cargo.toml` apply.

3. **The Aurora crates welkin uses** (`aurora-llsd`, `aurora-msg`,
   `aurora-prim`, `aurora-net`, `aurora-assets`) do not depend on wgpu. The
   wgpu dependency comes from `aurora-render` and `aurora-viewer`, which
   welkin does not pull in.

**No action needed.** The patches do not affect welkin's build, its
dependency tree, or its licence obligations.

---

## GPLv3 compatibility summary

Licences confirmed GPLv3-compatible: MIT, BSD-2-Clause, BSD-3-Clause,
Apache-2.0, ISC, zlib, PostgreSQL licence (Npgsql), SIL OFL 1.1 (fonts),
CC0 1.0, LGPL-2.1-or-later.

Licences NOT GPLv3-compatible or uncertain:
- **Ms-PL** (BclExtras, DotNetZip, SmartThreadPool): FSF considers
  GPL-incompatible. All dropped.
- **MPL-1.1** (GTCache): Not GPL-compatible on its own. Dropped.
- **CC-BY-SA 3.0** (viewer artwork, default terrain): Only 4.0 is one-way
  GPLv3-compatible. Do not include 3.0 content.
- **CC-BY-SA 2.5** (avataranimations.xml): Same issue. Do not include.
- **Custom JJ2000** (CSJ2K): Field-of-use restriction. Dropped; replaced by
  openjpeg.
- **"Used with permission"** (CSCompilerTools): Not an open-source licence.
  Dropped.
- **GPLv2 + FLOSS exception** (MySQL Connector): Compatible only via the
  exception. Dropped; replaced by sqlx.
- **Warp3D**: Unknown / presumed restrictive. Dropped.

## Naming and trademarks

- "Second Life" is a registered trademark of Linden Research, Inc. Use only
  in factual references to the platform or protocol.
- "OpenSimulator" names the upstream project from which this port derives.
  No endorsement by the OpenSimulator project is implied.
- "Aurora Viewer" and the Aurora logo are trademarks. Not used for this
  project or its binaries.
- Binary names: `welkin-robust`, `welkin-region`. The `opensim-` prefix
  identifies compatibility, not affiliation.
- LSL (Linden Scripting Language) is a descriptive term used by multiple
  projects. No trademark issue identified.

## Recommendations

1. **Run `cargo deny check` in CI.** The workspace `deny.toml` enforces
   the licence allow-list and checks the RustSec advisory database.
   Add it to the build pipeline.
2. **Do not ship a default asset library.** The C# tree's assets have
   mixed and unverified provenance. Let grid operators import their own.
   If a default library is needed later, build it from scratch with
   confirmed-licensed content only.
3. **Keep openjpeg-sys.** No pure Rust JPEG 2000 implementation exists.
   The BSD-2-Clause C library is mature and GPLv3-compatible. Accept
   the C FFI dependency.
4. **Do not include CC-BY-SA 3.0 content.** Only CC-BY-SA 4.0 has the
   one-way GPLv3 compatibility provision. This rules out Linden Lab
   viewer artwork and the default terrain texture.
5. **Audit collision sounds individually** before shipping any. The
   Freesound.org sourced sounds may include CC-BY-NC, which is
   GPL-incompatible.
6. **Pin Aurora Viewer crates to a reviewed commit.** Currently pinned at
   `3d523b14`. Re-audit when updating the pin.
7. **Generate new default textures** (terrain, avatar defaults) rather
   than porting the existing ones with unclear provenance.

---

## Open items

1. **bin/assets individual texture mapping**: Textures in TexturesAssetSet are
   not individually mapped to their licence source. If we ship a default asset
   library, each texture needs a confirmed licence.
2. **CollisionSounds licences**: Freesound.org sources need individual licence
   verification (CC0, CC-BY, or CC-BY-NC -- the last is NOT compatible).
3. **Default library strategy**: The C# default library has mixed provenance.
   Options: (a) ship no default library and let grids import their own,
   (b) build a new library from confirmed-licensed content,
   (c) audit each asset individually.
4. **netcd.dll licence**: No licence file found. Low priority (module dropped).
5. **Warp3D original licence**: If ever needed, would require sourcing clean
   implementation. Current plan: not needed for server.
