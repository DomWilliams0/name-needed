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

        debug!("processing final state");
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
    use crate::{map_range, PlanetParams};
    use common::cgmath::prelude::*;
    use common::cgmath::{Point3, Vector3};
    use common::*;
    use grid::{DynamicGrid, NEIGHBOURS_COUNT};
    use rand_distr::Uniform;

    use line_drawing::Bresenham3d;
    use std::f64::consts::{PI, TAU};

    pub struct ClimateIteration<'a> {
        params: PlanetParams,
        rando: &'a mut dyn RngCore,
        continents: &'a ContinentMap,
        step: usize,

        pub(crate) temperature: PlanetGrid<f64>,
        pub(crate) wind: PlanetGrid<Wind>,
        pub(crate) air_pressure: PlanetGrid<f64>,

        pub(crate) height_gradient: DynamicGrid<[f64; NEIGHBOURS_COUNT]>,
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
                height_gradient: DynamicGrid::new(params.planet_dims(1)),
            };

            iter.init();

            iter
        }

        fn init(&mut self) {
            // calculate height gradients
            for (coord, tile) in self.continents.grid.iter_coords() {
                let gradient = &mut self.height_gradient[coord];
                let height = tile.land_height();
                for (i, (n, _)) in self.continents.grid.wrapping_neighbours(coord).enumerate() {
                    let n_height = self.continents.grid[n].land_height();
                    let diff = n_height - height;
                    gradient[i] = diff;
                }
            }

            // set up initial temperature map
            (0..5).for_each(|_| self.apply_sunlight());

            // set up initial air pressure
            let mut pressure_rando = thread_rng();
            let surface_distrs: [_; LAND_DIVISIONS] = [
                Uniform::new(0.9, 0.98), // lowest land
                Uniform::new(0.8, 0.9),
                Uniform::new(0.7, 0.8),
                Uniform::new(0.6, 0.7), // highest land
            ];
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

            macro_rules! every {
                ($n:expr) => {
                    self.step % $n == 0
                };
            }

            if every!(3) {
                self.apply_sunlight();
            }

            self.move_air_vertically();

            self.make_wind();

            for _ in 0..3 {
                self.propagate_wind();
            }

            self.apply_wind();
            // TODO wind movingbrings air to level out pressure

            // TODO wind is not being affected by terrain at all
            // TODO wind is getting stuck low down and not rising

            self.step += 1;
        }

        // --------

        fn propagate_wind(&mut self) {
            // TODO reuse alloc
            let mut new_vals = PlanetGrid::<Wind>::new(&self.params);
            let wind_speed_modifier = self.params.wind_speed_modifier;
            let wind_speed_base = self.params.wind_speed_base;
            let dir_conform = self.params.wind_direction_conformity;

            let terrain = &self.height_gradient;
            for (coord, wind) in self.wind.iter_individual_layers() {
                // speed up when going downhill or flat, slow down going up
                let land_gradient = {
                    let angle = wind.velocity.angle(Vector3::unit_y()).normalize();
                    let div = map_range((0.0, TAU), (0.0, 7.0), angle.0) as usize;
                    terrain[coord][div]
                };

                let speed_modifier = {
                    // increase steepness of gradients
                    let gradient = land_gradient * wind_speed_modifier;

                    wind_speed_base - gradient
                };
                debug_assert!(speed_modifier.is_sign_positive());

                let new_vel = wind.velocity * speed_modifier;
                new_vals.0[coord].velocity = wind.velocity.lerp(new_vel, dir_conform);
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
            let air = &mut self.air_pressure;
            let temp = &mut self.temperature;
            for (coord, wind) in self.wind.iter_individual_layers_mut() {
                // TODO distribute across neighbours more smoothly, advection?

                #[cfg(debug_assertions)]
                wind.validate(coord);

                let wind_mag = wind.velocity.magnitude2();
                if wind_mag < 1.5_f64 {
                    // too weak
                    continue;
                }

                let start = (coord[0] as isize, coord[1] as isize, coord[2] as isize);
                let end = (
                    start.0 + wind.velocity.x.round() as isize,
                    start.1 + wind.velocity.y.round() as isize,
                    start.2 + wind.velocity.z.round() as isize,
                );

                for (x, y, z) in Bresenham3d::new(start, end) {
                    let dst = air.0.wrap_coord([x, y, z]);

                    // transfer
                    // TODO if too big (>0.01) we end up with little pockets of unchanging high pressure :(
                    let transfer = wind_mag * self.params.wind_transfer_rate;
                    decrement(&mut air.0[coord], transfer);
                    increment(&mut air.0[dst], transfer);

                    decrement(&mut temp.0[coord], transfer);
                    increment(&mut temp.0[dst], transfer);
                }
            }
        }

        /// Calculate wind velocities based on air pressure differences
        fn make_wind(&mut self) {
            let air_pressure = &self.air_pressure;
            let threshold = self.params.wind_pressure_threshold;
            let planet_size = self.params.planet_size as isize;
            let dir_conform = self.params.wind_direction_conformity;
            let wrap_threshold = (planet_size as f64 * 0.8).powi(2);
            let limit = 16.min(planet_size) as usize;

            macro_rules! mk_point {
                ($array:expr) => {{
                    let [x, y, z] = $array;
                    Point3::new(x as f64, y as f64, z as f64)
                }};
            }

            for (coord, wind) in self.wind.iter_individual_layers_mut() {
                let mut explore_state = Some((
                    air_pressure.0.flatten_coords(coord),
                    [coord[0] as isize, coord[1] as isize, coord[2] as isize],
                ));

                let explore = std::iter::from_fn(|| {
                    if let Some((idx, orig_coord)) = explore_state {
                        let this_pressure = air_pressure.0[idx];
                        explore_state = air_pressure
                            .0
                            .wrapping_neighbours_3d(coord)
                            .flat_map(|(n, orig)| {
                                if (air_pressure.0[n] - this_pressure) <= -threshold {
                                    // air pressure diff is big enough to create wind
                                    Some((n, orig, OrderedFloat(air_pressure.0[n] - this_pressure)))
                                } else {
                                    None
                                }
                            })
                            .min_by_key(|(_, _, diff)| *diff)
                            .map(|(n_idx, n_orig, _)| {
                                // update cumulative location
                                let new_relative_coord = [
                                    orig_coord[0] + (n_orig[0] - coord[0] as isize),
                                    orig_coord[1] + (n_orig[1] - coord[1] as isize),
                                    orig_coord[2] + (n_orig[2] - coord[2] as isize),
                                ];

                                (n_idx, new_relative_coord)
                            });
                    }

                    explore_state
                })
                .take(limit);

                let new_wind = match explore.last() {
                    Some((_, orig)) => {
                        let src = mk_point!(coord);

                        let vec = mk_point!(orig) - src;
                        debug_assert!(vec.magnitude2() < wrap_threshold, "{:?}", vec);
                        vec
                    }
                    _ => Vector3::zero(),
                };

                wind.velocity = wind.velocity.lerp(new_wind, dir_conform);
            }
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
            let sunlight_max = self.params.sunlight_max;
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

                if *temp < sunlight_max {
                    *temp = (*temp + increase).min(sunlight_max);
                }
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

    impl Wind {
        fn validate(&self, coord: [usize; 3]) {
            let check = |f: f64| {
                assert!(
                    !f.is_nan() && !f.is_infinite(),
                    "bad velocity at {:?}: {:?}",
                    coord,
                    self.velocity
                )
            };
            check(self.velocity.x);
            check(self.velocity.y);
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
