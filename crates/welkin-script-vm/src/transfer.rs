// SPDX-License-Identifier: GPL-3.0-or-later
//! Region-crossing transfer bundles: serialise a set of script instances
//! (state, pending events, timers) for migration to another region.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::instance::{ScriptEvent, ScriptInstance};
use crate::scheduler::ScriptScheduler;
use crate::{ScriptState, VmError};

// ---------------------------------------------------------------------------
// RegionTransferBundle
// ---------------------------------------------------------------------------

/// A serializable snapshot of one or more script instances, suitable for
/// transferring across region boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionTransferBundle {
    /// `(script_id, prim_id, saved_state)` for each transferred script.
    pub script_states: Vec<(Uuid, Uuid, ScriptState)>,
    /// Pending events that had not yet been processed at transfer time.
    pub pending_events: Vec<(Uuid, Vec<ScriptEvent>)>,
    /// Timer intervals that were active (so they can be re-registered).
    pub timers: Vec<(Uuid, Duration)>,
    /// Region that originated the transfer.
    pub source_region: Uuid,
    /// Unix-epoch timestamp (seconds) when the bundle was created.
    pub timestamp: u64,
}

impl RegionTransferBundle {
    /// Extract a transfer bundle for the listed scripts from a scheduler.
    ///
    /// The scripts are **removed** from the scheduler as part of the
    /// extraction so they don't continue running in the source region.
    pub fn from_scheduler(
        scheduler: &mut ScriptScheduler,
        script_ids: &[Uuid],
        region_id: Uuid,
    ) -> Result<Self, VmError> {
        let mut script_states = Vec::new();
        let mut pending_events = Vec::new();
        let mut timers = Vec::new();

        for &sid in script_ids {
            // Capture timer interval before removal.
            if let Some(interval) = scheduler.timer_interval(&sid) {
                timers.push((sid, interval));
            }

            if let Some(mut inst) = scheduler.remove_script(sid) {
                let state = inst.save_state()?;
                let events = inst.drain_events();
                script_states.push((inst.script_id, inst.prim_id, state));
                if !events.is_empty() {
                    pending_events.push((sid, events));
                }
            }
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            script_states,
            pending_events,
            timers,
            source_region: region_id,
            timestamp,
        })
    }

    /// Inject the scripts carried by this bundle into a destination
    /// scheduler, restoring their state, pending events and timers.
    pub fn apply_to_scheduler(self, scheduler: &mut ScriptScheduler) -> Result<(), VmError> {
        for (script_id, prim_id, state) in &self.script_states {
            let mut inst = ScriptInstance::restore_from(*script_id, *prim_id, state, 0)?;

            // Restore pending events.
            if let Some((_, events)) = self
                .pending_events
                .iter()
                .find(|(sid, _)| sid == script_id)
            {
                inst.restore_events(events.clone());
            }

            scheduler.add_script(inst);
        }

        // Restore timers.
        for (sid, interval) in &self.timers {
            scheduler.set_timer(*sid, *interval);
        }

        Ok(())
    }

    /// Serialize the bundle to JSON bytes.
    pub fn serialize_to_bytes(&self) -> Result<Vec<u8>, VmError> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize a bundle from JSON bytes.
    pub fn deserialize_from_bytes(bytes: &[u8]) -> Result<Self, VmError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT_SRC: &str = r#"
        _counter = 0
        function timer()
            _counter = _counter + 1
        end
        function touch_start(agent_id)
            _counter = _counter + 10
        end
    "#;

    #[test]
    fn round_trip_transfer() -> Result<(), VmError> {
        let region_a = Uuid::new_v4();
        let _region_b = Uuid::new_v4();

        // --- Source region scheduler --------------------------------------
        let mut sched_a = ScriptScheduler::new(10_000);

        let sid1 = Uuid::new_v4();
        let pid1 = Uuid::new_v4();
        let mut inst1 = ScriptInstance::new(sid1, pid1, SCRIPT_SRC, 0)?;
        inst1.track_global("_counter");
        // Give it a pending event that won't be processed before transfer.
        inst1.post_event(ScriptEvent::Touch {
            agent_id: Uuid::new_v4(),
        });
        sched_a.add_script(inst1);
        sched_a.set_timer(sid1, Duration::from_secs(5));

        let sid2 = Uuid::new_v4();
        let pid2 = Uuid::new_v4();
        let inst2 = ScriptInstance::new(sid2, pid2, SCRIPT_SRC, 0)?;
        sched_a.add_script(inst2);

        assert_eq!(sched_a.script_count(), 2);

        // --- Extract bundle for sid1 only ---------------------------------
        let bundle =
            RegionTransferBundle::from_scheduler(&mut sched_a, &[sid1], region_a)?;

        // sid1 removed from source scheduler.
        assert_eq!(sched_a.script_count(), 1);
        assert!(sched_a.get_script(&sid1).is_none());
        assert!(sched_a.get_script(&sid2).is_some());

        // Bundle contents.
        assert_eq!(bundle.script_states.len(), 1);
        assert_eq!(bundle.script_states[0].0, sid1);
        assert_eq!(bundle.script_states[0].1, pid1);
        assert_eq!(bundle.pending_events.len(), 1);
        assert_eq!(bundle.timers.len(), 1);
        assert_eq!(bundle.source_region, region_a);

        // --- Serialize / deserialize roundtrip ----------------------------
        let bytes = bundle.serialize_to_bytes()?;
        let bundle2 = RegionTransferBundle::deserialize_from_bytes(&bytes)?;
        assert_eq!(bundle2.script_states.len(), 1);
        assert_eq!(bundle2.source_region, region_a);

        // --- Destination region scheduler ---------------------------------
        let mut sched_b = ScriptScheduler::new(10_000);
        bundle2.apply_to_scheduler(&mut sched_b)?;

        assert_eq!(sched_b.script_count(), 1);
        let restored = sched_b.get_script(&sid1).unwrap();
        assert_eq!(restored.script_id, sid1);
        assert_eq!(restored.prim_id, pid1);
        // The pending touch event should have been restored.
        assert!(restored.has_pending_events());
        // Timer should be restored.
        assert!(sched_b.timer_interval(&sid1).is_some());

        // Run the pending event in the destination scheduler.
        let errors = sched_b.tick();
        assert!(errors.is_empty(), "tick errors: {errors:?}");
        assert!(!sched_b.get_script(&sid1).unwrap().has_pending_events());

        Ok(())
    }

    #[test]
    fn empty_bundle() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);
        let bundle = RegionTransferBundle::from_scheduler(
            &mut sched,
            &[Uuid::new_v4()], // non-existent id
            Uuid::new_v4(),
        )?;

        assert!(bundle.script_states.is_empty());
        assert!(bundle.pending_events.is_empty());
        assert!(bundle.timers.is_empty());

        // Applying an empty bundle is a no-op.
        let mut sched2 = ScriptScheduler::new(10_000);
        bundle.apply_to_scheduler(&mut sched2)?;
        assert_eq!(sched2.script_count(), 0);

        Ok(())
    }

    #[test]
    fn serialization_roundtrip_bytes() -> Result<(), VmError> {
        let mut sched = ScriptScheduler::new(10_000);

        let sid = Uuid::new_v4();
        let inst = ScriptInstance::new(sid, Uuid::new_v4(), SCRIPT_SRC, 0)?;
        sched.add_script(inst);

        let bundle = RegionTransferBundle::from_scheduler(
            &mut sched,
            &[sid],
            Uuid::new_v4(),
        )?;

        let bytes = bundle.serialize_to_bytes()?;
        assert!(!bytes.is_empty());

        let restored = RegionTransferBundle::deserialize_from_bytes(&bytes)?;
        assert_eq!(restored.script_states.len(), 1);
        assert_eq!(restored.script_states[0].0, sid);

        Ok(())
    }
}
