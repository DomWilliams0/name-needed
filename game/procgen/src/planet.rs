use crate::continent::ContinentMap;
use crate::rasterize::SlabGrid;
use common::parking_lot::RwLock;
use common::*;

use std::sync::Arc;
use unit::world::{ChunkLocation, SlabLocation};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

#[derive(Default)]
pub struct Region {
    pub height: f64,
}

pub struct PlanetInner {
    params: PlanetParams,
    continents: ContinentMap,
}

#[derive(Debug, Clone)]
pub struct PlanetParams {
    pub seed: u64,
    #[deprecated]
    pub radius: u32,

    /// Height and width of surface in some unit
    pub planet_size: u32,
    pub max_continents: usize,
}
impl Default for PlanetParams {
    fn default() -> Self {
        PlanetParams {
            seed: 0,
            radius: 4,
            planet_size: 128,
            max_continents: 12,
        }
    }
}

/// https://rosettacode.org/wiki/Map_range#Rust
#[inline]
fn map_range(from_range: (f64, f64), to_range: (f64, f64), s: f64) -> f64 {
    to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> Result<Planet, &'static str> {
        debug!("creating planet with params {:?}", params);
        let continents = ContinentMap::new(&params);
        let inner = Arc::new(RwLock::new(PlanetInner { params, continents }));
        Ok(Self(inner))
    }

    pub fn initial_generation(&mut self) {
        let mut planet = self.0.write();
        let params = planet.params.clone();

        let mut planet_rando = StdRng::seed_from_u64(params.seed);
        let continents = planet.continents.generate(&mut planet_rando);
        debug!("placed {count} continents", count = continents);

        /*        // populate heightmap
                let noise = Fbm::new()
                    .set_seed(params.seed as u32) // TODO seed loses so much of its entropy
                    .set_octaves(5)
                    .set_frequency(0.2);

                let sz = params.planet_size;
                let mut i = 0;
                for ry in 0..sz {
                    for rx in 0..sz {
                        let scale = 4.0;
                        let nx = rx as f64 / scale;
                        let ny = ry as f64 / scale;
                        let val = noise.get([nx, ny, 0.6]);

                        let height = map_range((-1.0, 1.0), (0.0, 1.0), val);
                        planet.regions.index_mut(i).height = height;
                        i += 1;
                    }
                }
        */
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // TODO could have separate copy of planet params per thread if immutable
        let inner = self.0.read();

        // radius is excluding 0,0
        let radius = inner.params.radius as i32;
        (
            ChunkLocation(-radius, -radius),
            ChunkLocation(radius, radius),
        )
    }

    pub fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        SlabGrid::default()
    }

    #[cfg(feature = "bin")]
    pub fn inner(&self) -> impl std::ops::Deref<Target = PlanetInner> + '_ {
        self.0.read()
    }

    #[cfg(feature = "bin")]
    pub fn as_image(&self) -> image::DynamicImage {
        use color::ColorRgb;
        use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
        use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut};

        let debug_colors = true;

        let mut colors = ColorRgb::unique_randoms(0.7, 0.4, &mut thread_rng()).unwrap();

        let planet = self.0.read();
        let mut image = ImageBuffer::from_pixel(
            planet.params.planet_size,
            planet.params.planet_size,
            Rgb(if debug_colors {
                [240, 240, 240]
            } else {
                [44, 114, 161]
            }),
        );

        for (_, blobs) in planet
            .continents
            .iter()
            .group_by(|(idx, _)| *idx)
            .into_iter()
        {
            let color = if debug_colors {
                colors.next_please()
            } else {
                ColorRgb::new(185, 130, 82)
            };

            for (_, blob) in blobs {
                draw_filled_circle_mut(
                    &mut image,
                    (blob.pos.0 as i32, blob.pos.1 as i32),
                    blob.radius as i32,
                    Rgb(color.into()),
                );

                if debug_colors {
                    // outline
                    draw_hollow_circle_mut(
                        &mut image,
                        (blob.pos.0 as i32, blob.pos.1 as i32),
                        blob.radius as i32,
                        Rgb([10, 10, 10]),
                    );
                }
            }
        }

        DynamicImage::ImageRgb8(image)
    }
}
