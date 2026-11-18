// Existing message definitions ...

/// Message sent by a client to update an object's animation state.
#[derive(Debug, Clone)]
pub struct ObjectAnimation {
    pub object_id: u64,
    pub animation: Vec<u8>, // raw animation data; format defined by the simulator
}

// Existing code continues...