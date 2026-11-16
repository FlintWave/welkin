# LLM World Copilot

**Status:** Future / Speculative — no resources allocated
**Priority:** Post-release
**Depends on:** integration-wiring, modern-avatar-features (glTF import)

## Summary

A server-side module that lets privileged in-world users send natural-language
requests to a configured LLM provider, which can then take actions in the
world: generate and rez 3D objects, modify terrain, write scripts, query region
state, or answer questions about the sim. The LLM has access to a set of
server-side tools (prim XML generation, mesh import, terrain API, script
injection) and returns results directly to the user via chat or inventory.

## Existing infrastructure

OpenSim already has two viable input paths for this:

### 1. RegionConsoleModule (SimConsoleAsync capability)

The C# source (`RegionConsoleModule.cs`) registers a `SimConsoleAsync` cap
handler that:
- Accepts POST requests with LLSD-encoded command strings
- Checks `IsEstateManagerOrOwner` or `IsGod` before executing
- Routes commands to a `Commands` registry or the main console
- Returns output to the user via EventQueue (`SimConsoleResponse` event)
- Supports `set console on/off` for streaming console output to the client

This is the cleanest path. A Welkin module could register additional commands
under this cap (e.g., `ai <prompt>`) and route them to an LLM backend instead
of the console command parser. Responses come back through the same
`SendConsoleOutput` path.

### 2. Chat channel listener

The `ChatModule` already filters by channel: channel 0 is public, channels 1+
are script-only, `DEBUG_CHANNEL` (2147483647) is debug. A module could listen
on a designated channel (configurable, e.g., channel 9999) for messages from
agents with sufficient permissions. The chat path is simpler for users (just
type `/9999 build me a table`) but less structured than the cap approach.

### Recommended approach

Use the RegionConsoleModule path as primary (more secure, proper permission
checks, structured I/O) and offer the chat channel as an optional convenience
for users whose viewers don't expose the console UI.

## Architecture

```
User (in-world) ──► SimConsoleAsync cap ──► LlmCopilotModule
        │                                         │
        │                                         ▼
        │                                   Permission check
        │                                   (god / estate mgr / config whitelist)
        │                                         │
        │                                         ▼
        │                                   LLM Provider
        │                                   (configurable endpoint)
        │                                         │
        │                                         ▼
        │                                   Tool execution
        │                                   ├── PrimXmlTool (rez prims)
        │                                   ├── MeshImportTool (glTF/Collada)
        │                                   ├── TerrainTool (heightmap edits)
        │                                   ├── ScriptTool (LSL generation)
        │                                   ├── RegionQueryTool (object/agent info)
        │                                   └── InventoryTool (give items)
        │                                         │
        ◄─────────────────────────────────────────┘
        Chat response / inventory delivery / object rez
```

## Configuration

In `welkin.toml`:

```toml
[llm_copilot]
enabled = false
provider = "ollama"           # ollama | claude | openai | custom
endpoint = "http://localhost:11434"
model = "llama3:latest"       # provider-specific model name
api_key = ""                  # empty for local providers
chat_channel = 9999           # 0 = disabled, otherwise channel number
max_tokens_per_request = 4096
rate_limit_per_minute = 10
allowed_tools = ["prim_xml", "terrain", "region_query"]  # operator controls which tools
permission_level = "estate_manager"  # god | estate_manager | whitelist
whitelisted_agents = []       # UUID list, used when permission_level = whitelist

[llm_copilot.tool_limits]
max_prims_per_request = 50
max_mesh_size_mb = 10
terrain_edit_radius_max = 64
```

## Tool definitions

### PrimXmlTool

Generates OpenSim XML2 prim descriptions and rezzes them in-world. The LLM
describes geometry (box, sphere, cylinder, torus, etc.) with position, rotation,
scale, color, and texture parameters. The module serializes to XML2 format and
loads via the scene's object deserialization path (equivalent to
`SceneXmlLoader.DeserializeGroupFromXml2`).

Input: structured description of one or more prims (type, position, size,
rotation, color, texture UUID)
Output: rezzed object(s) in world, ownership assigned to requesting agent

### MeshImportTool

Takes a text description, calls an external mesh generation service (Meshy API,
EmbodiedGen local CLI, Modly, or any configured endpoint), receives glTF/GLB or
Collada output, and imports it through Welkin's mesh upload pipeline. The result
goes to the user's inventory or is rezzed at a specified location.

This tool depends on the modern-avatar-features glTF import pipeline (Phase 4).

Input: text prompt for mesh generation, target location or "inventory"
Output: mesh object in world or inventory

### TerrainTool

Modifies the region heightmap within configured bounds. Can raise, lower,
smooth, or flatten terrain in a specified area. Uses the existing terrain
module's `ITerrainChannel` interface.

Input: operation (raise/lower/smooth/flatten/noise), center position, radius,
intensity
Output: modified terrain, confirmation message

### ScriptTool

Generates LSL scripts based on natural-language descriptions. The LLM writes
the script, the module compiles it through the script engine to verify syntax,
and either attaches it to a specified object or places it in the user's
inventory.

Input: description of desired behavior, target object (optional)
Output: compiled LSL script in inventory or attached to object

### RegionQueryTool

Read-only queries about the region: object count, agent list, parcel info,
physics stats, script stats. Gives the LLM context about the current state of
the world so it can make informed decisions.

Input: query type and parameters
Output: structured data about the region

### InventoryTool

Places generated items (scripts, notecards, objects) into the requesting
agent's inventory in a designated folder.

Input: item to deliver, folder name (created if needed)
Output: inventory item delivered to agent

## Security considerations

1. **Permission model**: only god-level users, estate managers, or explicitly
   whitelisted agent UUIDs can invoke the copilot. The operator configures
   which level in `welkin.toml`.

2. **Tool allowlist**: the operator explicitly enables each tool. A server
   running in a public grid might enable only `region_query` and `prim_xml`
   while keeping `terrain` and `script` disabled.

3. **Rate limiting**: per-user rate limit prevents abuse and runaway API costs.

4. **Resource caps**: max prims per request, max mesh size, max terrain edit
   radius. These prevent a single request from overwhelming the region.

5. **API key isolation**: the server holds the LLM API key, not the user. Users
   never see or configure the key. The operator pre-authorizes the server.

6. **Audit log**: every copilot request and response is logged with the
   requesting agent's UUID, timestamp, and tool invocations. Estate managers
   can review the log.

7. **Sandboxed execution**: generated LSL scripts run through the standard
   script engine with normal permission checks. The copilot cannot bypass
   script permissions.

8. **No escalation**: the copilot module runs with the requesting agent's
   permissions, not elevated permissions. It cannot do anything the user
   couldn't do manually, just faster.

## User experience

**Via console (primary):**
The user opens the viewer's region console (Ctrl+Shift+` in some viewers) and
types:
```
ai build a stone archway, 3 meters tall, at position <128, 128, 25>
```
Response appears in the console output panel.

**Via chat channel (optional):**
```
/9999 build a stone archway, 3 meters tall, here
```
"here" resolves to the user's current position. Response comes back on the same
channel as a whisper from "Welkin Copilot" (a named system object or NPC).

**Multi-turn:**
The module maintains a short conversation context per user (configurable
TTL, default 5 minutes). Saying `ai make it taller` refers to the previous
request's output.

## External service integration

The module is provider-agnostic. The `provider` config selects a built-in
adapter:

- **ollama**: local or remote Ollama instance, no API key needed for local.
  Good for private grids.
- **claude**: Anthropic API. Requires API key. Strongest reasoning.
- **openai**: OpenAI API. Requires API key. Broad model selection.
- **custom**: any OpenAI-compatible endpoint (vLLM, LiteLLM, etc.). Specify
  endpoint and optional API key.

For mesh generation specifically (MeshImportTool), a separate config section
would point to the mesh service:

```toml
[llm_copilot.mesh_provider]
provider = "meshy"            # meshy | embodiedgen | modly | none
endpoint = "https://api.meshy.ai"
api_key = ""
```

## Relationship to existing OpenSim extensions

OpenSim has third-party modules that do pieces of this (e.g., OSSL functions
like `osNpcCreate`, `osSetDynamicTextureURL`). This proposal differs by putting
the LLM on the server side rather than requiring users to write scripts that
call external APIs. The server operator controls access, costs, and which
capabilities are exposed.

## Non-goals for initial version

- No voice input (text only)
- No image/vision input (text descriptions only)
- No autonomous agents (copilot responds to explicit requests only)
- No cross-region operations (scoped to the region the user is in)
- No real-time collaboration (one user per conversation context)

## Future extensions (if the initial version proves useful)

- NPC agents that use the copilot to behave autonomously
- Vision input: user takes a snapshot, copilot analyzes it
- Multi-region operations for grid admins
- Persistent memory per user (the copilot remembers past requests)
- Plugin system for custom tools beyond the built-in set
- WebSocket streaming for long-running operations (progress updates)
