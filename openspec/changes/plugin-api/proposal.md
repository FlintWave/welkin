# Plugin API

**Status:** Future / Speculative — no resources allocated
**Priority:** Post-release
**Depends on:** integration-wiring (core server must be stable first)

## Summary

A plugin system for Welkin that lets external tools, services, and community
code connect to the running server through a stable, versioned API. Plugins
could provide new region modules, chat integrations, LLM backends, content
generation pipelines (EmbodiedGen, Meshy), custom physics, economy systems,
or anything else that extends the server without modifying Welkin's core.

## How OpenSim does it now

OpenSim uses Mono.Addins for module discovery and loading. The system has two
module types, both extending `IRegionModuleBase`:

**`INonSharedRegionModule`**: one instance per region. Used by physics, wind,
sun, script engines, and most region-specific logic. Each instance gets
`AddRegion(Scene)` called once, receives its own scene reference, and manages
its own lifecycle.

**`ISharedRegionModule`**: one instance across all regions. Used for grid-wide
services like user management and login connectors. Gets `AddRegion` called
once per scene, plus `PostInitialise` after all modules load.

Both types are discovered via `[Extension(Path = "/OpenSim/RegionModules")]`
attributes. The `RegionModulesControllerPlugin` (registered at
`/OpenSim/Startup`) scans assemblies, checks config for enabled modules, and
instantiates them. The `ReplaceableInterface` property allows stub modules to
be swapped out by third-party replacements without code changes.

**Strengths of this approach:**
- Well-understood lifecycle (Initialise, AddRegion, RegionLoaded, RemoveRegion, Close)
- Shared vs per-region scoping covers most use cases
- ReplaceableInterface allows clean substitution

**Weaknesses:**
- Mono.Addins ties the system to .NET/Mono runtime
- No sandboxing; a bad module can crash the server
- No versioned API contract; modules break across OpenSim releases
- No hot-reload; module changes require server restart
- No built-in permission model for what a module can access

## What other game servers do

### Minecraft (Bukkit/Spigot/Paper)

The most successful server plugin ecosystem. Key design choices:
- **Stable API layer**: plugins code against the Bukkit API, which is an
  interface layer over the server internals. The API stays stable across
  server implementations (Bukkit, Spigot, Paper all implement it).
- **Event-driven**: plugins register listeners for events (PlayerJoinEvent,
  BlockPlaceEvent, etc.). The server fires events and plugins react.
- **plugin.yml manifest**: each plugin declares name, version, dependencies,
  load order, and permissions in a YAML file.
- **Classloader isolation**: each plugin gets its own classloader, providing
  some isolation (though not true sandboxing).
- **No hot-reload** in practice; plugins load at startup.

### Garry's Mod / Source Engine

- Lua scripting layer over C++ engine
- Hot-reload via `lua_run` console command
- No sandboxing; server operators trust their addons
- Workshop integration for distribution

### Common patterns across all systems

1. A stable API surface that insulates plugins from internal changes
2. Event/hook system for plugins to react to game events
3. A manifest file declaring metadata and dependencies
4. Some form of load-order control
5. A way to register new commands, capabilities, or content

## Proposed architecture for Welkin

Three tiers, from lightest to heaviest integration:

### Tier 1: REST/gRPC API (external tools)

An HTTP/gRPC endpoint that external tools call into. Runs in a separate
thread, authenticated via API keys configured in `welkin.toml`.

```
External tool ──► HTTPS/gRPC ──► Welkin API endpoint
                                       │
                                       ▼
                                 Permission check
                                       │
                                       ▼
                                 Server action
```

Use cases:
- EmbodiedGen sends generated assets for import
- An LLM service queries region state and sends build commands
- A web dashboard reads statistics and manages users
- A Discord/Matrix bot bridges chat

This is the simplest tier to implement and the safest, since external tools
can only do what the API explicitly exposes. OpenSim already has a partial
version of this in its ROBUST service connectors and the SimConsoleAsync cap.

### Tier 2: WASM plugins (sandboxed in-process)

WebAssembly modules that run inside the server process with controlled access
to server APIs. The server loads .wasm files, provides host functions (the
plugin API), and executes plugin code in a sandboxed runtime.

```
Plugin (.wasm) ──► Wasmtime/Wasmer runtime ──► Host functions
                                                    │
                                                    ▼
                                              Server internals
                                              (via stable API only)
```

Relevant Rust crates:
- **wasmtime** (Bytecode Alliance): mature, production-ready WASM runtime
- **wasmer**: alternative runtime with good Rust integration
- **plugy** (0.3.1): WASM plugin framework for Rust, trait-based interface
- **wings**: newer WASM plugin system tied to the Geese event library
- **wasvy** (0.0.4): game-focused WASM plugin framework, early stage

Plugins would be compiled from Rust, C, or any language that targets WASM.
The host function interface is the API contract: plugins call
`welkin_spawn_object()`, `welkin_send_chat()`, `welkin_query_region()`, etc.

Advantages over native dynamic loading:
- Memory-safe sandboxing (plugins can't corrupt server memory)
- Platform-independent (same .wasm runs on Linux, macOS, Windows)
- Capability-based security (plugins only get the host functions they're
  granted)
- Hot-reload is straightforward (unload old module, load new one)

Disadvantages:
- Performance overhead for cross-boundary calls
- Limited access to system resources (by design, but constraining for some
  use cases)
- WASM component model is still maturing

### Tier 3: Native dynamic loading (power users)

Shared libraries (.so/.dylib/.dll) loaded via `libloading` at runtime. The
plugin implements a defined Rust trait, and the server calls into it through
a C-compatible FFI.

This is the most powerful option but the least safe. A native plugin has full
access to the process and can crash the server. Suitable for:
- Physics engine replacements
- Custom terrain generators
- Performance-critical modules where WASM overhead matters

Veloren (open-source Rust voxel game) uses this pattern for its plugin system,
with plans for server-managed plugins that are sent to clients on connect.

## Plugin manifest

Each plugin ships with a `plugin.toml`:

```toml
[plugin]
name = "embodiedgen-bridge"
version = "0.1.0"
api_version = "1.0"
authors = ["Community"]
license = "Apache-2.0"
description = "Bridges EmbodiedGen scene generation into Welkin"

[permissions]
spawn_objects = true
modify_terrain = false
read_region_state = true
send_chat = true
access_inventory = false

[dependencies]
# other plugins this one requires
```

The `permissions` section declares what server APIs the plugin needs. The
server operator reviews and approves permissions at install time, similar to
Android app permissions.

## Event system

Plugins register for events. The server fires events at defined points:

```rust
// Plugin registers interest
fn on_load(&mut self, api: &mut WelkinApi) {
    api.subscribe(Event::ChatMessage);
    api.subscribe(Event::ObjectRez);
    api.subscribe(Event::RegionHeartbeat);
}

// Server calls this when a subscribed event fires
fn on_event(&mut self, event: &Event, api: &mut WelkinApi) {
    match event {
        Event::ChatMessage { sender, channel, message, .. } => {
            // react to chat
        }
        _ => {}
    }
}
```

Event categories:
- **Chat**: message sent, channel activity
- **Object**: rez, derez, move, property change
- **Avatar**: login, logout, teleport, movement
- **Region**: heartbeat tick, terrain change, weather
- **Economy**: transaction, balance query
- **Admin**: console command, config change

## Security model

1. **Tier 1 (REST/gRPC)**: API key authentication, rate limiting, per-key
   permission scopes configured in `welkin.toml`
2. **Tier 2 (WASM)**: capability-based; plugins only get the host functions
   listed in their approved permissions. No filesystem or network access
   unless explicitly granted.
3. **Tier 3 (native)**: no sandboxing. Operator must trust the plugin.
   Recommended only for first-party or audited code.

All tiers:
- Audit log of plugin actions
- Rate limits on world-modifying operations
- Resource budgets (max objects per tick, max terrain edits per minute)
- Kill switch: operator can disable any plugin at runtime

## Scope: server-side vs client-side

The plugin API covers **server-side** integrations only. This distinction
matters because content can reach the world through two paths:

**Server-side (what the plugin API covers):**
- World state modification: rezzing objects, terrain changes, inventory injection
- Service integrations: LLM providers, chat bridges, content generation pipelines
- Grid operations: region management, user services, economy
- Anything that needs to talk to the running server directly

**Client-side (viewer's responsibility, not ours):**
- Local content creation: Blender modeling, Houdini procedural work, manual
  sculpting
- These tools export meshes (Collada, glTF) and the viewer uploads them through
  the standard mesh upload capability (UploadMesh cap)
- Aurora or any other viewer handles the upload UI; Welkin just accepts it
  through existing caps
- No plugin API needed for this path

**Gray area (either path works):**
- Terrain tools like QuadSpinner Gaia: a server operator could generate
  heightmaps server-side and push them through the plugin API's terrain
  endpoint, or a user could generate locally and upload through the viewer.
  Both are valid; the plugin API enables the server-side path.
- LLM-driven Blender workflows: an LLM builds objects in Blender (Blender
  has a Python API and MCP tools already exist for it), then pushes the
  exported mesh to Welkin through the plugin API rather than going through
  a viewer upload. This is the "external tool with its own API pushes content
  into the world" pattern.

The plugin API is a bridge for tools that need to modify the world without
going through a viewer. Content creation tools that work through the viewer's
upload pipeline don't need it at all.

## Example use cases

| Use case | Tier | Notes |
|---|---|---|
| EmbodiedGen scene import | 1 (REST) | Generates whole rooms/buildings, API imports |
| LLM chat copilot | 1 (REST) or 2 (WASM) | See llm-world-copilot proposal |
| Discord/Matrix chat bridge | 1 (REST) | Bidirectional message relay |
| Blender + LLM pipeline | 1 (REST) | LLM builds in Blender, pushes mesh to server |
| QuadSpinner Gaia terrain | 1 (REST) | Heightmap generation, server-side import |
| SideFX Houdini layouts | 1 (REST) | Procedural environment generation, batch import |
| Custom NPC behavior | 2 (WASM) | AI-driven NPCs, sandboxed |
| Economy system | 2 (WASM) | Transaction logic, balance tracking |
| Alternative physics engine | 3 (native) | Performance-critical |
| Custom terrain generator | 3 (native) | Procedural world building |
| Web admin dashboard | 1 (REST) | Read-only stats and management |
| Mesh generation (Meshy/Modly) | 1 (REST) | External API, import result |

## AI World Copilot (first plugin)

The AI world copilot is the flagship application of the plugin API. It is a
Tier 1 (REST) or Tier 2 (WASM) plugin that lets privileged in-world users
send natural-language requests to a configured LLM provider, which then takes
actions in the world: generate 3D objects, modify terrain, write scripts, query
region state, or answer questions about the sim.

### Input paths

Two input paths, both already present in the OpenSim codebase:

1. **RegionConsoleModule (SimConsoleAsync cap):** the user opens the viewer's
   region console and types `ai build a stone archway at <128,128,25>`. The
   server checks `IsEstateManagerOrOwner` or `IsGod`, routes the command to
   the LLM backend, and returns output through the same EventQueue response
   path. This is the primary path: structured, permission-checked, and
   auditable.

2. **Chat channel listener:** the user types `/9999 build me a table` on a
   designated channel. Simpler for users whose viewers don't expose the console
   UI, but less structured.

### Tools available to the LLM

| Tool | What it does |
|------|-------------|
| PrimXmlTool | Generates OpenSim XML2 prim descriptions and rezzes them in-world |
| MeshImportTool | Calls an external mesh service (EmbodiedGen, Meshy, Modly), imports the result |
| TerrainTool | Raises, lowers, smooths, or flattens terrain within configured bounds |
| ScriptTool | Generates LSL scripts, compiles through the script engine, places in inventory |
| RegionQueryTool | Read-only queries: object count, agent list, parcel info, physics stats |
| InventoryTool | Delivers generated items to the requesting agent's inventory |

Each tool is individually enabled by the server operator in `welkin.toml`.
Resource caps (max prims per request, max mesh size, terrain edit radius)
prevent a single request from overwhelming the region.

### Provider adapters

The module is provider-agnostic. Built-in adapters:
- **ollama**: local or remote instance, no API key needed for local. Good for private grids.
- **claude**: Anthropic API. Strongest reasoning.
- **openai**: OpenAI API. Broad model selection.
- **custom**: any OpenAI-compatible endpoint (vLLM, LiteLLM, etc.)

### Multi-turn context

The module maintains a short conversation context per user (configurable TTL,
default 5 minutes). Saying `ai make it taller` refers to the previous
request's output.

### Security

The copilot runs with the requesting agent's permissions, not elevated
permissions. It cannot do anything the user couldn't do manually, just faster.
API keys are server-side only; users never see them. Every request is logged
with the agent's UUID, timestamp, and tool invocations.

### Non-goals for initial version

No voice input, no image/vision input, no autonomous agents, no cross-region
operations, no real-time collaboration.

## EmbodiedGen as a generation backend

EmbodiedGen (Horizon Robotics, Apache 2.0) is the primary candidate for
server-side 3D generation through the MeshImportTool. It is not a simple
text-to-mesh tool; it is a world engine for embodied AI that produces
simulation-ready assets with collision meshes, physical properties, and
affordance annotations.

**V1 (2025):** six modules covering Image-to-3D, Text-to-3D, Texture
Generation, Articulated Object Generation, Scene Generation, and Layout
Generation.

**V2 (July 2026, arXiv 2607.07459):** adds multi-room and whole-house scene
generation with individually addressable furniture and room topology. This is
the most interesting capability for virtual worlds, since it can generate
entire furnished buildings rather than single objects.

**Requirements:** Python 3.10.13, CUDA (cu126 or cu128), LLM backend (Azure
OpenAI, OpenRouter, or Codex CLI). Development baseline is 24 GB VRAM (4090).
Practical minimum is 16-24 GB VRAM for the full pipeline. No multi-GPU
support (hardcodes CUDA_VISIBLE_DEVICES=0).

**Integration path:** EmbodiedGen outputs URDF and mesh files. Welkin would
convert from URDF/mesh to Collada or glTF, then import through the mesh upload
pipeline. The scene generation output (multiple objects with layout) would
become linked object sets in-world.

See `embodiedgen-notes.md` in this directory for detailed research notes.

## Compute rental model

GPU-intensive features like EmbodiedGen can be offered as a paid add-on for
hosted Welkin servers, creating a revenue path without closing the source.

**Cost structure:** a single EmbodiedGen generation takes 1-3 minutes of GPU
time. At cloud rates (~$0.48/hr for an RTX 4090), that is roughly $0.02 per
generation. Chargeable at $0.25-0.50 per generation, yielding 90%+ gross
margin.

**Market position:** no existing virtual world platform bundles AI generation
as a paid service. This is a first-mover opportunity.

**Open-core model:** Welkin stays open source. The hosted AI features are the
paid tier. Users with their own GPUs (24 GB+) can self-host the full pipeline.
The managed service targets users without that hardware.

See `compute-rental-research.md` in this directory for the full revenue
analysis.

## Non-goals for initial version

- Plugin marketplace or distribution system
- Client-side plugins (viewer extensions are out of scope)
- Plugin-to-plugin direct communication (use events instead)
- Backward compatibility with OpenSim Mono.Addins modules

## Sources

- [OpenSim IRegionModuleBase.cs](/var/home/erik/Projects/opensim-0.9.3.0-source/OpenSim/Region/Framework/Interfaces/IRegionModuleBase.cs)
- [OpenSim RegionModulesControllerPlugin.cs](https://sledjhamr.org/cgit/opensim-SC/tree/OpenSim/ApplicationPlugins/RegionModulesController/RegionModulesControllerPlugin.cs)
- [Bukkit/Spigot/Paper comparison](https://xgamingserver.com/blog/paper-vs-spigot-vs-bukkit/)
- [Veloren plugin guide](https://book.veloren.net/contributors/modders/writing-a-plugin.html)
- [plugy — Rust WASM plugin framework](https://docs.rs/crate/plugy/0.2.1)
- [wasvy — game-focused WASM plugins](https://docs.rs/crate/wasvy/0.0.4)
- [wings — WASM plugin system](https://docs.rs/crate/wings/latest)
- [Wasmtime](https://wasmtime.dev/)
