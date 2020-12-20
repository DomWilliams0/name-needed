pub use crate::climate::iteration::ClimateIteration;
use crate::continent::ContinentMap;
use crate::params::AirLayer;
use crate::PlanetParams;
use common::num_traits::real::Real;
use common::*;
use grid::{CoordRange, DynamicGrid};
use std::ops::{AddAssign, DivAssign};

pub struct Climate {}

impl Climate {
    pub fn simulate(
        continents: &ContinentMap,
        params: &PlanetParams,
        rando: &mut dyn RngCore,
        mut per_step: impl FnMut(u32, &ClimateIteration),
    ) -> Self {
        let mut iter = ClimateIteration::new(continents, params, rando);

        for step in 0..params.climate_iterations {
            per_step(step as u32, &iter);
            iter.step();
        }

        per_step(params.climate_iterations as u32, &iter);

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

const LAND_DIVISIONS: usize = 4;

impl<T: Default> PlanetGrid<T> {
    const LAND_DIVISIONS_F: f64 = LAND_DIVISIONS as f64;

    /// Size of z axis in grid
    pub const TOTAL_HEIGHT: usize = LAND_DIVISIONS + 1;
    pub const TOTAL_HEIGHT_F: f64 = Self::TOTAL_HEIGHT as f64;

    fn new(params: &PlanetParams) -> Self {
        PlanetGrid(DynamicGrid::new([
            params.planet_size as usize,
            params.planet_size as usize,
            Self::TOTAL_HEIGHT,
        ]))
    }

    #[inline]
    fn land_index_for_height(height: f64) -> usize {
        debug_assert!(height >= 0.0, "val={:?}", height);
        let rounded = (height * Self::LAND_DIVISIONS_F).floor() / Self::LAND_DIVISIONS_F;

        ((rounded * Self::LAND_DIVISIONS_F).floor() as usize).min(LAND_DIVISIONS - 1)
    }

    pub fn iter_layer(&self, layer: AirLayer) -> impl Iterator<Item = ([usize; 3], &T)> {
        self.0.iter_coords_with_z_range(layer.into())
    }

    fn iter_layer_mut(&mut self, layer: AirLayer) -> impl Iterator<Item = ([usize; 3], &mut T)> {
        self.0.iter_coords_with_z_range_mut(layer.into())
    }

    /// Each land layer followed by high air
    fn iter_individual_layers_mut(&mut self) -> impl Iterator<Item = ([usize; 3], &mut T)> {
        self.0.iter_coords_with_z_range_mut(CoordRange::All)
    }

    fn iter_individual_layers(&self) -> impl Iterator<Item = ([usize; 3], &T)> {
        self.0.iter_coords_with_z_range(CoordRange::All)
    }

    pub fn iter_layer_coords(
        coords: impl Into<CoordRange>,
        params: &PlanetParams,
    ) -> impl Iterator<Item = [usize; 3]> {
        let (iter, _) = DynamicGrid::<()>::iter_coords_alone_static(
            coords.into(),
            [
                params.planet_size as usize,
                params.planet_size as usize,
                Self::TOTAL_HEIGHT,
            ],
        );
        iter
    }
}

impl<T: Default + Real + AddAssign + DivAssign + From<f64>> PlanetGrid<T> {
    pub fn iter_average(&self, layer: AirLayer, mut f: impl FnMut([usize; 2], T)) {
        match layer {
            AirLayer::High => {
                // just one layer
                self.iter_layer(layer)
                    .for_each(|([x, y, _], val)| f([x, y], *val));
            }
            AirLayer::Surface => self
                .0
                .iter_coords_with_z_range(CoordRange::Single(0))
                .for_each(move |([x, y, _], val)| {
                    let mut val = *val;

                    for z in 1..Self::TOTAL_HEIGHT {
                        val += self.0[[x, y, z]];
                    }

                    val /= Self::TOTAL_HEIGHT_F.into();

                    f([x, y], val);
                }),
        }
    }
}

impl From<AirLayer> for CoordRange {
    fn from(layer: AirLayer) -> Self {
        match layer {
            AirLayer::Surface => CoordRange::Range(0, LAND_DIVISIONS),
            AirLayer::High => CoordRange::Single(LAND_DIVISIONS),
        }
    }
}

mod iteration {
    use crate::climate::{PlanetGrid, LAND_DIVISIONS};
    use crate::continent::ContinentMap;
    use crate::params::AirLayer;
    use crate::PlanetParams;
    use common::cgmath::num_traits::{clamp, real::Real};
    use common::cgmath::prelude::*;
    use common::cgmath::{Basis3, Point3, Rad, Rotation, Vector2, Vector3};
    use common::{debug, thread_rng, truncate, ArrayVec, OrderedFloat, Rng, RngCore};
    use grid::CoordRange;
    use rand_distr::Uniform;
    use std::f32::consts::{FRAC_PI_2, TAU};
    use std::f64::consts::PI;
    use std::f64::EPSILON;
    use strum::IntoEnumIterator;

    pub struct ClimateIteration<'a> {
        params: PlanetParams,
        rando: &'a mut dyn RngCore,
        continents: &'a ContinentMap,
        step: usize,

        pub(crate) temperature: PlanetGrid<f64>,
        pub(crate) wind: PlanetGrid<Wind>,
        pub(crate) air_pressure: PlanetGrid<f64>,
        pub(crate) wind_particles: Vec<WindParticle>,
    }

    pub(crate) struct Wind {
        pub velocity: Vector3<f64>,
    }

    pub(crate) struct WindParticle {
        pub velocity: Vector3<f32>,
        pub position: Point3<f32>,
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
                wind: PlanetGrid::new(params),
                air_pressure: PlanetGrid::new(params),
                wind_particles: Vec::with_capacity(params.wind_particles),
            };

            iter.init();

            iter
        }

        fn init(&mut self) {
            // set up initial temperature map
            (0..5).for_each(|_| self.apply_sunlight());

            // set up initial air pressure
            let mut pressure_rando = thread_rng();
            let surface_distrs = [
                Uniform::new(0.9, 0.98), // lowest land
                Uniform::new(0.8, 0.9),
                Uniform::new(0.7, 0.8),
                Uniform::new(0.6, 0.7), // highest land
            ];
            debug_assert_eq!(surface_distrs.len(), LAND_DIVISIONS);
            let high_distr = Uniform::new(0.05, 0.15);
            self.air_pressure
                .iter_layer_mut(AirLayer::Surface)
                .for_each(|([_, _, z], pressure)| {
                    // surface is high pressure
                    *pressure = pressure_rando.sample(&surface_distrs[z]);
                });

            self.air_pressure
                .iter_layer_mut(AirLayer::High)
                .for_each(|(_, pressure)| {
                    // high up is low pressure
                    *pressure = pressure_rando.sample(&high_distr);
                });
        }

        pub fn step(&mut self) {
            debug!("stepping climate simulation"; "step" => self.step);
            self.step += 1;

            self.apply_sunlight();
            self.move_air_vertically();

            self.make_wind();
            self.propagate_wind();
            // self.apply_wind();
            // TODO wind moving brings air to level out pressure

            // TODO wind is not being affected by terrain at all
            // TODO wind is getting stuck low down and not rising
        }

        // --------

        fn propagate_wind(&mut self) {
            let mut new_vals = PlanetGrid::<Wind>::new(&self.params);

            for ((coord, wind), tile) in self
                .wind
                .iter_individual_layers()
                .zip(self.continents.grid.iter())
            {
                let mut new_vel = Vector3::zero();

                // adjust speed based on angle
                let angle = wind.velocity.angle(Vector3::unit_z());
                let steepness = (angle.normalize() / 2.0).sin();
                new_vel += wind
                    .velocity
                    .truncate()
                    .normalize_to(steepness * 2.0)
                    .extend(0.0);

                new_vals.0[coord].velocity = wind.velocity.lerp(new_vel, 0.3);
            }

            // propagate forwards
            for (coord, this_wind) in self.wind.iter_individual_layers() {
                let [x, y, z] = coord;

                let dest_coord = {
                    let src = Point3::new(x as f64, y as f64, z as f64);
                    let Point3 { x, y, z } = src + this_wind.velocity;
                    let coord = [x.round() as isize, y.round() as isize, z.round() as isize];

                    new_vals.0.wrap_coord(coord)
                };

                new_vals.0[dest_coord].velocity += this_wind.velocity;
            }

            for (_, wind) in new_vals.iter_individual_layers_mut() {
                wind.velocity = truncate(wind.velocity, 1.0);
            }

            let _old = std::mem::replace(&mut self.wind, new_vals);
        }
        /// Apply wind velocities to transfer air pressure, temperature, moisture
        fn apply_wind(&mut self) {
            /*let air = &mut self.air_pressure;
            let temp = &mut self.temperature;
            for (coord, wind) in self.wind.iter_individual_layers_mut() {
                // TODO distribute across neighbours more smoothly, advection?

                let wind_mag = {
                    let wind_mag = wind.velocity.magnitude2();
                    if wind_mag < 0.1_f64.powi(2) {
                        // too weak
                        continue;
                    }
                    wind_mag.sqrt()
                };

                // get next tile in direction
                let dst = {
                    let dx = wind.velocity.x.abs().ceil().copysign(wind.velocity.x) as isize;
                    let dy = wind.velocity.y.abs().ceil().copysign(wind.velocity.y) as isize;
                    debug_assert!(dx != 0 || dy != 0);
                    let coord = [
                        coord[0] as isize + dx,
                        coord[1] as isize + dy,
                        0, // TODO only works if 1 layer of each
                    ];

                    self.wind.0.wrap_coord(coord)
                };

                // transfer
                // TODO if too big (>0.01) we end up with little pockets of unchanging high pressure :(
                let transfer = wind_mag * self.params.wind_transfer_rate;
                decrement(&mut air.0[coord], transfer);
                increment(&mut air.0[dst], transfer);

                decrement(&mut temp.0[coord], transfer);
                increment(&mut temp.0[dst], transfer);
            }*/
        }

        /// Calculate wind velocities based on air pressure differences
        fn make_wind(&mut self) {
            const MIN_DIFF: f64 = 0.001;

            let air_pressure = &self.air_pressure;

            for (coord, wind) in self.wind.iter_individual_layers_mut() {
                let this_pressure = air_pressure.0[coord];
                let neighbours = air_pressure
                    .0
                    .wrapping_neighbours_3d(coord)
                    .map(|idx| {
                        let n_value = air_pressure.0[idx];
                        (idx, n_value - this_pressure)
                    })
                    .collect::<ArrayVec<[(usize, f64); grid::NEIGHBOURS_COUNT * 3]>>();

                // find maximum difference
                let (max_diff_idx, max_diff) = *neighbours
                    .iter()
                    .max_by_key(|(_, diff)| OrderedFloat(diff.abs()))
                    .unwrap();

                // TODO diffuse/falloff to neighbouring neighbours too

                let wind_change = if max_diff.abs() < MIN_DIFF {
                    Vector3::zero()
                } else {
                    let tgt = air_pressure.0.unflatten_index(max_diff_idx);
                    Vector3::new(
                        (tgt[0] as isize - coord[0] as isize) as f64,
                        (tgt[1] as isize - coord[1] as isize) as f64,
                        (tgt[2] as isize - coord[2] as isize) as f64,
                    )
                    .normalize_to(max_diff)
                };

                wind.velocity = wind.velocity.lerp(wind_change, 0.1);
            }
        }

        #[deprecated]
        fn move_wind_particle(&mut self) {
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
                        let rot = Basis3::from_angle_z(Rad(rando.gen_range(-FRAC_PI_2, FRAC_PI_2)));
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

        /// Warm surface air rises, so surface pressure decreases.
        fn move_air_vertically(&mut self) {
            let temperature = &mut self.temperature;
            let air_pressure = &mut self.air_pressure;
            let mut temp_rando = thread_rng();
            let distr = Uniform::new(0.05, 0.15);

            for [x, y, z] in PlanetGrid::<()>::iter_layer_coords(AirLayer::Surface, &self.params) {
                let temp = &mut temperature.0[[x, y, z]];
                if *temp > 0.7 {
                    let pressure = &mut air_pressure.0[[x, y, z]];

                    // eprintln!("RISING {:?}", [x,y,z]);
                    let dec = temp_rando.sample(&distr);

                    // rising warm air leaves lower pressure and cooler air below
                    decrement(pressure, dec);
                    decrement(temp, dec);

                    // increases pressure and temperature above
                    let temp_above = &mut temperature.0[[x, y, z + 1]];
                    let pressure_above = &mut air_pressure.0[[x, y, z + 1]];
                    increment(temp_above, dec);
                    increment(pressure_above, dec);
                }
            }

            // TODO cold high air falls?
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
                .iter_layer_mut(AirLayer::Surface)
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

    impl Default for Wind {
        fn default() -> Self {
            Wind {
                velocity: Zero::zero(),
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

    fn point_to_tile(pos: Point3<f32>) -> [isize; 3] {
        [
            pos.x.floor() as isize,
            pos.y.floor() as isize,
            pos.z.floor() as isize,
        ]
    }

    #[inline]
    fn decrement(val: &mut f64, decrement: f64) {
        *val = (*val - decrement).max(0.0);
    }

    #[inline]
    fn increment(val: &mut f64, increment: f64) {
        *val = (*val + increment).min(1.0);
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

        grid.iter_average(AirLayer::Surface, |coord, avg| match coord {
            [0, 0] => assert!(avg.approx_eq(1.0 / PlanetGrid::<f64>::TOTAL_HEIGHT_F, (EPSILON, 2))),
            _ => assert!(avg.approx_eq(0.0, (EPSILON, 2))),
        });

        grid.iter_average(AirLayer::High, |coord, avg| {
            // high air not touched
            assert!(avg.approx_eq(0.0, (EPSILON, 2)));
        });
    }

    //noinspection DuplicatedCode
    #[test]
    fn planet_grid_layers() {
        let mut grid = grid::<i32>(2);

        grid.iter_layer_mut(AirLayer::Surface)
            .for_each(|([x, y, z], val)| {
                assert_eq!(z, 0);
                *val = 1;
                eprintln!("{},{},{}", x, y, z);
            });

        grid.iter_layer_mut(AirLayer::High)
            .for_each(|([x, y, z], val)| {
                assert_eq!(z, 1);
                *val = 5;
                eprintln!(":: {},{},{}", x, y, z);
            });

        grid.iter_layer(AirLayer::Surface)
            .for_each(|(_, val)| assert_eq!(*val, 1));
        grid.iter_layer(AirLayer::High)
            .for_each(|(_, val)| assert_eq!(*val, 5));
    }
}
