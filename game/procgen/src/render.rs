use crate::{Planet, PlanetParams};
use color::ColorRgb;
use common::*;
use image::{GenericImage, ImageBuffer, Rgb, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_hollow_circle_mut};
use std::path::Path;

pub struct Render {
    planet: Planet,
    /// None during drawing only
    image: Option<RgbaImage>,
}

trait PixelPos {
    fn pos(&self) -> (u32, u32);
}

impl Render {
    pub fn with_planet(planet: Planet) -> Self {
        let p = planet.inner();
        let params = &p.params;
        let image = ImageBuffer::new(params.planet_size, params.planet_size);

        drop(p);
        Render {
            planet,
            image: Some(image),
        }
    }

    pub fn draw_continents(&mut self) {
        let planet = self.planet.inner();
        let params = &planet.params.render;

        // take ownership of image
        let mut image = self.image.take().expect("image was taken but not replaced");

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
                    tile.density() as f32
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

        // put image back
        self.image = Some(image);
    }

    pub fn save(&self, path: impl AsRef<Path>) -> BoxedResult<()> {
        let image = self
            .image
            .as_ref()
            .expect("image was taken but not replaced");

        let path = path.as_ref();
        image.save(path)?;
        info!("saved image to {file}", file = path.display());
        Ok(())
    }
}

fn put_pixel(image: &mut RgbaImage, pos: impl PixelPos, color: ColorRgb) {
    let (w, h) = image.dimensions();
    let (x, y) = pos.pos();
    debug_assert!(x < w && y < h);

    unsafe { image.unsafe_put_pixel(x, y, Rgba(color.into())) }
}

impl PixelPos for [usize; 3] {
    fn pos(&self) -> (u32, u32) {
        (self[0] as u32, self[1] as u32)
    }
}
