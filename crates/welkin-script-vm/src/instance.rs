// SPDX-License-Identifier: GPL-3.0-or-later
//! Script instance: wraps a Luau VM with identity, event queue and state
//! management for a single LSL script attached to a prim.

use std::collections::VecDeque;
use std::fmt;

use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::sync::Arc;
use parking_lot::RwLock;
use crate::{ScriptState, VmError, compile_source, restore_script_state, serialize_script_state};
use crate::builtins::{self, ScriptContext};

// ---------------------------------------------------------------------------
// ScriptEvent
// ---------------------------------------------------------------------------

/// An event that can be delivered to a script instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptEvent {
    Touch { agent_id: Uuid },
    TouchEnd { agent_id: Uuid },
    TouchUpdate { agent_id: Uuid },
    Timer,
    Listen {
        channel: i32,
        name: String,
        id: Uuid,
        message: String,
    },
    StateEntry,
    OnRez { start_param: i32 },
    Changed { change: u32 },
    Collision { num_detected: i32 },
    CollisionStart { num_detected: i32 },
    CollisionEnd { num_detected: i32 },
    LinkMessage {
        sender_num: i32,
        num: i32,
        str_val: String,
        id: String,
    },
    Sensor { num_detected: i32 },
    NoSensor,
    HttpResponse {
        request_id: String,
        status: i32,
        metadata: Vec<String>,
        body: String,
    },
    HttpRequest {
        request_id: String,
        method: String,
        body: String,
    },
    Money { agent_id: Uuid, amount: i32 },
    RunTimePermissions { permissions: u32 },
    Dataserver {
        request_id: String,
        data: String,
    },
    AtTarget {
        handle: i32,
        target_pos: [f64; 3],
        our_pos: [f64; 3],
    },
    NotAtTarget,
    AtRotTarget {
        handle: i32,
        target_rot: [f64; 4],
        our_rot: [f64; 4],
    },
    NotAtRotTarget,
    Email {
        time: String,
        address: String,
        subject: String,
        message: String,
        num_remaining: i32,
    },
    Control { id: Uuid, level: u32, edge: u32 },
    LandCollisionStart { position: [f64; 3] },
    LandCollision { position: [f64; 3] },
    LandCollisionEnd { position: [f64; 3] },
    Attach { id: Uuid },
    ObjectRez { id: Uuid },
    MovingStart,
    MovingEnd,
    PathUpdate { update_type: i32, reserved: Vec<String> },
    RemoteData {
        event_type: i32,
        channel: Uuid,
        message_id: Uuid,
        sender: String,
        idata: i32,
        sdata: String,
    },
    TransactionResult { id: Uuid, success: i32, data: String },
    Custom {
        name: String,
        args: Vec<serde_json::Value>,
    },
}

impl ScriptEvent {
    pub fn handler_name(&self) -> &str {
        match self {
            ScriptEvent::Touch { .. } => "touch_start",
            ScriptEvent::TouchEnd { .. } => "touch_end",
            ScriptEvent::TouchUpdate { .. } => "touch",
            ScriptEvent::Timer => "timer",
            ScriptEvent::Listen { .. } => "listen",
            ScriptEvent::StateEntry => "state_entry",
            ScriptEvent::OnRez { .. } => "on_rez",
            ScriptEvent::Changed { .. } => "changed",
            ScriptEvent::Collision { .. } => "collision",
            ScriptEvent::CollisionStart { .. } => "collision_start",
            ScriptEvent::CollisionEnd { .. } => "collision_end",
            ScriptEvent::LinkMessage { .. } => "link_message",
            ScriptEvent::Sensor { .. } => "sensor",
            ScriptEvent::NoSensor => "no_sensor",
            ScriptEvent::HttpResponse { .. } => "http_response",
            ScriptEvent::HttpRequest { .. } => "http_request",
            ScriptEvent::Money { .. } => "money",
            ScriptEvent::RunTimePermissions { .. } => "run_time_permissions",
            ScriptEvent::Dataserver { .. } => "dataserver",
            ScriptEvent::AtTarget { .. } => "at_target",
            ScriptEvent::NotAtTarget => "not_at_target",
            ScriptEvent::AtRotTarget { .. } => "at_rot_target",
            ScriptEvent::NotAtRotTarget => "not_at_rot_target",
            ScriptEvent::Email { .. } => "email",
            ScriptEvent::Control { .. } => "control",
            ScriptEvent::LandCollisionStart { .. } => "land_collision_start",
            ScriptEvent::LandCollision { .. } => "land_collision",
            ScriptEvent::LandCollisionEnd { .. } => "land_collision_end",
            ScriptEvent::Attach { .. } => "attach",
            ScriptEvent::ObjectRez { .. } => "object_rez",
            ScriptEvent::MovingStart => "moving_start",
            ScriptEvent::MovingEnd => "moving_end",
            ScriptEvent::PathUpdate { .. } => "path_update",
            ScriptEvent::RemoteData { .. } => "remote_data",
            ScriptEvent::TransactionResult { .. } => "transaction_result",
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
    ctx: Option<Arc<RwLock<ScriptContext>>>,
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
            ctx: None,
        })
    }

    pub fn register_builtins(&mut self, ctx: Arc<RwLock<ScriptContext>>) -> Result<(), VmError> {
        builtins::register_all(&self.vm, &ctx)?;
        self.ctx = Some(ctx);
        Ok(())
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

    fn event_args(&self, event: &ScriptEvent) -> Result<Vec<mlua::Value>, VmError> {
        let mut args: Vec<mlua::Value> = Vec::new();
        match event {
            ScriptEvent::Touch { agent_id }
            | ScriptEvent::TouchEnd { agent_id }
            | ScriptEvent::TouchUpdate { agent_id } => {
                args.push(mlua::Value::String(self.vm.create_string(agent_id.to_string())?));
            }
            ScriptEvent::Timer | ScriptEvent::StateEntry | ScriptEvent::NoSensor => {}
            ScriptEvent::Listen { channel, name, id, message } => {
                args.push(mlua::Value::Integer(i64::from(*channel)));
                args.push(mlua::Value::String(self.vm.create_string(name.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(id.to_string())?));
                args.push(mlua::Value::String(self.vm.create_string(message.as_bytes())?));
            }
            ScriptEvent::OnRez { start_param } => {
                args.push(mlua::Value::Integer(i64::from(*start_param)));
            }
            ScriptEvent::Changed { change } => {
                args.push(mlua::Value::Integer(i64::from(*change)));
            }
            ScriptEvent::Collision { num_detected }
            | ScriptEvent::CollisionStart { num_detected }
            | ScriptEvent::CollisionEnd { num_detected }
            | ScriptEvent::Sensor { num_detected } => {
                args.push(mlua::Value::Integer(i64::from(*num_detected)));
            }
            ScriptEvent::LinkMessage { sender_num, num, str_val, id } => {
                args.push(mlua::Value::Integer(i64::from(*sender_num)));
                args.push(mlua::Value::Integer(i64::from(*num)));
                args.push(mlua::Value::String(self.vm.create_string(str_val.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(id.as_bytes())?));
            }
            ScriptEvent::HttpResponse { request_id, status, metadata, body } => {
                args.push(mlua::Value::String(self.vm.create_string(request_id.as_bytes())?));
                args.push(mlua::Value::Integer(i64::from(*status)));
                let tbl = self.vm.create_table()?;
                for (i, m) in metadata.iter().enumerate() {
                    tbl.set(i + 1, m.as_str())?;
                }
                args.push(mlua::Value::Table(tbl));
                args.push(mlua::Value::String(self.vm.create_string(body.as_bytes())?));
            }
            ScriptEvent::HttpRequest { request_id, method, body } => {
                args.push(mlua::Value::String(self.vm.create_string(request_id.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(method.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(body.as_bytes())?));
            }
            ScriptEvent::Money { agent_id, amount } => {
                args.push(mlua::Value::String(self.vm.create_string(agent_id.to_string())?));
                args.push(mlua::Value::Integer(i64::from(*amount)));
            }
            ScriptEvent::RunTimePermissions { permissions } => {
                args.push(mlua::Value::Integer(i64::from(*permissions)));
            }
            ScriptEvent::Dataserver { request_id, data } => {
                args.push(mlua::Value::String(self.vm.create_string(request_id.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(data.as_bytes())?));
            }
            ScriptEvent::AtTarget { handle, target_pos, our_pos } => {
                args.push(mlua::Value::Integer(i64::from(*handle)));
                let tp = self.vm.create_table()?;
                tp.set("x", target_pos[0])?;
                tp.set("y", target_pos[1])?;
                tp.set("z", target_pos[2])?;
                args.push(mlua::Value::Table(tp));
                let op = self.vm.create_table()?;
                op.set("x", our_pos[0])?;
                op.set("y", our_pos[1])?;
                op.set("z", our_pos[2])?;
                args.push(mlua::Value::Table(op));
            }
            ScriptEvent::NotAtTarget => {}
            ScriptEvent::AtRotTarget { handle, target_rot, our_rot } => {
                args.push(mlua::Value::Integer(i64::from(*handle)));
                let tr = self.vm.create_table()?;
                tr.set("x", target_rot[0])?;
                tr.set("y", target_rot[1])?;
                tr.set("z", target_rot[2])?;
                tr.set("w", target_rot[3])?;
                args.push(mlua::Value::Table(tr));
                let or = self.vm.create_table()?;
                or.set("x", our_rot[0])?;
                or.set("y", our_rot[1])?;
                or.set("z", our_rot[2])?;
                or.set("w", our_rot[3])?;
                args.push(mlua::Value::Table(or));
            }
            ScriptEvent::NotAtRotTarget => {}
            ScriptEvent::Email { time, address, subject, message, num_remaining } => {
                args.push(mlua::Value::String(self.vm.create_string(time.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(address.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(subject.as_bytes())?));
                args.push(mlua::Value::String(self.vm.create_string(message.as_bytes())?));
                args.push(mlua::Value::Integer(i64::from(*num_remaining)));
            }
            ScriptEvent::Control { id, level, edge } => {
                args.push(mlua::Value::String(self.vm.create_string(id.to_string())?));
                args.push(mlua::Value::Integer(i64::from(*level)));
                args.push(mlua::Value::Integer(i64::from(*edge)));
            }
            ScriptEvent::LandCollisionStart { position }
            | ScriptEvent::LandCollision { position }
            | ScriptEvent::LandCollisionEnd { position } => {
                let tbl = self.vm.create_table()?;
                tbl.set("x", position[0])?;
                tbl.set("y", position[1])?;
                tbl.set("z", position[2])?;
                args.push(mlua::Value::Table(tbl));
            }
            ScriptEvent::Attach { id } | ScriptEvent::ObjectRez { id } => {
                args.push(mlua::Value::String(self.vm.create_string(id.to_string())?));
            }
            ScriptEvent::MovingStart | ScriptEvent::MovingEnd => {}
            ScriptEvent::PathUpdate { update_type, reserved } => {
                args.push(mlua::Value::Integer(i64::from(*update_type)));
                let tbl = self.vm.create_table()?;
                for (i, item) in reserved.iter().enumerate() {
                    tbl.set(i + 1, item.as_str())?;
                }
                args.push(mlua::Value::Table(tbl));
            }
            ScriptEvent::RemoteData { event_type, channel, message_id, sender, idata, sdata } => {
                args.push(mlua::Value::Integer(i64::from(*event_type)));
                args.push(mlua::Value::String(self.vm.create_string(channel.to_string())?));
                args.push(mlua::Value::String(self.vm.create_string(message_id.to_string())?));
                args.push(mlua::Value::String(self.vm.create_string(sender.as_bytes())?));
                args.push(mlua::Value::Integer(i64::from(*idata)));
                args.push(mlua::Value::String(self.vm.create_string(sdata.as_bytes())?));
            }
            ScriptEvent::TransactionResult { id, success, data } => {
                args.push(mlua::Value::String(self.vm.create_string(id.to_string())?));
                args.push(mlua::Value::Integer(i64::from(*success)));
                args.push(mlua::Value::String(self.vm.create_string(data.as_bytes())?));
            }
            ScriptEvent::Custom { args: json_args, .. } => {
                use mlua::LuaSerdeExt;
                for a in json_args {
                    args.push(self.vm.to_value(a)?);
                }
            }
        }
        Ok(args)
    }

    fn dispatch_event(&self, event: &ScriptEvent) -> Result<(), VmError> {
        let globals = self.vm.globals();
        let handler_name = event.handler_name();

        // LSL transpiled scripts define __dispatch for state-aware routing.
        if let Ok(dispatch_fn) = globals.get::<mlua::Function>("__dispatch") {
            let mut multi = mlua::MultiValue::new();
            multi.push_back(mlua::Value::String(
                self.vm.create_string(handler_name.as_bytes())?,
            ));
            for v in self.event_args(event)? {
                multi.push_back(v);
            }
            dispatch_fn.call::<()>(multi)?;
            return Ok(());
        }

        // Fallback: direct handler lookup (raw Luau scripts).
        let func: mlua::Function = match globals.get(handler_name) {
            Ok(f) => f,
            Err(mlua::Error::FromLuaConversionError { .. }) => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        let args = self.event_args(event)?;
        let multi = mlua::MultiValue::from_iter(args);
        func.call::<()>(multi)?;

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), VmError> {
        let vm = Lua::new();
        if self.memory_limit > 0 {
            vm.set_memory_limit(self.memory_limit)?;
        }
        vm.load(&self.bytecode).exec()?;
        if let Some(ref ctx) = self.ctx {
            builtins::register_all(&vm, ctx)?;
        }
        self.vm = vm;
        self.state = ScriptInstanceState::Idle;
        self.event_queue.clear();
        self.post_event(ScriptEvent::StateEntry);
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
            ctx: None,
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

    #[test]
    fn dispatch_uses_lsl_dispatch_function() -> Result<(), VmError> {
        let source = r#"
            _dispatched_event = ""
            _dispatched_arg = ""
            function __dispatch(event_name, ...)
                _dispatched_event = event_name
                local args = {...}
                if #args > 0 then
                    _dispatched_arg = tostring(args[1])
                end
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;

        let aid = Uuid::new_v4();
        inst.post_event(ScriptEvent::Touch { agent_id: aid });
        inst.run_next_event()?;
        assert_eq!(inst.state, ScriptInstanceState::Idle);

        let dispatched: String = inst.vm.globals().get("_dispatched_event")?;
        assert_eq!(dispatched, "touch_start");
        let arg: String = inst.vm.globals().get("_dispatched_arg")?;
        assert_eq!(arg, aid.to_string());

        Ok(())
    }

    #[test]
    fn new_event_variants_dispatch() -> Result<(), VmError> {
        let source = r#"
            _last_event = ""
            function touch_end(id) _last_event = "touch_end" end
            function touch(id) _last_event = "touch" end
            function link_message(s, n, str, id) _last_event = "link_message" end
            function sensor(n) _last_event = "sensor" end
            function no_sensor() _last_event = "no_sensor" end
            function http_response(rid, status, meta, body) _last_event = "http_response" end
            function http_request(rid, method, body) _last_event = "http_request" end
            function money(id, amount) _last_event = "money" end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;

        let events: Vec<ScriptEvent> = vec![
            ScriptEvent::TouchEnd { agent_id: Uuid::new_v4() },
            ScriptEvent::TouchUpdate { agent_id: Uuid::new_v4() },
            ScriptEvent::LinkMessage { sender_num: 1, num: 0, str_val: "hi".into(), id: "k".into() },
            ScriptEvent::Sensor { num_detected: 3 },
            ScriptEvent::NoSensor,
            ScriptEvent::HttpResponse { request_id: "r1".into(), status: 200, metadata: vec![], body: "ok".into() },
            ScriptEvent::HttpRequest { request_id: "r2".into(), method: "GET".into(), body: "".into() },
            ScriptEvent::Money { agent_id: Uuid::new_v4(), amount: 100 },
        ];

        let expected = ["touch_end", "touch", "link_message", "sensor", "no_sensor", "http_response", "http_request", "money"];

        for (ev, name) in events.into_iter().zip(expected.iter()) {
            inst.post_event(ev);
            inst.run_next_event()?;
            let last: String = inst.vm.globals().get("_last_event")?;
            assert_eq!(&last, name, "event mismatch");
        }

        Ok(())
    }

    #[test]
    fn reset_clears_state_and_fires_entry() -> Result<(), VmError> {
        let source = r#"
            _counter = 0
            _entered = false
            function timer()
                _counter = _counter + 1
            end
            function state_entry()
                _entered = true
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;

        inst.post_event(ScriptEvent::Timer);
        inst.post_event(ScriptEvent::Timer);
        inst.post_event(ScriptEvent::Timer);
        while inst.has_pending_events() {
            inst.run_next_event()?;
        }

        let count: i32 = inst.vm.globals().get("_counter")?;
        assert_eq!(count, 3);

        inst.reset()?;

        // After reset, counter should be back to 0
        let count: i32 = inst.vm.globals().get("_counter")?;
        assert_eq!(count, 0);

        // reset() posts a StateEntry event
        assert!(inst.has_pending_events());
        inst.run_next_event()?;
        let entered: bool = inst.vm.globals().get("_entered")?;
        assert!(entered);

        Ok(())
    }

    #[test]
    fn event_control() -> Result<(), VmError> {
        let source = r#"
            _ctrl_id = ""
            _ctrl_level = 0
            _ctrl_edge = 0
            function control(id, level, edge)
                _ctrl_id = id
                _ctrl_level = level
                _ctrl_edge = edge
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        let agent = Uuid::new_v4();
        inst.post_event(ScriptEvent::Control { id: agent, level: 5, edge: 3 });
        inst.run_next_event()?;

        let got_id: String = inst.vm.globals().get("_ctrl_id")?;
        assert_eq!(got_id, agent.to_string());
        let got_level: i64 = inst.vm.globals().get("_ctrl_level")?;
        assert_eq!(got_level, 5);
        let got_edge: i64 = inst.vm.globals().get("_ctrl_edge")?;
        assert_eq!(got_edge, 3);
        Ok(())
    }

    #[test]
    fn event_land_collision_start() -> Result<(), VmError> {
        let source = r#"
            _pos_x = 0
            _pos_y = 0
            _pos_z = 0
            function land_collision_start(pos)
                _pos_x = pos.x
                _pos_y = pos.y
                _pos_z = pos.z
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        inst.post_event(ScriptEvent::LandCollisionStart { position: [1.0, 2.5, 3.75] });
        inst.run_next_event()?;

        let x: f64 = inst.vm.globals().get("_pos_x")?;
        let y: f64 = inst.vm.globals().get("_pos_y")?;
        let z: f64 = inst.vm.globals().get("_pos_z")?;
        assert!((x - 1.0).abs() < f64::EPSILON);
        assert!((y - 2.5).abs() < f64::EPSILON);
        assert!((z - 3.75).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn event_land_collision() -> Result<(), VmError> {
        let source = r#"
            _pos_x = 0
            function land_collision(pos)
                _pos_x = pos.x
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        inst.post_event(ScriptEvent::LandCollision { position: [10.0, 0.0, 0.0] });
        inst.run_next_event()?;

        let x: f64 = inst.vm.globals().get("_pos_x")?;
        assert!((x - 10.0).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn event_land_collision_end() -> Result<(), VmError> {
        let source = r#"
            _pos_z = 0
            function land_collision_end(pos)
                _pos_z = pos.z
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        inst.post_event(ScriptEvent::LandCollisionEnd { position: [0.0, 0.0, 55.5] });
        inst.run_next_event()?;

        let z: f64 = inst.vm.globals().get("_pos_z")?;
        assert!((z - 55.5).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn event_attach() -> Result<(), VmError> {
        let source = r#"
            _attach_id = ""
            function attach(id)
                _attach_id = id
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        let avatar = Uuid::new_v4();
        inst.post_event(ScriptEvent::Attach { id: avatar });
        inst.run_next_event()?;

        let got: String = inst.vm.globals().get("_attach_id")?;
        assert_eq!(got, avatar.to_string());
        Ok(())
    }

    #[test]
    fn event_object_rez() -> Result<(), VmError> {
        let source = r#"
            _rez_id = ""
            function object_rez(id)
                _rez_id = id
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        let obj = Uuid::new_v4();
        inst.post_event(ScriptEvent::ObjectRez { id: obj });
        inst.run_next_event()?;

        let got: String = inst.vm.globals().get("_rez_id")?;
        assert_eq!(got, obj.to_string());
        Ok(())
    }

    #[test]
    fn event_moving_start_and_end() -> Result<(), VmError> {
        let source = r#"
            _moving = false
            function moving_start()
                _moving = true
            end
            function moving_end()
                _moving = false
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        inst.post_event(ScriptEvent::MovingStart);
        inst.run_next_event()?;
        let moving: bool = inst.vm.globals().get("_moving")?;
        assert!(moving);

        inst.post_event(ScriptEvent::MovingEnd);
        inst.run_next_event()?;
        let moving: bool = inst.vm.globals().get("_moving")?;
        assert!(!moving);
        Ok(())
    }

    #[test]
    fn event_path_update() -> Result<(), VmError> {
        let source = r#"
            _ptype = 0
            _reserved_len = 0
            function path_update(update_type, reserved)
                _ptype = update_type
                _reserved_len = #reserved
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        inst.post_event(ScriptEvent::PathUpdate {
            update_type: 2,
            reserved: vec!["a".into(), "b".into()],
        });
        inst.run_next_event()?;

        let ptype: i64 = inst.vm.globals().get("_ptype")?;
        assert_eq!(ptype, 2);
        let rlen: i64 = inst.vm.globals().get("_reserved_len")?;
        assert_eq!(rlen, 2);
        Ok(())
    }

    #[test]
    fn event_remote_data() -> Result<(), VmError> {
        let source = r#"
            _rd_etype = 0
            _rd_channel = ""
            _rd_msgid = ""
            _rd_sender = ""
            _rd_idata = 0
            _rd_sdata = ""
            function remote_data(event_type, channel, message_id, sender, idata, sdata)
                _rd_etype = event_type
                _rd_channel = channel
                _rd_msgid = message_id
                _rd_sender = sender
                _rd_idata = idata
                _rd_sdata = sdata
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        let ch = Uuid::new_v4();
        let mid = Uuid::new_v4();
        inst.post_event(ScriptEvent::RemoteData {
            event_type: 1,
            channel: ch,
            message_id: mid,
            sender: "agent".into(),
            idata: 42,
            sdata: "payload".into(),
        });
        inst.run_next_event()?;

        let etype: i64 = inst.vm.globals().get("_rd_etype")?;
        assert_eq!(etype, 1);
        let got_ch: String = inst.vm.globals().get("_rd_channel")?;
        assert_eq!(got_ch, ch.to_string());
        let got_mid: String = inst.vm.globals().get("_rd_msgid")?;
        assert_eq!(got_mid, mid.to_string());
        let got_sender: String = inst.vm.globals().get("_rd_sender")?;
        assert_eq!(got_sender, "agent");
        let got_idata: i64 = inst.vm.globals().get("_rd_idata")?;
        assert_eq!(got_idata, 42);
        let got_sdata: String = inst.vm.globals().get("_rd_sdata")?;
        assert_eq!(got_sdata, "payload");
        Ok(())
    }

    #[test]
    fn event_transaction_result() -> Result<(), VmError> {
        let source = r#"
            _tr_id = ""
            _tr_success = 0
            _tr_data = ""
            function transaction_result(id, success, data)
                _tr_id = id
                _tr_success = success
                _tr_data = data
            end
        "#;

        let mut inst = ScriptInstance::new(Uuid::new_v4(), Uuid::new_v4(), source, 0)?;
        let tid = Uuid::new_v4();
        inst.post_event(ScriptEvent::TransactionResult {
            id: tid,
            success: 1,
            data: "done".into(),
        });
        inst.run_next_event()?;

        let got_id: String = inst.vm.globals().get("_tr_id")?;
        assert_eq!(got_id, tid.to_string());
        let got_success: i64 = inst.vm.globals().get("_tr_success")?;
        assert_eq!(got_success, 1);
        let got_data: String = inst.vm.globals().get("_tr_data")?;
        assert_eq!(got_data, "done");
        Ok(())
    }
}
