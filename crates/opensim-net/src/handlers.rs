// SPDX-License-Identifier: GPL-3.0-or-later

//! Core protocol handlers: stub implementations that decode each viewer
//! message and log key fields.  Real scene integration is deferred to later
//! tasks.

use crate::dispatch::PacketDispatcher;
use opensim_msg::field_str;
use opensim_msg::msgs::*;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Register all core protocol handlers on a dispatcher.
pub fn register_core_handlers(dispatcher: &mut PacketDispatcher) {
    // Task 2.4: Core viewer handlers
    register_use_circuit_code(dispatcher);
    register_complete_agent_movement(dispatcher);
    register_agent_update(dispatcher);
    register_chat_from_viewer(dispatcher);

    // Task 2.5: Object handlers
    register_object_add(dispatcher);
    register_object_delete(dispatcher);
    register_object_select(dispatcher);
    register_object_deselect(dispatcher);
    register_object_grab(dispatcher);
    register_multiple_object_update(dispatcher);

    // Task 2.6: Avatar handlers
    register_avatar_animation(dispatcher);
    register_agent_set_appearance(dispatcher);
    register_viewer_effect(dispatcher);

    // Task P1-5: Missing handlers viewers actually send
    register_region_handshake_reply(dispatcher);
    register_agent_throttle(dispatcher);
}

// ---------------------------------------------------------------------------
// Task 2.4 — Core viewer handlers
// ---------------------------------------------------------------------------

fn register_use_circuit_code(d: &mut PacketDispatcher) {
    d.register::<UseCircuitCode, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            code = msg.circuit_code.code,
            session = %msg.circuit_code.session_id,
            id = %msg.circuit_code.id,
            "UseCircuitCode",
        );
        Ok(())
    });
}

fn register_complete_agent_movement(d: &mut PacketDispatcher) {
    d.register::<CompleteAgentMovement, _>(|ctx, msg| {
        tracing::debug!(
            agent = %msg.agent_data.agent_id,
            session = %msg.agent_data.session_id,
            circuit_code = msg.agent_data.circuit_code,
            addr = %ctx.addr,
            "CompleteAgentMovement",
        );
        Ok(())
    });
}

fn register_agent_update(d: &mut PacketDispatcher) {
    d.register::<AgentUpdate, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            flags = msg.agent_data.control_flags,
            "AgentUpdate",
        );
        Ok(())
    });
}

fn register_chat_from_viewer(d: &mut PacketDispatcher) {
    d.register::<ChatFromViewer, _>(|ctx, msg| {
        let text = field_str(&msg.chat_data.message);
        tracing::debug!(
            agent = %ctx.agent_id,
            channel = msg.chat_data.channel,
            chat_type = msg.chat_data.type_,
            message = %text,
            "ChatFromViewer",
        );
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Task 2.5 — Object handlers
// ---------------------------------------------------------------------------

fn register_object_add(d: &mut PacketDispatcher) {
    d.register::<ObjectAdd, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            p_code = msg.object_data.p_code,
            material = msg.object_data.material,
            "ObjectAdd",
        );
        Ok(())
    });
}

fn register_object_delete(d: &mut PacketDispatcher) {
    d.register::<ObjectDelete, _>(|ctx, msg| {
        let ids: Vec<u32> = msg.object_data.iter().map(|o| o.object_local_id).collect();
        tracing::debug!(
            agent = %ctx.agent_id,
            count = ids.len(),
            local_ids = ?ids,
            "ObjectDelete",
        );
        Ok(())
    });
}

fn register_object_select(d: &mut PacketDispatcher) {
    d.register::<ObjectSelect, _>(|ctx, msg| {
        let ids: Vec<u32> = msg.object_data.iter().map(|o| o.object_local_id).collect();
        tracing::debug!(
            agent = %ctx.agent_id,
            count = ids.len(),
            local_ids = ?ids,
            "ObjectSelect",
        );
        Ok(())
    });
}

fn register_object_deselect(d: &mut PacketDispatcher) {
    d.register::<ObjectDeselect, _>(|ctx, msg| {
        let ids: Vec<u32> = msg.object_data.iter().map(|o| o.object_local_id).collect();
        tracing::debug!(
            agent = %ctx.agent_id,
            count = ids.len(),
            local_ids = ?ids,
            "ObjectDeselect",
        );
        Ok(())
    });
}

fn register_object_grab(d: &mut PacketDispatcher) {
    d.register::<ObjectGrab, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            local_id = msg.object_data.local_id,
            "ObjectGrab",
        );
        Ok(())
    });
}

fn register_multiple_object_update(d: &mut PacketDispatcher) {
    d.register::<MultipleObjectUpdate, _>(|ctx, msg| {
        let ids: Vec<u32> = msg.object_data.iter().map(|o| o.object_local_id).collect();
        tracing::debug!(
            agent = %ctx.agent_id,
            count = ids.len(),
            local_ids = ?ids,
            "MultipleObjectUpdate",
        );
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Task 2.6 — Avatar handlers
// ---------------------------------------------------------------------------

fn register_avatar_animation(d: &mut PacketDispatcher) {
    d.register::<AvatarAnimation, _>(|_ctx, msg| {
        tracing::debug!(
            sender = %msg.sender.id,
            animation_count = msg.animation_list.len(),
            "AvatarAnimation",
        );
        Ok(())
    });
}

fn register_agent_set_appearance(d: &mut PacketDispatcher) {
    d.register::<AgentSetAppearance, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            visual_params = msg.visual_param.len(),
            texture_entry_len = msg.object_data.texture_entry.len(),
            "AgentSetAppearance",
        );
        Ok(())
    });
}

fn register_viewer_effect(d: &mut PacketDispatcher) {
    d.register::<ViewerEffect, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            effect_count = msg.effect.len(),
            "ViewerEffect",
        );
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Task P1-5 — Handlers the viewer sends during region entry
// ---------------------------------------------------------------------------

fn register_region_handshake_reply(d: &mut PacketDispatcher) {
    d.register::<RegionHandshakeReply, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            flags = msg.region_info.flags,
            "RegionHandshakeReply",
        );
        Ok(())
    });
}

fn register_agent_throttle(d: &mut PacketDispatcher) {
    d.register::<AgentThrottle, _>(|ctx, msg| {
        tracing::debug!(
            agent = %ctx.agent_id,
            throttle_len = msg.throttle.throttles.len(),
            "AgentThrottle",
        );
        Ok(())
    });
}
