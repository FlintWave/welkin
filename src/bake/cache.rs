use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BakeType {
    Head,
    Upper,
    Lower,
    Eyes,
    Skirt,
    Hair,
}

impl BakeType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(BakeType::Head),
            1 => Some(BakeType::Upper),
            2 => Some(BakeType::Lower),
            3 => Some(BakeType::Eyes),
            4 => Some(BakeType::Skirt),
            5 => Some(BakeType::Hair),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct BakeCache {
    layers: HashMap<BakeType, Vec<u8>>,
}

impl BakeCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, bake_type: BakeType, data: Vec<u8>) {
        self.layers.insert(bake_type, data);
    }

    pub fn get(&self, bake_type: BakeType) -> Option<&Vec<u8>> {
        self.layers.get(&bake_type)
    }

    pub fn upload_baked_texture(&mut self, bake_type: BakeType, data: Vec<u8>) {
        self.store(bake_type, data);
    }
}