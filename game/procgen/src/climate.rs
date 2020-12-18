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
pub struct PlanetGrid<T>(DynamicGrid<T>);

impl<T: Default> PlanetGrid<T> {
    const LAND_DIVISIONS: usize = 4;
    const LAND_DIVISIONS_F: f64 = Self::LAND_DIVISIONS as f64;

    const TOTAL_HEIGHT: usize = Self::LAND_DIVISIONS + 1;
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
        debug_assert!(height >= 0.0 && height <= 1.0);
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
    use common::*;
    use grid::DynamicGrid;
    use std::f64::consts::PI;

    pub struct ClimateIteration<'a> {
        params: PlanetParams,
        rando: &'a mut dyn RngCore,
        continents: &'a ContinentMap,
        step: usize,

        pub(crate) temperature: PlanetGrid<f64>,
    }

    impl<'a> ClimateIteration<'a> {
        pub fn new(
            continents: &'a ContinentMap,
            params: &PlanetParams,
            rando: &'a mut dyn RngCore,
        ) -> Self {
            let size = params.planet_size as usize;
            let mut iter = ClimateIteration {
                params: params.clone(),
                rando,
                continents,
                step: 0,

                temperature: PlanetGrid::new(params),
            };

            iter.init();

            iter
        }

        fn init(&mut self) {
            // set up initial temperature map
            (0..5).for_each(|_| self.apply_sunlight())
        }

        pub fn step(&mut self) {
            debug!("stepping climate simulation"; "step" => self.step);
            self.step += 1;

            // TODO actually do climate things
        }

        // --------

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
