// SPDX-License-Identifier: GPL-3.0-or-later
//! Script instance: wraps a Luau VM with identity, event queue and state
//! management for a single LSL script attached to a prim.

use std::collections::VecDeque;
use std::fmt;

use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ScriptState, VmError, compile_source, restore_script_state, serialize_script_state};

// ---------------------------------------------------------------------------
// ScriptEvent
// ---------------------------------------------------------------------------

/// An event that can be delivered to a script instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptEvent {
    /// A touch event (touch_start handler).
    Touch { agent_id: Uuid },
    /// Timer tick.
    Timer,
    /// Listen event from llListen.
    Listen {
        channel: i32,
        name: String,
        id: Uuid,
        message: String,
    },
    /// Entry into current state (state_entry handler).
    StateEntry,
    /// Object rezzed (on_rez handler).
    OnRez { start_param: i32 },
    /// Something changed (changed handler).
    Changed { change: u32 },
    /// Collision event.
    Collision { num_detected: i32 },
    /// Catch-all for events not yet given first-class variants.
    Custom {
        name: String,
        args: Vec<serde_json::Value>,
    },
}

impl ScriptEvent {
    /// Returns the Luau handler function name for this event.
    pub fn handler_name(&self) -> &str {
        match self {
            ScriptEvent::Touch { .. } => "touch_start",
            ScriptEvent::Timer => "timer",
            ScriptEvent::Listen { .. } => "listen",
            ScriptEvent::StateEntry => "state_entry",
            ScriptEvent::OnRez { .. } => "on_rez",
            ScriptEvent::Changed { .. } => "changed",
            ScriptEvent::Collision { .. } => "collision",
            ScriptEvent::Custom { name, .. } => name.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// ScriptInstanceState
// ---------------------------------------------------------------------------

/// Runtime state of a script instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptInstanceState {
    Idle,
    Running,
    Suspended,
    Stopped,
    Error(String),
}

impl fmt::Display for ScriptInstanceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Error(e) => write!(f, "Error({e})"),
        }
    }
}

// ---------------------------------------------------------------------------
// ScriptInstance
// ---------------------------------------------------------------------------

/// A single script instance attached to a prim in the region.
///
/// Holds a sandboxed Luau VM, compiled bytecode, an inbound event queue and
/// bookkeeping state.  The scheduler drives execution by calling
/// [`run_next_event`](Self::run_next_event).
pub struct ScriptInstance {
    /// The Luau VM for this script.
    vm: Lua,
    /// Unique id of the script asset.
    pub script_id: Uuid,
    /// Unique id of the prim this script is attached to.
    pub prim_id: Uuid,
    /// Compiled Luau bytecode.
    bytecode: Vec<u8>,
    /// Current lifecycle state.
    pub state: ScriptInstanceState,
    /// Memory limit applied to the VM (0 = unlimited).
    pub memory_limit: usize,
    /// Pending inbound events, processed FIFO.
    event_queue: VecDeque<ScriptEvent>,
    /// Names of globals the script has declared (tracked for serialization).
    tracked_globals: Vec<String>,
}

impl ScriptInstance {
    /// Compile `source` and create a new script instance.
    ///
    /// The VM is created *without* sandboxing so that the script can define
    /// its handler functions as globals.  The bytecode is executed once to
    /// register those handlers.
    pub fn new(
        script_id: Uuid,
        prim_id: Uuid,
        source: &str,
        memory_limit: usize,
    ) -> Result<Self, VmError> {
        let bytecode = compile_source(source)?;
        let vm = Lua::new();
        if memory_limit > 0 {
            vm.set_memory_limit(memory_limit)?;
        }

        // Execute the bytecode so handler functions are registered as globals.
        vm.load(&bytecode).exec()?;

        Ok(Self {
            vm,
            script_id,
            prim_id,
            bytecode,
            state: ScriptInstanceState::Idle,
            memory_limit,
            event_queue: VecDeque::new(),
            tracked_globals: Vec::new(),
        })
    }

    /// Enqueue an event for later processing.
    pub fn post_event(&mut self, event: ScriptEvent) {
        if self.state != ScriptInstanceState::Stopped {
            self.event_queue.push_back(event);
        }
    }

    /// Returns `true` if there are events waiting to be processed.
    pub fn has_pending_events(&self) -> bool {
        !self.event_queue.is_empty()
    }

    /// Returns the number of pending events.
    pub fn pending_event_count(&self) -> usize {
        self.event_queue.len()
    }

    /// Drain and return all pending events (used by transfer bundles).
    pub fn drain_events(&mut self) -> Vec<ScriptEvent> {
        self.event_queue.drain(..).collect()
    }

    /// Push events back (used when restoring from a transfer bundle).
    pub fn restore_events(&mut self, events: Vec<ScriptEvent>) {
        for e in events {
            self.event_queue.push_back(e);
        }
    }

    /// Pop the next event and invoke the corresponding Luau handler function.
    ///
    /// Returns `Ok(())` if the handler ran (or was missing — a no-op).
    /// Returns `Err` on VM errors.
    pub fn run_next_event(&mut self) -> Result<(), VmError> {
        let event = match self.event_queue.pop_front() {
            Some(e) => e,
            None => return Ok(()),
        };

        self.state = ScriptInstanceState::Running;

        let result = self.dispatch_event(&event);

        match &result {
            Ok(()) => {
                self.state = ScriptInstanceState::Idle;
            }
            Err(e) => {
                self.state = ScriptInstanceState::Error(e.to_string());
            }
        }

        result
    }

    /// Internal: call the Luau handler for `event`, passing appropriate args.
    fn dispatch_event(&self, event: &ScriptEvent) -> Result<(), VmError> {
        let globals = self.vm.globals();
        let handler_name = event.handler_name();

        // If the handler doesn't exist, silently ignore the event (matches
        // LSL semantics — scripts without a timer() handler just ignore
        // timer events).
        let func: mlua::Function = match globals.get(handler_name) {
            Ok(f) => f,
            Err(mlua::Error::FromLuaConversionError { .. }) => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        match event {
            ScriptEvent::Touch { agent_id } => {
                func.call::<()>(agent_id.to_string())?;
            }
            ScriptEvent::Timer => {
                func.call::<()>(())?;
            }
            ScriptEvent::Listen {
                channel,
                name,
                id,
                message,
            } => {
                func.call::<()>((*channel, name.as_str(), id.to_string(), message.as_str()))?;
            }
            ScriptEvent::StateEntry => {
                func.call::<()>(())?;
            }
            ScriptEvent::OnRez { start_param } => {
                func.call::<()>(*start_param)?;
            }
            ScriptEvent::Changed { change } => {
                func.call::<()>(*change)?;
            }
            ScriptEvent::Collision { num_detected } => {
                func.call::<()>(*num_detected)?;
            }
            ScriptEvent::Custom { args, .. } => {
                // Convert JSON args to Lua values.
                use mlua::LuaSerdeExt;
                let lua_args: Vec<mlua::Value> = args
                    .iter()
                    .map(|a| self.vm.to_value(a))
                    .collect::<Result<_, _>>()?;
                let multi = mlua::MultiValue::from_iter(lua_args);
                func.call::<()>(multi)?;
            }
        }

        Ok(())
    }

    /// Register a global name to be tracked for serialization.
    pub fn track_global(&mut self, name: &str) {
        if !self.tracked_globals.iter().any(|g| g == name) {
            self.tracked_globals.push(name.to_string());
        }
    }

    /// Serialize the script's bytecode and tracked globals into a
    /// [`ScriptState`] that can be persisted or transferred.
    pub fn save_state(&self) -> Result<ScriptState, VmError> {
        let keys: Vec<&str> = self.tracked_globals.iter().map(|s| s.as_str()).collect();
        serialize_script_state(&self.vm, self.bytecode.clone(), &keys)
    }

    /// Restore a `ScriptInstance` from a previously saved [`ScriptState`].
    pub fn restore_from(
        script_id: Uuid,
        prim_id: Uuid,
        state: &ScriptState,
        memory_limit: usize,
    ) -> Result<Self, VmError> {
        let vm = restore_script_state(state)?;
        if memory_limit > 0 {
            vm.set_memory_limit(memory_limit)?;
        }

        // Re-execute bytecode so handler functions are registered.
        vm.load(&state.bytecode).exec()?;

        Ok(Self {
            vm,
            script_id,
            prim_id,
            bytecode: state.bytecode.clone(),
            state: ScriptInstanceState::Idle,
            memory_limit,
            event_queue: VecDeque::new(),
            tracked_globals: Vec::new(),
        })
    }

    /// Set the interrupt callback on the underlying VM.
    ///
    /// The scheduler uses this to enforce instruction-count limits per time
    /// slice.
    pub fn set_interrupt<F>(&self, f: F)
    where
        F: Fn() -> Result<mlua::VmState, mlua::Error> + Send + 'static,
    {
        self.vm.set_interrupt(move |_| f());
    }

    /// Remove the interrupt callback.
    pub fn remove_interrupt(&self) {
        self.vm.remove_interrupt();
    }
}

// Debug can't auto-derive because Lua isn't Debug.
impl fmt::Debug for ScriptInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptInstance")
            .field("script_id", &self.script_id)
            .field("prim_id", &self.prim_id)
            .field("state", &self.state)
            .field("memory_limit", &self.memory_limit)
            .field("pending_events", &self.event_queue.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const HANDLER_SOURCE: &str = r#"
        _touches = 0
        _timer_fires = 0
        _last_message = ""
        _state_entered = false
        _rez_param = -1
        _change_val = 0

        function touch_start(agent_id)
            _touches = _touches + 1
        end

        function timer()
            _timer_fires = _timer_fires + 1
        end

        function listen(channel, name, id, message)
            _last_message = message
        end

        function state_entry()
            _state_entered = true
        end

        function on_rez(start_param)
            _rez_param = start_param
        end

        function changed(change)
            _change_val = change
        end
    "#;

    #[test]
    fn create_instance_and_run_touch() -> Result<(), VmError> {
        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), HANDLER_SOURCE, 0)?;

        assert!(!inst.has_pending_events());
        assert_eq!(inst.state, ScriptInstanceState::Idle);

        inst.post_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });
        assert!(inst.has_pending_events());

        inst.run_next_event()?;
        assert!(!inst.has_pending_events());
        assert_eq!(inst.state, ScriptInstanceState::Idle);

        Ok(())
    }

    #[test]
    fn run_multiple_event_types() -> Result<(), VmError> {
        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), HANDLER_SOURCE, 0)?;

        inst.post_event(ScriptEvent::StateEntry);
        inst.post_event(ScriptEvent::Timer);
        inst.post_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });
        inst.post_event(ScriptEvent::Listen {
            channel: 0,
            name: "Tester".into(),
            id: Uuid::new_v4(),
            message: "hello world".into(),
        });
        inst.post_event(ScriptEvent::OnRez { start_param: 42 });
        inst.post_event(ScriptEvent::Changed { change: 7 });

        assert_eq!(inst.pending_event_count(), 6);

        // Run all events
        while inst.has_pending_events() {
            inst.run_next_event()?;
        }

        assert_eq!(inst.state, ScriptInstanceState::Idle);
        Ok(())
    }

    #[test]
    fn missing_handler_is_noop() -> Result<(), VmError> {
        // Script with NO handlers defined
        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), "-- empty script", 0)?;

        inst.post_event(ScriptEvent::Timer);
        inst.run_next_event()?;
        assert_eq!(inst.state, ScriptInstanceState::Idle);
        Ok(())
    }

    #[test]
    fn save_and_restore_state() -> Result<(), VmError> {
        let sid = Uuid::new_v4();
        let pid = Uuid::new_v4();

        let mut inst = ScriptInstance::new(sid, pid, HANDLER_SOURCE, 0)?;
        inst.track_global("_touches");
        inst.track_global("_timer_fires");

        // Fire some events to mutate globals
        inst.post_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });
        inst.post_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });
        inst.post_event(ScriptEvent::Timer);
        while inst.has_pending_events() {
            inst.run_next_event()?;
        }

        // Save
        let saved = inst.save_state()?;

        // Restore into a new instance
        let inst2 = ScriptInstance::restore_from(sid, pid, &saved, 0)?;
        assert_eq!(inst2.state, ScriptInstanceState::Idle);
        assert_eq!(inst2.script_id, sid);
        assert_eq!(inst2.prim_id, pid);

        Ok(())
    }

    #[test]
    fn stopped_instance_ignores_events() -> Result<(), VmError> {
        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), HANDLER_SOURCE, 0)?;

        inst.state = ScriptInstanceState::Stopped;
        inst.post_event(ScriptEvent::Timer);
        assert!(!inst.has_pending_events());
        Ok(())
    }

    #[test]
    fn custom_event_dispatches() -> Result<(), VmError> {
        let source = r#"
            _custom_result = 0
            function my_custom(a, b)
                _custom_result = a + b
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;

        inst.post_event(ScriptEvent::Custom {
            name: "my_custom".into(),
            args: vec![serde_json::Value::from(10), serde_json::Value::from(32)],
        });

        inst.run_next_event()?;
        assert_eq!(inst.state, ScriptInstanceState::Idle);
        Ok(())
    }
}
