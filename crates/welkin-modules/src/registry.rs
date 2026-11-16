// SPDX-License-Identifier: LGPL-3.0-or-later

//! Module registry and scene reference handle.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::events::SceneEvents;
use crate::module::{ModuleConfig, ModuleError, RegionModule};

// ---------------------------------------------------------------------------
// SceneRef — lightweight handle that modules store
// ---------------------------------------------------------------------------

/// A lightweight, cloneable reference to a scene that modules may keep.
///
/// Carries the region identity, the shared event bus, and a back-pointer to
/// the registry so modules can look up sibling interfaces.
#[derive(Clone)]
pub struct SceneRef {
    pub region_id: Uuid,
    pub region_name: String,
    pub region_handle: u64,
    pub region_size_x: u32,
    pub region_size_y: u32,
    pub events: Arc<SceneEvents>,
    pub registry: Arc<RwLock<ModuleRegistry>>,
    /// Outbound path to connected viewers (null until the region installs one).
    pub egress: crate::egress::SharedEgress,
}

impl SceneRef {
    pub fn new(region_id: Uuid, region_name: String) -> Self {
        let registry = ModuleRegistry::new();
        Self {
            region_id,
            region_name,
            region_handle: 0,
            region_size_x: 256,
            region_size_y: 256,
            events: Arc::new(SceneEvents::new()),
            registry: Arc::new(RwLock::new(registry)),
            egress: crate::egress::null_egress(),
        }
    }
}

impl std::fmt::Debug for SceneRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneRef")
            .field("region_id", &self.region_id)
            .field("region_name", &self.region_name)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// ModuleRegistry
// ---------------------------------------------------------------------------

/// Owns all loaded modules and provides `TypeId`-keyed interface lookup,
/// mirroring C#'s `RequestModuleInterface<T>()` pattern.
pub struct ModuleRegistry {
    modules: Vec<Box<dyn RegionModule>>,
    interfaces: HashMap<TypeId, usize>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            interfaces: HashMap::new(),
        }
    }

    /// Register a module.  Returns its index in the registry.
    pub fn register<M: RegionModule>(&mut self, module: M) -> usize {
        let idx = self.modules.len();
        self.modules.push(Box::new(module));
        idx
    }

    /// Associate a `TypeId` with a previously registered module index so that
    /// [`get_module`](Self::get_module) can find it by interface type.
    pub fn register_interface<I: ?Sized + 'static>(&mut self, module_idx: usize) {
        self.interfaces.insert(TypeId::of::<I>(), module_idx);
    }

    /// Look up a module by the interface type it was registered under.
    pub fn get_module<I: ?Sized + 'static>(&self) -> Option<&dyn RegionModule> {
        let idx = self.interfaces.get(&TypeId::of::<I>())?;
        self.modules.get(*idx).map(|m| m.as_ref())
    }

    /// Number of registered modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Mutably look up a registered module by name.
    pub fn find_module_mut(&mut self, name: &str) -> Option<&mut dyn RegionModule> {
        self.modules
            .iter_mut()
            .find(|m| m.name() == name)
            .map(|m| m.as_mut())
    }

    /// Whether a module with the given name is registered.
    pub fn has_module(&self, name: &str) -> bool {
        self.modules.iter().any(|m| m.name() == name)
    }

    /// Initialise every module in registration order.
    pub fn initialise_all(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        for module in &mut self.modules {
            tracing::debug!(module = module.name(), "initialising module");
            module.initialise(config)?;
        }
        Ok(())
    }

    /// Call `add_region` on every module in registration order.
    pub fn add_region_all(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        for module in &mut self.modules {
            tracing::debug!(module = module.name(), region = %scene.region_name, "adding region");
            module.add_region(scene)?;
        }
        Ok(())
    }

    /// Call `remove_region` on every module in registration order.
    pub fn remove_region_all(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        for module in &mut self.modules {
            tracing::debug!(module = module.name(), region = %scene.region_name, "removing region");
            module.remove_region(scene)?;
        }
        Ok(())
    }

    /// Close every module in **reverse** registration order (last registered
    /// closes first), matching C# behaviour.
    pub fn close_all(&mut self) -> Result<(), ModuleError> {
        for module in self.modules.iter_mut().rev() {
            tracing::debug!(module = module.name(), "closing module");
            module.close()?;
        }
        Ok(())
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
