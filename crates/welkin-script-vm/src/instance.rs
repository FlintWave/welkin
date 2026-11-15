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
            ScriptEvent::Collision { num_detected } | ScriptEvent::Sensor { num_detected } => {
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
}
