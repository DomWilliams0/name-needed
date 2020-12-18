use crate::climate::ClimateIteration;

pub trait ProgressTracker {
    fn update(&mut self, planet: Planet, climate: &ClimateIteration);
}

#[cfg(feature = "bin")]
mod gif {
    use crate::climate::ClimateIteration;
    use crate::progress::ProgressTracker;
    use crate::{Planet, Render};
    use image::gif::GifEncoder;
    use image::{Delay, Frame};
    use std::fs::File;
    use std::path::Path;
    use std::time::Duration;

    pub struct GifProgressTracker(GifEncoder<File>);

    impl GifProgressTracker {
        pub fn new(out_path: impl AsRef<Path>) -> std::io::Result<Self> {
            let file = File::create(out_path)?;
            let encoder = GifEncoder::new(file);
            Ok(Self(encoder))
        }
    }

    impl ProgressTracker for GifProgressTracker {
        fn update(&mut self, planet: Planet, climate: &ClimateIteration) {
            let mut render = Render::with_planet(planet);
            render.draw_continents();
            render.draw_climate_overlay(climate);

            const FPS: f32 = 8.0;
            let frame = Frame::from_parts(
                render.into_image(),
                0,
                0,
                Delay::from_saturating_duration(Duration::from_secs_f32(1.0 / FPS)),
            );
            self.0
                .encode_frame(frame)
                .expect("failed to encode progress gif frame");
        }
    }
}

use crate::Planet;
#[cfg(feature = "bin")]
pub use gif::GifProgressTracker;

pub struct NopProgressTracker;

impl ProgressTracker for NopProgressTracker {
    fn update(&mut self, planet: Planet, climate: &ClimateIteration) {}
}
