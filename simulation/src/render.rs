use std::cell::RefCell;
use std::rc::Rc;

use specs::prelude::*;
use specs_derive::Component;

use world::SliceRange;

use crate::movement::Position;

/// Physical attributes to be rendered
#[derive(Component, Debug, Copy, Clone)]
#[storage(VecStorage)]
pub struct Physical {
    /// temporary flat color
    pub color: (u8, u8, u8),
}

pub trait Renderer {
    type Target;

    fn init(&mut self, _target: Rc<RefCell<Self::Target>>) {}

    fn start(&mut self) {}

    fn entity(&mut self, pos: &Position, physical: &Physical);

    fn finish(&mut self) {}
}

/// Wrapper for calling generic Renderer in render system
pub(crate) struct RenderSystem<'a, R: Renderer> {
    pub target: Rc<RefCell<R::Target>>,
    pub slices: SliceRange,
    pub renderer: &'a mut R,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (ReadStorage<'a, Position>, ReadStorage<'a, Physical>);

    fn run(&mut self, (pos, physical): Self::SystemData) {
        self.renderer.init(self.target.clone());

        self.renderer.start();

        for (pos, physical) in (&pos, &physical).join() {
            if self.slices.contains(pos.z) {
                self.renderer.entity(pos, physical);
            }
        }

        self.renderer.finish();
    }
}
