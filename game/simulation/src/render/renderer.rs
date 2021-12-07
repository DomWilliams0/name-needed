use color::Color;
use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::transform::{PhysicalComponent, TransformRenderDescription};
use crate::{RenderComponent, TransformComponent};

pub trait Renderer {
    type FrameContext;
    type Error: Error;

    /// Initialize frame rendering
    fn init(&mut self, target: Self::FrameContext);

    /// Start rendering simulation
    fn sim_start(&mut self);

    /// `transform` is interpolated
    fn sim_entity(
        &mut self,
        transform: &TransformRenderDescription,
        render: &RenderComponent,
        physical: &PhysicalComponent,
    );

    /// The entity with the given transform is selected, highlight it
    /// Call in addition to `sim_entity`
    fn sim_selected(
        &mut self,
        transform: &TransformRenderDescription,
        physical: &PhysicalComponent,
    );

    /// Finish rendering simulation
    fn sim_finish(&mut self) -> Result<(), Self::Error>;

    fn debug_start(&mut self) {}

    #[allow(unused_variables)]
    fn debug_add_line(&mut self, from: WorldPoint, to: WorldPoint, color: Color) {}

    #[allow(unused_variables)]
    fn debug_add_quad(&mut self, points: [WorldPoint; 4], color: Color) {}

    #[allow(unused_variables)]
    fn debug_add_circle(&mut self, centre: WorldPoint, radius: f32, color: Color) {}

    fn debug_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// End rendering frame
    fn deinit(&mut self) -> Self::FrameContext;

    // ----

    fn tile_selection(&mut self, a: WorldPosition, b: WorldPosition, color: Color) {
        let (ax, ay, az) = WorldPoint::from(a).xyz();
        let (bx, by, bz) = WorldPoint::from(b).xyz();

        let bl = {
            let x = ax.min(bx);
            let y = ay.min(by);
            let z = az.min(bz);
            WorldPoint::new_unchecked(x, y, z)
        };
        let tr = {
            let x = ax.max(bx) + 1.0;
            let y = ay.max(by) + 1.0;
            let z = az.max(bz);
            WorldPoint::new_unchecked(x, y, z)
        };

        let w = tr.x() - bl.x();
        let h = tr.y() - bl.y();

        let br = bl + Vector2::new(w, 0.0);
        let tl = bl + Vector2::new(0.0, h);

        self.debug_add_quad([bl, br, tr, tl], color);
        // TODO render translucent quad over selected blocks, showing which are visible/occluded. cache this mesh
    }

    fn debug_add_square_around(&mut self, centre: WorldPoint, radius: f32, color: Color) {
        let quad = [
            centre + (-radius, -radius, 0.0),
            centre + (-radius, radius, 0.0),
            centre + (radius, radius, 0.0),
            centre + (radius, -radius, 0.0),
        ];

        self.debug_add_quad(quad, color);
    }
}
