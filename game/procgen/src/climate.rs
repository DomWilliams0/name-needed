pub use crate::climate::iteration::ClimateIteration;
use crate::continent::ContinentMap;
use crate::PlanetParams;
use common::num_traits::real::Real;
use common::*;
use grid::{CoordRange, DynamicGrid};
use std::ops::{AddAssign, Deref, DivAssign};

pub struct Climate {}

impl Climate {
    pub fn simulate(
        continents: &ContinentMap,
        params: &PlanetParams,
        rando: &mut dyn RngCore,
        mut per_step: impl FnMut(&ClimateIteration),
    ) -> Self {
        let mut iter = ClimateIteration::new(continents, params, rando);

        for _ in 0..params.climate_iterations {
            per_step(&iter);
            iter.step();
        }

        per_step(&iter);

        Climate {}
    }
}

/// Grid covering the planet with the z dimension representing a few layers of surface air and 1
/// layer of high-up air (idk the terms I'm not a geographer)
///
/// Height between 0-1 is sorted into the nearest z axis index. Height above 1 only belongs in the
/// top high-up air level, regardless of value. Wind lives here and so should be capped at some
/// reasonable value.
pub struct PlanetGrid<T>(DynamicGrid<T>);

impl<T: Default> PlanetGrid<T> {
    const LAND_DIVISIONS: usize = 4;
    const LAND_DIVISIONS_F: f64 = Self::LAND_DIVISIONS as f64;

    /// Size of z axis in grid
    const TOTAL_HEIGHT: usize = Self::LAND_DIVISIONS + 1;

    /// Size of z axis in grid e.g. 5.0
    const TOTAL_HEIGHT_F: f64 = Self::TOTAL_HEIGHT as f64;

    fn new(params: &PlanetParams) -> Self {
        PlanetGrid(DynamicGrid::new([
            params.planet_size as usize,
            params.planet_size as usize,
            Self::TOTAL_HEIGHT,
        ]))
    }

    fn iter_land_mut(&mut self) -> impl Iterator<Item = ([usize; 3], &mut T)> {
        self.0
            .iter_coords_with_z_range_mut(CoordRange::Range(0, Self::LAND_DIVISIONS))
    }

    #[inline]
    fn land_index_for_height(height: f64) -> usize {
        debug_assert!(height >= 0.0);
        let rounded = (height * Self::LAND_DIVISIONS_F).floor() / Self::LAND_DIVISIONS_F;

        ((rounded * Self::LAND_DIVISIONS_F).floor() as usize).min(Self::LAND_DIVISIONS - 1)
    }
}

impl<T: Default + Real + AddAssign + DivAssign + From<f64>> PlanetGrid<T> {
    pub fn iter_average(&self) -> impl Iterator<Item = ([usize; 2], T)> + '_ {
        self.0
            .iter_coords_with_z_range(CoordRange::Single(0))
            .map(move |([x, y, _], val)| {
                let mut val = *val;

                for z in 1..Self::TOTAL_HEIGHT {
                    val += self.0[[x, y, z]];
                }

                val /= Self::TOTAL_HEIGHT_F.into();

                ([x, y], val)
            })
    }
}

mod iteration {
    use crate::climate::PlanetGrid;
    use crate::continent::ContinentMap;
    use crate::PlanetParams;
    use common::cgmath::num_traits::clamp;
    use common::cgmath::{Basis3, Matrix3, Rotation};
    use common::*;
    use grid::DynamicGrid;
    use rand_distr::Uniform;
    use std::f32::consts::{FRAC_PI_2, PI as PI32, TAU};
    use std::f64::consts::PI;

    pub struct ClimateIteration<'a> {
        params: PlanetParams,
        rando: &'a mut dyn RngCore,
        continents: &'a ContinentMap,
        step: usize,

        pub(crate) temperature: PlanetGrid<f64>,
        pub(crate) wind_particles: Vec<WindParticle>,
    }

    pub(crate) struct WindParticle {
        pub velocity: cgmath::Vector3<f32>,
        pub position: cgmath::Point3<f32>,
        // TODO moisture and temperature carried by wind
    }

    impl<'a> ClimateIteration<'a> {
        pub const MAX_WIND_HEIGHT: f64 = 2.0;

        pub fn new(
            continents: &'a ContinentMap,
            params: &PlanetParams,
            rando: &'a mut dyn RngCore,
        ) -> Self {
            let mut iter = ClimateIteration {
                params: params.clone(),
                rando,
                continents,
                step: 0,

                temperature: PlanetGrid::new(params),
                wind_particles: Vec::with_capacity(params.wind_particles),
            };

            iter.init();

            iter
        }

        fn init(&mut self) {
            // set up initial temperature map
            (0..5).for_each(|_| self.apply_sunlight());

            // spawn wind particles randomly above land
            let rando = &mut self.rando;
            let rando_distro_idx = Uniform::new(0, self.continents.grid.len());

            let continents = self.continents;
            self.wind_particles
                .extend((0..self.params.wind_particles).map(|_| {
                    // choose a random tile
                    let idx = rando.sample(&rando_distro_idx);
                    let tile_height = continents.grid[idx].height();

                    // position in the centre of the tile at a random height above it
                    let [x, y, _] = continents.grid.unflatten_index(idx);
                    // let z = rando.gen_range(tile_height + 0.1, Self::MAX_WIND_HEIGHT);
                    let z = tile_height;

                    WindParticle {
                        velocity: Vector3::new(
                            rando.gen_range(-1.0, 1.0),
                            rando.gen_range(-1.0, 1.0),
                            0.0,
                        ),
                        position: Point3::new(x as f32, y as f32, z as f32),
                    }
                }));
        }

        pub fn step(&mut self) {
            debug!("stepping climate simulation"; "step" => self.step);
            self.step += 1;

            self.move_wind();
            self.apply_sunlight();
        }

        // --------

        fn move_wind(&mut self) {
            const WIND_RISE_FALL_THRESHOLD: f32 = 0.1;
            const MAX_WIND_SPEED: f32 = 4.0;

            let continents = self.continents;
            let xy_limit = self.params.planet_size as f32;
            let rando = &mut self.rando;

            // TODO is averaging the wind direction the right way to go to help wind converge together?
            // average wind direction
            // const AVERAGE: usize = 4;
            // let mut land_wind_direction = DynamicGrid::<(f32, f32, u32)>::new([
            //     self.params.planet_size as usize / AVERAGE,
            //     self.params.planet_size as usize / AVERAGE,
            //     1,
            // ]);

            // eprintln!("{} -------", self.step);

            self.wind_particles.iter_mut().for_each(|wind| {
                // eprintln!("vel={:?}", wind.velocity);
                // eprintln!("pos={:?}", wind.position);

                let wind_height = wind.position.z;

                let lookup_tile = |coord| &continents.grid[continents.grid.wrap_coord(coord)];

                // update velocity
                // TODO helper on grid to unsafely lookup with a debug assert

                let mut speed_up = true;
                let tile_in_front = lookup_tile(wind.tile_in_front());

                if tile_in_front.is_land() {
                    // land is ahead, get its height
                    let land_in_front = tile_in_front.height() as f32;

                    let height_diff = land_in_front - wind_height;
                    // eprintln!(
                    //     "in front land {:?} height diff {}",
                    //     wind.tile_in_front(),
                    //     height_diff
                    // );

                    if height_diff >= WIND_RISE_FALL_THRESHOLD {
                        // wind rises over land
                        // 0=0 radians = flat, 1=pi/2 radians
                        let incline_rad = height_diff * FRAC_PI_2;

                        // go upwards with positive z
                        wind.velocity.z += incline_rad;

                        // slow down horizontally
                        let hor_slow = 0.8;
                        wind.velocity.x *= hor_slow;
                        wind.velocity.y *= hor_slow;

                        // change direction a tad horizontally
                        let rot = Basis3::from_angle_z(rad(rando.gen_range(-FRAC_PI_2, FRAC_PI_2)));
                        wind.velocity = rot.rotate_vector(wind.velocity);

                        eprintln!("RISE");

                        speed_up = false;
                    } else if height_diff <= -WIND_RISE_FALL_THRESHOLD {
                        // wind falls down big height difference
                        let decline_rad = height_diff * FRAC_PI_2;

                        // go downwards with negative z
                        wind.velocity.z += decline_rad;
                        eprintln!("FALL");

                        speed_up = false;
                    }
                }

                if speed_up {
                    // wind goes straight, increasing speed.
                    // faster over sea than land
                    let land_below = lookup_tile(wind.tile_below());
                    let speed_increase = if land_below.is_land() { 1.05 } else { 1.12 };

                    wind.velocity = Vector3::new(
                        wind.velocity.x * speed_increase,
                        wind.velocity.y * speed_increase,
                        0.0,
                    );

                    // eprintln!("SPEED");
                }

                // limit velocity
                wind.velocity = truncate(wind.velocity, MAX_WIND_SPEED);

                // add velocity to average map
                // {
                //     let pos = [
                //         wind.position.x as usize / AVERAGE,
                //         wind.position.y as usize / AVERAGE,
                //         0,
                //     ];
                //     let (xs, ys, count) = &mut land_wind_direction[pos];
                //
                //     *xs += wind.velocity.x;
                //     *ys += wind.velocity.y;
                //     *count += 1;
                // }
            });

            // calculate average velocity per cell
            // land_wind_direction.iter_mut().for_each(|(x, y, count)| {
            //     let count = *count as f32;
            //
            //     let vec = Vector2::new(*x / count, *y / count).normalize();
            //
            //     *x = vec.x;
            //     *y = vec.y;
            // });

            self.wind_particles.iter_mut().for_each(|wind| {
                // let (x, y, count) = land_wind_direction[[
                //     wind.position.x as usize / AVERAGE,
                //     wind.position.y as usize / AVERAGE,
                //     0,
                // ]];
                // if count > 1 {
                //     let new_vel = {
                //         let vel = wind.velocity.truncate();
                //         let avg_vel = Vector2::new(x, y);
                //         let lerped = vel.lerp(avg_vel, 0.25);
                //         lerped.extend(wind.velocity.z)
                //     };
                //
                //     // eprintln!("{:?} to {:?}", wind.velocity, new_vel);
                //     wind.velocity = new_vel;
                // }

                // apply velocity to wind position
                wind.position += wind.velocity;

                // wrap xy position
                wind.position.x = wind.position.x.rem_euclid(xy_limit);
                wind.position.y = wind.position.y.rem_euclid(xy_limit);

                // clamp z position to land height
                let land_height = continents.grid
                    [[wind.position.x as usize, wind.position.y as usize, 0]]
                .height() as f32;
                wind.position.z = clamp(wind.position.z, land_height, Self::MAX_WIND_HEIGHT as f32);
            });
        }

        /// Gently heat up air directly above the planet surface. Land heats up faster than water,
        /// and the equator heats up more than the poles.
        fn apply_sunlight(&mut self) {
            let planet_size = self.params.planet_size as f64;
            let latitude_coefficient = PI / planet_size;

            // heat up surface air that's just above land height only
            // e.g. height is 0.0, only heat z=0. height is 0.5, only heat up to z=half
            for (([_, y, z], temp), tile) in self
                .temperature
                .iter_land_mut()
                .zip(self.continents.grid.iter())
            {
                // TODO height doesnt change, calculate this once in a separate grid
                let land_idx = PlanetGrid::<f64>::land_index_for_height(tile.height());
                if z > land_idx {
                    // this air tile is too high above land to be affected by raw sunlight
                    continue;
                }

                let increase = {
                    // land warms faster than sea
                    let base_increase = if tile.is_land() { 0.05 } else { 0.01 };

                    // 0 at poles, 1 at equator
                    let latitude_multiplier = (y as f64 * latitude_coefficient).sin();

                    base_increase * latitude_multiplier
                };

                *temp = (*temp + increase).min(1.0);
            }
        }
    }

    impl WindParticle {
        fn tile_below(&self) -> [isize; 3] {
            point_to_tile(self.position)
        }

        fn tile_in_front(&self) -> [isize; 3] {
            point_to_tile(self.position + self.velocity)
        }
    }

    fn point_to_tile(pos: cgmath::Point3<f32>) -> [isize; 3] {
        [
            pos.x.floor() as isize,
            pos.y.floor() as isize,
            pos.z.floor() as isize,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::EPSILON;

    fn grid<T: Default>(size: u32) -> PlanetGrid<T> {
        let mut params = PlanetParams::dummy();
        params.planet_size = size;
        PlanetGrid::new(&params)
    }

    #[test]
    fn planet_grid_land_index() {
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(0.1), 0);
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(0.3), 1);
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(0.55), 2);
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(0.89), 3);
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(1.0), 3);
        assert_eq!(PlanetGrid::<f64>::land_index_for_height(4.0), 3);
    }

    #[test]
    fn planet_grid_average() {
        let mut grid = grid::<f64>(2);

        grid.0[[0, 0, 0]] = 1.0;

        for (coord, avg) in grid.iter_average() {
            match coord {
                [0, 0] => {
                    assert!(avg.approx_eq(1.0 / PlanetGrid::<f64>::TOTAL_HEIGHT_F, (EPSILON, 2)))
                }
                _ => assert!(avg.approx_eq(0.0, (EPSILON, 2))),
            }
        }
    }
}
