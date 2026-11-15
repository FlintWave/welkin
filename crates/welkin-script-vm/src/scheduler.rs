// SPDX-License-Identifier: GPL-3.0-or-later
//! Script scheduler: manages a collection of script instances, fires timers,
//! and round-robins event execution with per-slice instruction limits.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::VmError;
use crate::instance::{ScriptEvent, ScriptInstance, ScriptInstanceState};

// ---------------------------------------------------------------------------
// TimerEntry
// ---------------------------------------------------------------------------

/// A pending timer: fires `ScriptEvent::Timer` at `next_fire` and repeats
/// every `interval`.
#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub script_id: Uuid,
    pub next_fire: Instant,
    pub interval: Duration,
}

// BinaryHeap is a max-heap; we want the *earliest* fire time first, so we
// reverse the ordering.
impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.next_fire == other.next_fire
    }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: smallest next_fire should come out first.
        other.next_fire.cmp(&self.next_fire)
    }
}

// ---------------------------------------------------------------------------
// ScriptScheduler
// ---------------------------------------------------------------------------

/// Manages multiple [`ScriptInstance`]s, their timers, and round-robin
/// execution of pending events.
pub struct ScriptScheduler {
    /// All managed script instances, keyed by script_id.
    scripts: HashMap<Uuid, ScriptInstance>,
    /// Min-heap of timer entries (earliest-first via reversed Ord).
    timer_heap: BinaryHeap<TimerEntry>,
    /// Maximum number of interrupt callbacks per event dispatch before the
    /// VM is yielded.  This approximates an instruction budget.
    max_instructions_per_slice: usize,
}

impl ScriptScheduler {
    /// Create a new scheduler.
    ///
    /// `max_instructions` controls how many interrupt callbacks each script
    /// is allowed per event dispatch before being cut short.
    pub fn new(max_instructions: usize) -> Self {
        Self {
            scripts: HashMap::new(),
            timer_heap: BinaryHeap::new(),
            max_instructions_per_slice: max_instructions,
        }
    }

    /// Add a script instance to the scheduler.
    pub fn add_script(&mut self, instance: ScriptInstance) {
        self.scripts.insert(instance.script_id, instance);
    }

    /// Remove a script and its timer (if any).
    pub fn remove_script(&mut self, script_id: Uuid) -> Option<ScriptInstance> {
        // Remove from timer heap by rebuilding without the matching entry.
        let old_heap = std::mem::take(&mut self.timer_heap);
        self.timer_heap = old_heap
            .into_iter()
            .filter(|t| t.script_id != script_id)
            .collect();
        self.scripts.remove(&script_id)
    }

    /// Set (or replace) a repeating timer for a script.
    pub fn set_timer(&mut self, script_id: Uuid, interval: Duration) {
        // Remove any existing timer for this script first.
        let old_heap = std::mem::take(&mut self.timer_heap);
        self.timer_heap = old_heap
            .into_iter()
            .filter(|t| t.script_id != script_id)
            .collect();

        self.timer_heap.push(TimerEntry {
            script_id,
            next_fire: Instant::now() + interval,
            interval,
        });
    }

    /// Cancel the timer for a script.
    pub fn clear_timer(&mut self, script_id: Uuid) {
        let old_heap = std::mem::take(&mut self.timer_heap);
        self.timer_heap = old_heap
            .into_iter()
            .filter(|t| t.script_id != script_id)
            .collect();
    }

    /// Returns an immutable reference to a script instance.
    pub fn get_script(&self, script_id: &Uuid) -> Option<&ScriptInstance> {
        self.scripts.get(script_id)
    }

    /// Returns a mutable reference to a script instance.
    pub fn get_script_mut(&mut self, script_id: &Uuid) -> Option<&mut ScriptInstance> {
        self.scripts.get_mut(script_id)
    }

    /// How many scripts are currently managed.
    pub fn script_count(&self) -> usize {
        self.scripts.len()
    }

    /// Iterate over all managed script instances.
    pub fn all_scripts(&self) -> impl Iterator<Item = &ScriptInstance> {
        self.scripts.values()
    }

    /// Return the IDs of all managed scripts.
    pub fn all_script_ids(&self) -> Vec<Uuid> {
        self.scripts.keys().copied().collect()
    }

    /// Returns the timer interval for a script, if one is set.
    pub fn timer_interval(&self, script_id: &Uuid) -> Option<Duration> {
        self.timer_heap
            .iter()
            .find(|t| &t.script_id == script_id)
            .map(|t| t.interval)
    }

    /// Post an event to a single script.
    pub fn post_event(&mut self, script_id: Uuid, event: ScriptEvent) {
        if let Some(inst) = self.scripts.get_mut(&script_id) {
            inst.post_event(event);
        }
    }

    /// Post an event to every script attached to a given prim.
    pub fn post_event_to_prim(&mut self, prim_id: Uuid, event: ScriptEvent) {
        for inst in self.scripts.values_mut() {
            if inst.prim_id == prim_id {
                inst.post_event(event.clone());
            }
        }
    }

    /// Post an event to every managed script.
    pub fn broadcast_event(&mut self, event: ScriptEvent) {
        for inst in self.scripts.values_mut() {
            inst.post_event(event.clone());
        }
    }

    pub fn scripts_on_prim(&self, prim_id: &Uuid) -> Vec<Uuid> {
        self.scripts
            .iter()
            .filter(|(_, inst)| &inst.prim_id == prim_id)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn reset_script(&mut self, script_id: &Uuid) -> Result<(), VmError> {
        if let Some(instance) = self.scripts.get_mut(script_id) {
            instance.reset()?;
        }
        Ok(())
    }

    pub fn reset_prim_scripts(&mut self, prim_id: &Uuid) -> Vec<VmError> {
        let ids: Vec<Uuid> = self.scripts_on_prim(prim_id);
        let mut errors = Vec::new();
        for id in ids {
            if let Some(inst) = self.scripts.get_mut(&id)
                && let Err(e) = inst.reset()
            {
                errors.push(e);
            }
        }
        errors
    }

    /// Advance the scheduler by one tick:
    ///
    /// 1. Fire any timers whose `next_fire` is at or before `now`.
    /// 2. Round-robin through scripts that have pending events, running one
    ///    event per script.  Each event dispatch is bounded by the
    ///    instruction limit via `set_interrupt`.
    pub fn tick(&mut self) -> Vec<(Uuid, VmError)> {
        let now = Instant::now();
        let mut errors: Vec<(Uuid, VmError)> = Vec::new();

        // --- 1. Fire due timers -------------------------------------------
        let mut refired: Vec<TimerEntry> = Vec::new();

        while let Some(entry) = self.timer_heap.peek() {
            if entry.next_fire > now {
                break;
            }
            let mut entry = self.timer_heap.pop().unwrap();

            if let Some(inst) = self.scripts.get_mut(&entry.script_id) {
                inst.post_event(ScriptEvent::Timer);
            }

            // Reschedule.
            entry.next_fire = now + entry.interval;
            refired.push(entry);
        }

        for entry in refired {
            self.timer_heap.push(entry);
        }

        // --- 2. Round-robin event dispatch --------------------------------
        let ids: Vec<Uuid> = self.scripts.keys().copied().collect();

        let limit = self.max_instructions_per_slice;

        for id in ids {
            let inst = match self.scripts.get_mut(&id) {
                Some(i) => i,
                None => continue,
            };

            if !inst.has_pending_events() {
                continue;
            }

            if inst.state == ScriptInstanceState::Stopped {
                continue;
            }

            // Set an instruction counter interrupt.
            let counter = Arc::new(AtomicU64::new(0));
            let counter_clone = counter.clone();
            let limit_u64 = limit as u64;

            inst.set_interrupt(move || {
                if counter_clone.fetch_add(1, AtomicOrdering::Relaxed) >= limit_u64 {
                    Err(mlua::Error::runtime("instruction limit exceeded"))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            });

            let result = inst.run_next_event();

            inst.remove_interrupt();

            if let Err(e) = result {
                errors.push((id, e));
            }
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TIMER_SCRIPT: &str = r#"
        _count = 0
        function timer()
            _count = _count + 1
        end
    "#;

    const TOUCH_SCRIPT: &str = r#"
        _touched = false
        function touch_start(agent_id)
            _touched = true
        end
    "#;

    #[test]
    fn add_remove_scripts() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);
        assert_eq!(sched.script_count(), 0);

        let id1 = Uuid::new_v4();
        let inst1 = ScriptInstance::new(id1, Uuid::new_v4(), TIMER_SCRIPT, 0)?;
        sched.add_script(inst1);
        assert_eq!(sched.script_count(), 1);

        let id2 = Uuid::new_v4();
        let inst2 = ScriptInstance::new(id2, Uuid::new_v4(), TOUCH_SCRIPT, 0)?;
        sched.add_script(inst2);
        assert_eq!(sched.script_count(), 2);

        sched.remove_script(id1);
        assert_eq!(sched.script_count(), 1);
        assert!(sched.get_script(&id2).is_some());
        assert!(sched.get_script(&id1).is_none());

        Ok(())
    }

    #[test]
    fn broadcast_event_reaches_all() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);

        let id1 = Uuid::new_v4();
        let inst1 = ScriptInstance::new(id1, Uuid::new_v4(), TOUCH_SCRIPT, 0)?;
        sched.add_script(inst1);

        let id2 = Uuid::new_v4();
        let inst2 = ScriptInstance::new(id2, Uuid::new_v4(), TOUCH_SCRIPT, 0)?;
        sched.add_script(inst2);

        sched.broadcast_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });

        assert!(sched.get_script(&id1).unwrap().has_pending_events());
        assert!(sched.get_script(&id2).unwrap().has_pending_events());

        let errors = sched.tick();
        assert!(errors.is_empty(), "tick errors: {errors:?}");

        assert!(!sched.get_script(&id1).unwrap().has_pending_events());
        assert!(!sched.get_script(&id2).unwrap().has_pending_events());

        Ok(())
    }

    #[test]
    fn tick_dispatches_events() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);

        let id = Uuid::new_v4();
        let inst = ScriptInstance::new(id, Uuid::new_v4(), TIMER_SCRIPT, 0)?;
        sched.add_script(inst);

        sched.post_event(id, ScriptEvent::Timer);
        sched.post_event(id, ScriptEvent::Timer);

        // First tick runs one event.
        let errors = sched.tick();
        assert!(errors.is_empty());
        // Second event still pending.
        assert!(sched.get_script(&id).unwrap().has_pending_events());

        // Second tick runs the remaining event.
        let errors = sched.tick();
        assert!(errors.is_empty());
        assert!(!sched.get_script(&id).unwrap().has_pending_events());

        Ok(())
    }

    #[test]
    fn set_and_clear_timer() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);

        let id = Uuid::new_v4();
        let inst = ScriptInstance::new(id, Uuid::new_v4(), TIMER_SCRIPT, 0)?;
        sched.add_script(inst);

        // Set a timer with zero interval so it fires immediately.
        sched.set_timer(id, Duration::from_millis(0));

        // A tick should fire the timer and post a Timer event.
        std::thread::sleep(Duration::from_millis(1));
        let errors = sched.tick();
        assert!(errors.is_empty());

        // The timer event was both posted and run in the same tick.
        // Verify the timer is still registered (it repeats).
        assert!(sched.timer_interval(&id).is_some());

        // Clear the timer.
        sched.clear_timer(id);
        assert!(sched.timer_interval(&id).is_none());

        Ok(())
    }

    #[test]
    fn instruction_limit_produces_error() -> Result<(), VmError> {
        let source = r#"
            function timer()
                local x = 0
                while true do x = x + 1 end
            end
        "#;

        let mut sched = ScriptScheduler::new(500); // very low limit

        let id = Uuid::new_v4();
        let inst = ScriptInstance::new(id, Uuid::new_v4(), source, 0)?;
        sched.add_script(inst);

        sched.post_event(id, ScriptEvent::Timer);

        let errors = sched.tick();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, id);

        // The script should be in error state.
        let inst = sched.get_script(&id).unwrap();
        assert!(matches!(inst.state, ScriptInstanceState::Error(_)));

        Ok(())
    }

    #[test]
    fn post_event_to_prim_targets_prim() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);

        let prim_a = Uuid::new_v4();
        let prim_b = Uuid::new_v4();

        let id1 = Uuid::new_v4();
        let inst1 = ScriptInstance::new(id1, prim_a, TOUCH_SCRIPT, 0)?;
        sched.add_script(inst1);

        let id2 = Uuid::new_v4();
        let inst2 = ScriptInstance::new(id2, prim_a, TIMER_SCRIPT, 0)?;
        sched.add_script(inst2);

        let id3 = Uuid::new_v4();
        let inst3 = ScriptInstance::new(id3, prim_b, TOUCH_SCRIPT, 0)?;
        sched.add_script(inst3);

        sched.post_event_to_prim(prim_a, ScriptEvent::Touch { agent_id: Uuid::new_v4() });

        assert!(sched.get_script(&id1).unwrap().has_pending_events());
        assert!(sched.get_script(&id2).unwrap().has_pending_events());
        assert!(!sched.get_script(&id3).unwrap().has_pending_events());

        Ok(())
    }

    #[test]
    fn scripts_on_prim_returns_correct_ids() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);
        let prim = Uuid::new_v4();

        let id1 = Uuid::new_v4();
        sched.add_script(ScriptInstance::new(id1, prim, TIMER_SCRIPT, 0)?);

        let id2 = Uuid::new_v4();
        sched.add_script(ScriptInstance::new(id2, prim, TOUCH_SCRIPT, 0)?);

        let id3 = Uuid::new_v4();
        sched.add_script(ScriptInstance::new(id3, Uuid::new_v4(), TIMER_SCRIPT, 0)?);

        let mut ids = sched.scripts_on_prim(&prim);
        ids.sort();
        let mut expected = vec![id1, id2];
        expected.sort();
        assert_eq!(ids, expected);

        Ok(())
    }

    #[test]
    fn reset_script_clears_and_fires_entry() -> Result<(), VmError> {
        let source = r#"
            _count = 0
            function timer() _count = _count + 1 end
            function state_entry() end
        "#;

        let mut sched = ScriptScheduler::new(10_000);
        let id = Uuid::new_v4();
        let inst = ScriptInstance::new(id, Uuid::new_v4(), source, 0)?;
        sched.add_script(inst);

        sched.post_event(id, ScriptEvent::Timer);
        sched.tick();

        sched.reset_script(&id)?;

        // Reset posts state_entry
        assert!(sched.get_script(&id).unwrap().has_pending_events());

        Ok(())
    }
}
