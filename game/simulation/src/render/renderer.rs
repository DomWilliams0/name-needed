use color::ColorRgb;
use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::{RenderComponent, TransformComponent};

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
    fn debug_add_line(&mut self, from: WorldPoint, to: WorldPoint, color: ColorRgb) {}

    #[allow(unused_variables)]
    fn debug_add_tri(&mut self, points: [WorldPoint; 3], color: ColorRgb) {}

    fn debug_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// End rendering frame
    fn deinit(&mut self) -> Self::Target;

    // ----

    fn tile_selection(&mut self, a: WorldPosition, b: WorldPosition, color: ColorRgb) {
        let a = WorldPoint::from(a);
        let b = WorldPoint::from(b);

        let bl = {
            let x = a.0.min(b.0);
            let y = a.1.min(b.1);
            let z = a.2.min(b.2);
            WorldPoint(x, y, z)
        };
        let tr = {
            let x = a.0.max(b.0) + 1.0;
            let y = a.1.max(b.1) + 1.0;
            let z = a.2.max(b.2);
            WorldPoint(x, y, z)
        };

        let w = tr.0 - bl.0;
        let h = tr.1 - bl.1;

        let br = bl + Vector2::new(w, 0.0);
        let tl = bl + Vector2::new(0.0, h);

        self.debug_add_line(bl, br, color);
        self.debug_add_line(br, tr, color);
        self.debug_add_line(tl, tr, color);
        self.debug_add_line(bl, tl, color);

        // TODO render translucent quad over selected blocks, showing which are visible/occluded. cache this mesh
    }
}
