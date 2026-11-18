use serde::{Deserialize, Serialize};

/// Represents the current animation state for an object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimationState(pub Vec<u8>);

impl From<Vec<u8>> for AnimationState {
    fn from(data: Vec<u8>) -> Self {
        AnimationState(data)
    }
}