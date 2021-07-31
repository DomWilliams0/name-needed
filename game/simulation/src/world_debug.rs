use crate::render::DebugRenderer;
use crate::{EcsWorld, InnerWorldRef, Renderer, ThreadedWorldLoader, WorldViewer};
use color::ColorRgb;

use std::hash::Hasher;
use unit::world::WorldPoint;

#[derive(Default)]
pub struct FeatureBoundaryDebugRenderer {
    cache: Vec<FeatureLine>,
}

type FeatureLine = (ColorRgb, WorldPoint, WorldPoint);

impl<R: Renderer> DebugRenderer<R> for FeatureBoundaryDebugRenderer {
    fn identifier(&self) -> &'static str {
        "feature boundaries"
    }

    fn name(&self) -> &'static str {
        "Feature boundaries\0"
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

        let mut frame = Frame::new(&mut self.cache);
        loader.feature_boundaries_in_range(chunks, z_range.into(), |feat, point| {
            frame.submit(feat, point);
        });

        frame.consume(renderer);
    }
}

struct Frame<'a> {
    cache: &'a mut Vec<FeatureLine>,
    last: Option<(usize, WorldPoint)>,
    color: ColorRgb,
    this_frame_idx: usize,
}

impl<'a> Frame<'a> {
    fn new(cache: &'a mut Vec<FeatureLine>) -> Self {
        let this_frame_idx = cache.len();
        Self {
            cache,
            last: None,
            color: ColorRgb::new(0, 0, 0), // unused default value
            this_frame_idx,
        }
    }

    fn submit(&mut self, feat: usize, point: impl Into<WorldPoint>) {
        let point = point.into();
        let mut new_color = true;
        if let Some((last_feat, last)) = self.last {
            if feat == last_feat {
                new_color = false;
                self.cache.push((self.color, last, point));
            };
        }

        if new_color {
            // calculate unique color for feature that hopefully doesn't clash with others
            let feat_hash = {
                let mut hasher = ahash::AHasher::new_with_keys(123, 456);
                hasher.write_usize(feat);
                hasher.finish()
            };

            let hue = (feat_hash & 0xFFFFFFFF) as f32 / (u32::MAX as f32);
            self.color = ColorRgb::new_hsl(hue, 0.7, 0.7);
        }

        self.last = Some((feat, point));
    }

    fn consume(self, renderer: &mut impl Renderer) {
        if self.cache.len() != self.this_frame_idx {
            // got new lines this frame, blat cache with them
            self.cache.copy_within(self.this_frame_idx.., 0);
            self.cache.truncate(self.cache.len() - self.this_frame_idx);
        }

        for (color, from, to) in self.cache.iter().copied() {
            renderer.debug_add_line(from, to, color);

            // individual vertices
            renderer.debug_add_square_around(from, 0.5, color);
        }
    }
}
