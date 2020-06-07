use crate::{RenderComponent, TransformComponent};
use color::ColorRgb;
use std::fmt::Debug;
use unit::view::ViewPoint;

pub trait Renderer {
    type Target;
    type Error: Debug;

    /// Initialize frame rendering
    fn init(&mut self, target: Self::Target);

    /// Start rendering simulation
    fn sim_start(&mut self);

    /// `transform` is interpolated
    fn sim_entity(&mut self, transform: &TransformComponent, render: &RenderComponent);

    /// The entity with the given transform is selected, highlight it
    /// Call in addition to `sim_entity`
    fn sim_selected(&mut self, transform: &TransformComponent);

    /// Finish rendering simulation
    fn sim_finish(&mut self) -> Result<(), Self::Error>;

    fn debug_start(&mut self) {}

    #[allow(unused_variables)]
    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {}

    #[allow(unused_variables)]
    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: ColorRgb) {}

    fn debug_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// End rendering frame
    fn deinit(&mut self) -> Self::Target;
}
