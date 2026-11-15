<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Module development guide

[← Back to docs](index.md) · [Architecture](architecture.md) · [Region simulator core](region-simulator-core-summary.md)

Welkin loads region modules at startup through the `RegionModule` trait
defined in the `welkin-modules` crate. Third-party modules implement this
trait, register with the `ModuleRegistry`, and receive lifecycle callbacks
as regions come online and shut down.

## Licensing boundary

The `welkin-modules` crate is licensed LGPL-3.0-or-later. Code that
implements `RegionModule` and links against the public trait API can use
any license, including proprietary. If you modify the `welkin-modules`
crate itself, those modifications must also be LGPL-3.0-or-later.

The public API boundary consists of these types:

- `RegionModule` (trait)
- `ModuleConfig` (trait)
- `ModuleType` (enum: `Shared`, `NonShared`)
- `ModuleError` (error type)
- `SceneRef` (region handle)
- `SceneEvents` (event bus)
- `ModuleRegistry` (module container)

Everything on the other side of these traits is Welkin's internal
implementation and falls under LGPL. Your module only touches the trait
surface.

## The RegionModule trait

```rust
pub trait RegionModule: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn module_type(&self) -> ModuleType;
    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError>;
    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError>;
    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError>;
    fn close(&mut self) -> Result<(), ModuleError>;
}
```

`ModuleType` controls instantiation:

- `NonShared` creates one instance per region. Use this when your module
  holds per-region state (prim counts, terrain data, local chat).
- `Shared` creates one instance across all regions. Use this for caches,
  user lookups, or anything that should not be duplicated.

## Lifecycle

The registry drives modules through five steps in order:

1. **`initialise(config)`** -- called once. Read your configuration keys
   here. The `ModuleConfig` trait gives typed access to the server's TOML
   (or imported INI) sections.
2. **`add_region(scene)`** -- called for each region the module should
   attach to. Store the `SceneRef`, subscribe to events on
   `scene.events`, and look up sibling modules through `scene.registry`.
3. The module is active. It handles events, responds to requests, and
   interacts with the scene through the `SceneRef`.
4. **`remove_region(scene)`** -- the region is shutting down. Unsubscribe
   from events and release any per-region resources.
5. **`close()`** -- final cleanup. The registry calls `close` in reverse
   registration order (last registered closes first), matching the C#
   OpenSimulator convention.

## Configuration

Modules read configuration through the `ModuleConfig` trait:

```rust
pub trait ModuleConfig: Send + Sync {
    fn get_string(&self, section: &str, key: &str, default: &str) -> String;
    fn get_int(&self, section: &str, key: &str, default: i64) -> i64;
    fn get_bool(&self, section: &str, key: &str, default: bool) -> bool;
    fn get_float(&self, section: &str, key: &str, default: f64) -> f64;
}
```

Section and key names follow the OpenSimulator INI convention. Welkin's
native format is TOML, where INI sections become TOML tables. Environment
variables override any key: set `OPENSIM__Section__Key=value` and the
config layer picks it up before modules see it.

## SceneRef

The `SceneRef` is a lightweight, cloneable handle that modules keep to
interact with the region they are attached to:

```rust
pub struct SceneRef {
    pub region_id: Uuid,
    pub region_name: String,
    pub region_handle: u64,
    pub region_size_x: u32,
    pub region_size_y: u32,
    pub events: Arc<SceneEvents>,
    pub registry: Arc<RwLock<ModuleRegistry>>,
}
```

Use `scene.registry` to look up sibling modules by interface type
(`scene.registry.read().get_module::<ChatModule>()`), the same pattern as
C#'s `RequestModuleInterface<T>()`.

## Module registry

The `ModuleRegistry` owns all loaded modules:

| Method | What it does |
|---|---|
| `register(module)` | Adds a module, returns its index |
| `register_interface::<T>(idx)` | Associates a `TypeId` with that index for lookup |
| `get_module::<T>()` | Finds a module by interface type |
| `initialise_all(config)` | Calls `initialise` on every module in registration order |
| `add_region_all(scene)` | Calls `add_region` on every module |
| `remove_region_all(scene)` | Calls `remove_region` on every module |
| `close_all()` | Calls `close` in reverse registration order |

## Example: a minimal module

```rust
use welkin_modules::{ModuleConfig, ModuleError, ModuleType, RegionModule, SceneRef};

pub struct GreeterModule { message: String }

impl GreeterModule {
    pub fn new() -> Self { Self { message: String::new() } }
}

impl RegionModule for GreeterModule {
    fn name(&self) -> &str { "GreeterModule" }
    fn module_type(&self) -> ModuleType { ModuleType::NonShared }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.message = config.get_string("Greeter", "Message", "Welcome to the region");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, msg = %self.message, "GreeterModule attached");
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> { Ok(()) }
    fn close(&mut self) -> Result<(), ModuleError> { Ok(()) }
}
```

Add to `welkin.toml`:

```toml
[Greeter]
Message = "Welcome to my grid"
```

## Built-in modules

Welkin ships 53 region modules across these categories:

| Category | Examples |
|---|---|
| Chat and messaging | ChatModule, MessageTransferModule, OfflineMessageModule |
| Estate and land | EstateManagementModule, LandManagementModule, DwellModule |
| Permissions | DefaultPermissionsModule |
| Groups and social | GroupsModule, FriendsModule, BasicProfileModule, MuteListModule |
| Economy | MoneyModule (SampleMoneyModule, Gloebit, DTL/NSL proxy) |
| Voice | WebRtcVoiceModule (wolfvoice sidecar) |
| NPCs | NPCModule (20 os* NPC functions) |
| Hypergrid | HGEntityTransferModule, HGFriendsModule, HGInventoryAccessModule |
| World map | WorldMapModule, MapImageModule, MapSearchModule |
| Environment | WindModule, SoundModule, LightShareModule |
| Inventory | InventoryAccessModule, AttachmentsModule, XferModule |
| Framework | UserManagementModule, MonitorModule, FlotsamAssetCache, IPBanModule |

Source for all built-in modules: `crates/welkin-modules/src/`.
