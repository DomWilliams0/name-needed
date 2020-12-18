pub use crate::climate::iteration::ClimateIteration;
use crate::continent::ContinentMap;
use crate::PlanetParams;
use common::*;

pub struct Climate {}

impl Climate {
    pub fn simulate(
        continents: &ContinentMap,
        params: &PlanetParams,
        rando: &mut dyn RngCore,
        mut per_step: impl FnMut(&ClimateIteration),
    ) -> Self {
        let mut iter = ClimateIteration::new(continents, params, rando);

        for _ in 0..5 {
            per_step(&iter);
            iter.step();
        }

        per_step(&iter);

        Climate {}
    }
}

mod iteration {
    use crate::continent::ContinentMap;
    use crate::PlanetParams;
    use common::*;
    use grid::DynamicGrid;
    use std::f64::consts::PI;

    pub struct ClimateIteration<'a> {
        params: PlanetParams,
        rando: &'a mut dyn RngCore,
        continents: &'a ContinentMap,

        pub(crate) temperature: DynamicGrid<f64>,
        step: usize,
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

                temperature: DynamicGrid::new([size, size, 1]),
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

            for ((coord, temp), tile) in self
                .temperature
                .iter_coords_mut()
                .zip(self.continents.grid.iter())
            {
                let increase = {
                    // land warms faster than sea
                    let base_increase = if tile.is_land() { 0.05 } else { 0.01 };

                    // 0 at poles, 1 at equator
                    let latitude_multiplier = (coord[1] as f64 * latitude_coefficient).sin();

                    base_increase * latitude_multiplier
                };

                *temp = (*temp + increase).min(1.0);
            }
        }
    }
}
