#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AnimationState {
    Stopped,
    Playing,
    Paused,
}

pub trait Animation {
    fn play(&mut self, animation_id: crate::common::Uuid);
    fn stop(&mut self);
    fn pause(&mut self);
    fn get_state(&self) -> AnimationState;
    fn get_current_animation(&self) -> Option<crate::common::Uuid>;
}