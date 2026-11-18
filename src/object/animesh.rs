use crate::animation::{Animation, AnimationState};
use crate::common::Uuid;
use crate::scene::Scene;

#[derive(Debug, Clone)]
pub struct Animesh {
    // existing fields...
    pub id: Uuid,
    // ... other fields ...
    // Store current animation state
    pub current_animation: Option<Uuid>,
    pub animation_state: AnimationState,
}

impl Animesh {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            // initialize other fields as needed
            current_animation: None,
            animation_state: AnimationState::Stopped,
        }
    }

    // existing methods...
}

// Implement the Animation trait so that Animesh objects can play animations
impl Animation for Animesh {
    fn play(&mut self, animation_id: Uuid) {
        // Store the animation ID and set state to Playing
        self.current_animation = Some(animation_id);
        self.animation_state = AnimationState::Playing;
    }

    fn stop(&mut self) {
        self.current_animation = None;
        self.animation_state = AnimationState::Stopped;
    }

    fn pause(&mut self) {
        if self.animation_state == AnimationState::Playing {
            self.animation_state = AnimationState::Paused;
        }
    }

    fn get_state(&self) -> AnimationState {
        self.animation_state
    }

    fn get_current_animation(&self) -> Option<Uuid> {
        self.current_animation
    }
}

// Ensure Animesh can be added to a scene and that its animation state is respected
impl Scene {
    pub fn add_animesh(&mut self, animesh: Animesh) {
        self.objects.insert(animesh.id, Box::new(animesh));
    }

    pub fn play_animation_on_animesh(&mut self, object_id: Uuid, animation_id: Uuid) {
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if let Some(animesh) = obj.as_any_mut().downcast_mut::<Animesh>() {
                animesh.play(animation_id);
            }
        }
    }
}