use crate::render::DebugRenderer;
use crate::{EcsWorld, InnerWorldRef, Renderer, ThreadedWorldLoader, WorldViewer};
use color::ColorRgb;
use common::{SeedableRng, SmallRng};
use unit::world::WorldPoint;

#[derive(Default)]
pub struct FeatureBoundaryDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for FeatureBoundaryDebugRenderer {
    fn identifier(&self) -> &'static str {
        "feature boundaries"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        loader: &ThreadedWorldLoader,
        _: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        let chunks = viewer.visible_chunks();
        let z_range = viewer.terrain_range();

        let mut last = None;

        let mut randy = SmallRng::seed_from_u64(0x49ff171a42a18cda);
        let mut colors = ColorRgb::unique_randoms(0.8, 0.8, &mut randy).unwrap(); // valid params
        let mut color = colors.next_please();

        loader.feature_boundaries_in_range(chunks, z_range.into(), |feat, point| {
            let point = WorldPoint::from(point);
            if let Some((last_feat, last)) = last {
                if feat != last_feat {
                    color = colors.next_please();
                };

                renderer.debug_add_line(last, point, color);
            }

            last = Some((feat, point));
        });
    }
}
