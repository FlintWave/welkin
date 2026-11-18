use crate::ecs::components::{AnimationState, Position};
use crate::ecs::entity::EntityId;
use crate::network::message::ObjectAnimation;
use crate::server::Server;
use crate::viewer::Viewer;

/// Handles an incoming `ObjectAnimation` message by updating the object's animation state
/// and forwarding the update to all viewers that are within range of the object.
///
/// The range is defined by the server's `viewer_view_distance` setting.
pub fn handle_object_animation(server: &mut Server, msg: ObjectAnimation) {
    // Update the object's animation state component, creating it if necessary.
    let entity = EntityId::from(msg.object_id);
    if let Some(anim) = server.world.get_component_mut::<AnimationState>(entity) {
        *anim = AnimationState::from(msg.animation);
    } else {
        // If the object didn't have an animation component, insert a new one.
        server
            .world
            .add_component(entity, AnimationState::from(msg.animation));
    }

    // Determine the object's position to compute viewer range.
    let obj_position = match server.world.get_component::<Position>(entity) {
        Some(pos) => pos.clone(),
        None => {
            // If the object has no position we cannot determine range;
            // just abort forwarding.
            log::warn!("ObjectAnimation received for entity without Position component: {}", entity);
            return;
        }
    };

    // Forward the animation message to all viewers within range.
    let range = server.config.viewer_view_distance;
    for viewer in server.viewers.iter_mut() {
        if in_view_range(viewer, &obj_position, range) {
            viewer.send_message(ObjectAnimation {
                object_id: msg.object_id,
                animation: msg.animation.clone(),
            });
        }
    }
}

/// Helper that determines whether a viewer is close enough to an object to receive its
/// animation update.
fn in_view_range(viewer: &Viewer, obj_pos: &Position, max_distance: f32) -> bool {
    // If the viewer does not have a known position (e.g., during login), treat it as out of range.
    let viewer_pos = match viewer.position() {
        Some(p) => p,
        None => return false,
    };
    let dx = viewer_pos.x - obj_pos.x;
    let dy = viewer_pos.y - obj_pos.y;
    let dz = viewer_pos.z - obj_pos.z;
    (dx * dx + dy * dy + dz * dz).sqrt() <= max_distance
}