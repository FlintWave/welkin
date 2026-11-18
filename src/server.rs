use crate::ecs::World;
use crate::viewer::Viewer;
use crate::config::Config;

pub struct Server {
    pub world: World,
    pub viewers: Vec<Viewer>,
    pub config: Config,
    // other fields...
}

impl Server {
    // existing methods ...

    /// Dispatches an incoming `ObjectAnimation` message.
    pub fn dispatch_object_animation(&mut self, msg: crate::network::message::ObjectAnimation) {
        crate::handlers::object_animation::handle_object_animation(self, msg);
    }
}