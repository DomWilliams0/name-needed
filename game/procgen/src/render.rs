use std::path::Path;

use image::imageops::FilterType;
use image::{GenericImage, ImageBuffer, Rgb, Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_rect_mut, draw_hollow_circle_mut, draw_line_segment_mut,
};
use imageproc::rect::Rect;

use color::ColorRgb;
use common::*;
use grid::{DynamicGrid, GridImpl};
use unit::world::{all_slabs_in_range, ChunkLocation, SlabLocation, CHUNK_SIZE, SLAB_SIZE};

use crate::params::{AirLayer, RenderProgressParams};
use crate::region::CHUNKS_PER_REGION_SIDE;
use crate::{map_range, Planet, RegionLocation, SlabGrid};
use common::num_traits::clamp;

#[derive(Clone)]
pub struct Render {
    planet: Planet,
    /// None during drawing only
    image: Option<RgbaImage>,
}

trait PixelPos {
    fn pos(self) -> (u32, u32);
}

trait PixelColor: Clone {
    fn color(self) -> Rgba<u8>;
}

impl Render {
    pub async fn with_planet(planet: Planet) -> Self {
        let p = planet.inner().await;
        let params = &p.params;
        assert!(params.render.scale > 0);

        drop(p);
        Render {
            planet,
            image: None,
        }
    }

    pub async fn draw_continents(&mut self) {
        let planet = self.planet.inner().await;
        let planet_size = planet.params.planet_size;
        let params = &planet.params.render;

        // create 1:1 image
        let mut image = ImageBuffer::new(planet_size, planet_size);

        if params.draw_continent_blobs {
            // special drawing of continents with land blobs
            let mut random_colors = ColorRgb::unique_randoms(0.7, 0.4, &mut thread_rng()).unwrap();

            let sea = if params.draw_debug_colors {
                ColorRgb::new(240, 240, 240)
            } else {
                ColorRgb::new(44, 114, 161)
            };

            image.pixels_mut().for_each(|p| *p = Rgba(sea.into()));

            for (_continent, blobs) in planet
                .continents
                .iter()
                .group_by(|(continent, _)| *continent)
                .into_iter()
            {
                let land = if params.draw_debug_colors {
                    // separate colour per continent
                    random_colors.next_please()
                } else {
                    ColorRgb::new(185, 130, 82)
                };

                for (_, blob) in blobs {
                    draw_filled_circle_mut(
                        &mut image,
                        (blob.pos.0 as i32, blob.pos.1 as i32),
                        blob.radius as i32,
                        Rgba(land.into()),
                    );

                    if params.draw_continent_blobs_outline {
                        draw_hollow_circle_mut(
                            &mut image,
                            (blob.pos.0 as i32, blob.pos.1 as i32),
                            blob.radius as i32,
                            Rgba([10, 10, 10, 255]),
                        );
                    }
                }
            }
        } else {
            for (coord, tile) in planet.continents.grid.iter_coords() {
                let float = if params.draw_height {
                    tile.height() as f32
                } else if params.draw_density {
                    unsafe { tile.density() as f32 }
                } else {
                    0.8
                };
                let c = if tile.is_land() {
                    ColorRgb::new_hsl(1.0, 0.8, float)
                } else {
                    ColorRgb::new_hsl(0.5, 0.4, float)
                };

                put_pixel(&mut image, coord, c);
            }
        }

        let scale = params.scale;
        drop(planet);
        self.store_scaled_image(image, scale);
    }

    fn store_scaled_image(&mut self, image: RgbaImage, scale: u32) {
        self.image = Some(if scale == 1 {
            image
        } else {
            let new_size = scale * image.width();
            image::imageops::resize(&image, new_size, new_size, FilterType::Nearest)
        });
    }

    #[cfg(feature = "climate")]
    pub async fn draw_climate_overlay(
        &mut self,
        climate: &crate::climate::ClimateIteration,
        layer: AirLayer,
        what: RenderProgressParams,
    ) {
        use crate::climate::PlanetGrid;

        let planet = self.planet.inner().await;
        let params = &planet.params;

        let scale = params.render.scale;

        // overlay is scaled size
        let mut overlay = RgbaImage::new(params.planet_size * scale, params.planet_size * scale);

        match what {
            RenderProgressParams::Temperature => {
                climate.temperature.iter_average(layer, |coord, val| {
                    debug_assert!(val >= 0.0 && val <= 1.0, "val={:?}", val);
                    let c = color_for_temperature(val as f32);
                    put_pixel_scaled(&mut overlay, scale, coord, c.array_with_alpha(50));
                })
            }
            RenderProgressParams::Wind => {
                let opacity = match layer {
                    AirLayer::Surface => 240,
                    AirLayer::High => 100,
                };

                let scale = scale as f32;

                // TODO per land layer?
                for ([x, y, z], wind) in climate.wind.iter_layer(layer) {
                    const EPSILON: f64 = 0.3;
                    if wind.velocity.magnitude2() <= EPSILON.powi(2) {
                        // too short
                        continue;
                    }
                    let velocity_trunc = wind.velocity.truncate().cast::<f32>().unwrap();

                    let line_start = Vector2::new(x as f32, y as f32);
                    let line_end = line_start + velocity_trunc;

                    let direction = velocity_trunc.angle(Vector2::unit_y()).normalize()
                        / cgmath::Rad::full_turn();

                    let _hue = match layer {
                        AirLayer::Surface => {
                            let val = z as f64 / PlanetGrid::<f64>::TOTAL_HEIGHT_F;
                            map_range((0.0, 1.0), (0.3, 0.8), val as f32)
                        }
                        AirLayer::High => 0.1,
                    };

                    let color = ColorRgb::new_hsl(direction, 0.6, 0.5);

                    draw_line_segment_mut(
                        &mut overlay,
                        (line_start.x * scale, line_start.y * scale),
                        (line_end.x * scale, line_end.y * scale),
                        Rgba(color.array_with_alpha(opacity)),
                    );

                    // let magnitude = wind.velocity.magnitude();
                    // let c = color_for_temperature(magnitude as f32);
                    // put_pixel_scaled(&mut overlay, scale, coord, c.array_with_alpha(50));
                }
            }
            RenderProgressParams::AirPressure => {
                climate.air_pressure.iter_average(layer, |coord, val| {
                    debug_assert!(val >= 0.0 && val <= 1.0, "val={:?}", val);
                    let c = color_for_temperature(val as f32);
                    put_pixel_scaled(&mut overlay, scale, coord, c.array_with_alpha(50));
                });
            }
        };

        let image = self.image.as_mut().expect("image has not been created");
        image::imageops::overlay(image, &overlay, 0, 0);
    }

    pub async fn draw_region(&mut self, region_loc: RegionLocation) {
        // params
        let inner = self.planet.inner().await;
        let start_slab = inner.params.render.region_start_slab;
        let max_depth = inner.params.render.region_max_depth;
        let scale = inner.params.render.scale;
        drop(inner);

        // create 1:1 image for region
        let mut image = {
            let region_size = CHUNKS_PER_REGION_SIDE.as_u32() * CHUNK_SIZE.as_u32();
            ImageBuffer::new(region_size, region_size)
        };

        let (mut min_height, mut max_height) = (i32::MAX, i32::MIN);
        let mut processed_chunks = DynamicGrid::new([
            CHUNKS_PER_REGION_SIDE.as_usize(),
            CHUNKS_PER_REGION_SIDE.as_usize(),
            1,
        ]);

        let (from_chunk_local, to_chunk_local) = (
            ChunkLocation(0, 0),
            ChunkLocation(
                CHUNKS_PER_REGION_SIDE.as_i32() - 1,
                CHUNKS_PER_REGION_SIDE.as_i32() - 1,
            ),
        );

        for chunk_local in from_chunk_local.iter_until(to_chunk_local) {
            let chunk = region_loc.local_chunk_to_global(chunk_local);

            // log_scope!(o!(chunk));
            // TODO fix log_scope crashing with async
            debug!("generating chunk"; chunk);

            let mut visible_blocks =
                DynamicGrid::new([CHUNK_SIZE.as_usize(), CHUNK_SIZE.as_usize(), 1]);

            let mut initialized_count = 0;
            const TOTAL_TO_INITIALIZE: usize = CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize();

            for slab_z in ((start_slab - max_depth + 1)..=start_slab).rev() {
                debug!("generating slabs at {z}", z = slab_z);
                // generate slab
                let slab = SlabLocation::new(slab_z, chunk);
                let generated = self.planet.generate_slab(slab).await;

                // copy highest non-air blocks to image
                for y in 0..CHUNK_SIZE.as_usize() {
                    for x in 0..CHUNK_SIZE.as_usize() {
                        for z in (0..SLAB_SIZE.as_usize()).rev() {
                            let block = generated[&[x as i32, y as i32, z as i32]];
                            if block.is_air() {
                                continue;
                            }

                            // aha, solid block. store global z (possibly negative) to scale to
                            // range later and make positive
                            let z = (slab_z * SLAB_SIZE.as_i32()) + z as i32;
                            visible_blocks[[x, y, 0]] = Some((z, block.ty));
                            max_height = max_height.max(z);
                            min_height = min_height.min(z);
                            break;
                        }
                    }
                }

                initialized_count = visible_blocks.iter().filter(|opt| opt.is_some()).count();
                debug_assert!(initialized_count <= TOTAL_TO_INITIALIZE);

                trace!(
                    "{left} blocks left to initialize",
                    left = TOTAL_TO_INITIALIZE - initialized_count
                );

                if initialized_count == TOTAL_TO_INITIALIZE {
                    // all done
                    break;
                }
            }

            if initialized_count < TOTAL_TO_INITIALIZE {
                warn!(
                    "there are {count} uninitialized blocks in the given slab range, try tweaking \
                max depth or start slab",
                    count = TOTAL_TO_INITIALIZE - initialized_count
                );
            }

            processed_chunks[[chunk_local.x() as usize, chunk_local.y() as usize, 0]] =
                Some(visible_blocks);
        }

        info!(
            "z range from {min} to {max}",
            min = min_height,
            max = max_height
        );
        if min_height == max_height {
            warn!("region might be filled with solid blocks, try tweaking start slab")
        }

        // render chunks to image
        for ([cx, cy, _], chunk) in processed_chunks.iter_coords() {
            let visible_blocks = chunk.as_ref().unwrap(); // definitely initialized

            for ([bx, by, _], block) in visible_blocks.iter_coords() {
                let color = match *block {
                    Some((z, block_type)) => {
                        let max_height = max_height as f32;
                        let min_height = min_height as f32;

                        let l = map_range(
                            (min_height, max_height),
                            (0.2, 0.8),
                            clamp(z as f32, min_height, max_height),
                        );

                        let (h, s) = block_type.color_hs();
                        ColorRgb::new_hsl(h, s, l)
                    }
                    None => {
                        // hot pink if missing
                        ColorRgb::new_hsl(0.83, 1.0, 0.7)
                    }
                };

                let px = (cx * CHUNK_SIZE.as_usize()) + bx;
                let py = (cy * CHUNK_SIZE.as_usize()) + by;
                *image.get_pixel_mut(px as u32, py as u32) = color.color();
            }
        }

        self.store_scaled_image(image, scale);
    }

    pub fn save(&self, path: impl AsRef<Path>) -> BoxedResult<()> {
        let image = self.image.as_ref().expect("image has not been created");

        let path = path.as_ref();
        image.save(path)?;
        info!("saved image to {file}", file = path.display());
        Ok(())
    }

    pub fn into_image(mut self) -> RgbaImage {
        self.image.take().expect("image was taken but not replaced")
    }
}

fn put_pixel(image: &mut RgbaImage, pos: impl PixelPos, color: impl PixelColor) {
    let (w, h) = image.dimensions();
    let (x, y) = pos.pos();
    debug_assert!(x < w && y < h);

    unsafe { image.unsafe_put_pixel(x, y, color.color()) }
}

fn put_pixel_scaled(image: &mut RgbaImage, scale: u32, pos: impl PixelPos, color: impl PixelColor) {
    let (x, y) = pos.pos();
    draw_filled_rect_mut(
        image,
        Rect::at((x * scale) as i32, (y * scale) as i32).of_size(scale, scale),
        color.color(),
    );
}

/// 0=cold, 1=hot
/// https://gist.github.com/stasikos/06b02d18f570fc1eaa9f
#[allow(clippy::excessive_precision)]
fn color_for_temperature(temp: f32) -> ColorRgb {
    // scale to kelvin
    let kelvin = (1.0 - temp) * 8000.0;

    let x = (kelvin / 1000.0).min(40.0);

    fn poly(coefficients: &[f32], x: f32) -> f32 {
        let mut result = coefficients[0];
        let mut xn = x;
        for c in &coefficients[1..] {
            result += xn * *c;
            xn *= x;
        }
        result
    }

    let r = if kelvin < 6527.0 {
        1.0
    } else {
        const REDPOLY: [f32; 8] = [
            4.93596077e0,
            -1.29917429e0,
            1.64810386e-01,
            -1.16449912e-02,
            4.86540872e-04,
            -1.19453511e-05,
            1.59255189e-07,
            -8.89357601e-10,
        ];
        poly(&REDPOLY, x)
    };

    // G
    let g = if kelvin < 850.0 {
        0.0
    } else if kelvin <= 6600.0 {
        const GREENPOLY: [f32; 8] = [
            -4.95931720e-01,
            1.08442658e0,
            -9.17444217e-01,
            4.94501179e-01,
            -1.48487675e-01,
            2.49910386e-02,
            -2.21528530e-03,
            8.06118266e-05,
        ];
        poly(&GREENPOLY, x)
    } else {
        const GREENPOLY: [f32; 8] = [
            3.06119745e0,
            -6.76337896e-01,
            8.28276286e-02,
            -5.72828699e-03,
            2.35931130e-04,
            -5.73391101e-06,
            7.58711054e-08,
            -4.21266737e-10,
        ];

        poly(&GREENPOLY, x)
    };

    // B
    let b = if kelvin < 1900.0 {
        0.0
    } else if kelvin < 6600.0 {
        const BLUEPOLY: [f32; 8] = [
            4.93997706e-01,
            -8.59349314e-01,
            5.45514949e-01,
            -1.81694167e-01,
            4.16704799e-02,
            -6.01602324e-03,
            4.80731598e-04,
            -1.61366693e-05,
        ];
        poly(&BLUEPOLY, x)
    } else {
        1.0
    };

    ColorRgb::new_float(r, g, b)
}

impl PixelPos for [usize; 3] {
    fn pos(self) -> (u32, u32) {
        (self[0] as u32, self[1] as u32)
    }
}

impl PixelPos for [usize; 2] {
    fn pos(self) -> (u32, u32) {
        (self[0] as u32, self[1] as u32)
    }
}

impl PixelPos for (u32, u32) {
    fn pos(self) -> (u32, u32) {
        self
    }
}

impl PixelColor for ColorRgb {
    fn color(self) -> Rgba<u8> {
        Rgba(self.array_with_alpha(255))
    }
}

impl PixelColor for [u8; 4] {
    fn color(self) -> Rgba<u8> {
        Rgba(self)
    }
}