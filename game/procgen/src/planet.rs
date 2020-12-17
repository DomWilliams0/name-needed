use crate::continent::ContinentMap;
use crate::rasterize::SlabGrid;
use common::parking_lot::RwLock;
use common::*;

use crate::params::PlanetParams;
use image::GenericImage;
use std::sync::Arc;
use unit::world::{ChunkLocation, SlabLocation};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

unsafe impl Send for Planet {}
unsafe impl Sync for Planet {}

#[derive(Default)]
pub struct Region {
    pub height: f64,
}

pub struct PlanetInner {
    params: PlanetParams,
    continents: ContinentMap,
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

        let mut planet_rando = StdRng::seed_from_u64(params.seed());

        // place continents
        let (continents, total_blobs) = planet.continents.generate(&mut planet_rando);
        // TODO reject if continent or land blob count is too low
        info!(
            "placed {count} continents with {blobs} land blobs",
            count = continents,
            blobs = total_blobs
        );

        // rasterize continents onto grid and discover depth i.e. distance from land/sea border,
        // and place initial heightmap
        planet.continents.discover(&mut planet_rando);
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // TODO could have separate copy of planet params per thread if immutable

        // radius is excluding 0,0
        // TODO radius no longer makes sense
        let radius = 5;
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
        use crate::params::DrawMode;
        use color::ColorRgb;
        use image::{DynamicImage, ImageBuffer, Rgb};
        use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut};

        let planet = self.0.read();

        let mut image = ImageBuffer::new(planet.params.planet_size, planet.params.planet_size);

        macro_rules! put_pixel {
            ($xy:expr, $colour:expr) => {
                let xy = $xy;
                let (x, y) = (xy[0] as u32, xy[1] as u32);
                debug_assert!(x < image.dimensions().0 && y < image.dimensions().1);
                unsafe {
                    image.unsafe_put_pixel(x, y, $colour);
                }
            };
        }

        match planet.params.draw_mode() {
            DrawMode::Outlines {
                debug_colors,
                outlines,
            } => {
                let mut colors = ColorRgb::unique_randoms(0.7, 0.4, &mut thread_rng()).unwrap();

                let planet = self.0.read();

                let (sea, land) = if debug_colors {
                    (ColorRgb::new(240, 240, 240), colors.next_please())
                } else {
                    (ColorRgb::new(44, 114, 161), ColorRgb::new(185, 130, 82))
                };

                for (coord, tile) in planet.continents.grid.iter_coords() {
                    let c = if tile.is_land() { land } else { sea };

                    let c = Rgb(c.into());
                    put_pixel!(coord, c);
                }

                if outlines {
                    for (_, blob) in planet.continents.iter() {
                        draw_filled_circle_mut(
                            &mut image,
                            (blob.pos.0 as i32, blob.pos.1 as i32),
                            blob.radius as i32,
                            Rgb(land.into()),
                        );
                        draw_hollow_circle_mut(
                            &mut image,
                            (blob.pos.0 as i32, blob.pos.1 as i32),
                            blob.radius as i32,
                            Rgb([10, 10, 10]),
                        );
                    }
                }
            }

            DrawMode::Density => {
                for (coord, tile) in planet.continents.grid.iter_coords() {
                    let d = tile.density() as f32;
                    let c = if tile.is_land() {
                        ColorRgb::new_hsl(1.0, 0.8, d)
                    } else {
                        ColorRgb::new_hsl(0.5, 0.8, d)
                    };

                    let c = Rgb(c.into());
                    put_pixel!(coord, c);
                }
            }
            DrawMode::Height => {
                for (coord, tile) in planet.continents.grid.iter_coords() {
                    let height = tile.height as f32;
                    let c = if tile.is_land() {
                        ColorRgb::new_hsl(1.0, 0.8, height)
                    } else {
                        ColorRgb::new_hsl(0.5, 0.4, height)
                    };

                    let c = Rgb(c.into());
                    put_pixel!(coord, c);
                }
            }
        };

        DynamicImage::ImageRgb8(image)
    }
}
