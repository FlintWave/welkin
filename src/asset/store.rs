use std::collections::HashMap;
use std::any::TypeId;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AssetId<T> {
    id: usize,
    _marker: std::marker::PhantomData<T>,
}

pub struct AssetStore {
    next_id: usize,
    assets: HashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

impl AssetStore {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            assets: HashMap::new(),
        }
    }

    pub fn store_asset<T: 'static + Send + Sync>(&mut self, asset: T) -> AssetId<T> {
        let type_id = TypeId::of::<T>();
        let entry = self.assets.entry(type_id).or_insert_with(|| Box::new(Mutex::new(HashMap::<usize, Arc<T>>::new())));
        let map = entry.downcast_mut::<Mutex<HashMap<usize, Arc<T>>>>().unwrap();
        let mut guard = map.lock().unwrap();
        let id = self.next_id;
        self.next_id += 1;
        guard.insert(id, Arc::new(asset));
        AssetId { id, _marker: std::marker::PhantomData }
    }

    pub fn get_asset<T: 'static + Send + Sync>(&self, id: AssetId<T>) -> Option<Arc<T>> {
        self.assets.get(&TypeId::of::<T>())?.downcast_ref::<Mutex<HashMap<usize, Arc<T>>>>()?.lock().unwrap().get(&id.id).cloned()
    }
}