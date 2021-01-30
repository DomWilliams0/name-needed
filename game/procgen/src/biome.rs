use crate::continent::ContinentMap;
use crate::{map_range, PlanetParams};
use common::*;
use noise::{Fbm, MultiFractal, NoiseFn, Point4, Seedable};
use std::f64::consts::{PI, TAU};

pub struct BiomeSampler {
    latitude_coefficient: f64,

    height: Noise<Fbm>,
    temperature: Noise<Fbm>,
    moisture: Noise<Fbm>,
}

#[derive(Debug)]
pub enum Biome {
    Ocean,
    IcyOcean,
    CoastOcean,
    Beach,
    Plains,
    Forest,
    RainForest,
    Desert,
    Tundra,
}

/// Noise generator with its rough limits
struct Noise<N: NoiseFn<Point4<f64>>> {
    noise: N,
    limits: (f64, f64),
    planet_size: f64,
}

impl BiomeSampler {
    pub fn new(rando: &mut dyn RngCore, params: &PlanetParams) -> Self {
        let latitude_coefficient = PI / params.planet_size as f64;

        let height = Noise::new(
            params
                .height_noise
                .configure(Fbm::new().set_seed(rando.gen())),
            params,
            "height",
        );
        let temperature = Noise::new(
            params
                .temp_noise
                .configure(Fbm::new().set_seed(rando.gen())),
            params,
            "temperature",
        );
        let moisture = Noise::new(
            params
                .moisture_noise
                .configure(Fbm::new().set_seed(rando.gen())),
            params,
            "moisture",
        );

        Self {
            latitude_coefficient,
            height,
            temperature,
            moisture,
        }
    }

    /// (coastline_proximity, elevation, moisture, temperature)
    pub fn sample(&self, pos: (f64, f64), continents: &ContinentMap) -> (f64, f64, f64, f64) {
        let coastline_proximity = continents.coastline_proximity(pos);
        let elevation = self.elevation(pos, coastline_proximity);
        let moisture = self.moisture(pos, coastline_proximity);
        let temperature = self.temperature(pos, elevation);

        (coastline_proximity, elevation, moisture, temperature)
    }

    pub fn sample_biome(&self, pos: (f64, f64), continents: &ContinentMap) -> Biome {
        let (coastline_proximity, elevation, moisture, temperature) = self.sample(pos, continents);
        Biome::map(coastline_proximity, elevation, temperature, moisture)
    }

    // -------

    fn moisture(&self, pos: (f64, f64), coastline_proximity: f64) -> f64 {
        let raw_moisture = self.moisture.sample_wrapped_normalized(pos);

        if coastline_proximity < 0.0 {
            // in the ocean, i think it's very wet
            return 1.0;
        }

        // moister closer to the sea
        let mul = map_range((0.0, 1.0), (0.8, 1.2), 1.0 - coastline_proximity);

        // less moist at equator from the heat, but dont increase moisture at poles any more
        let latitude = map_range((0.0, 1.0), (0.8, 1.2), 1.0 - self.latitude_mul(pos.1)).min(1.0);

        raw_moisture * mul * latitude
    }

    fn temperature(&self, (x, y): (f64, f64), elevation: f64) -> f64 {
        let latitude = self.latitude_mul(y);
        let raw_temp = self.temperature.sample_wrapped_normalized((x, y));

        // TODO elevation is negative sometimes at the coasts?

        // average sum of:
        //  - latitude: lower at poles, higher at equator
        //  - elevation: lower by sea, higher in-land
        //  - raw noise: 0-1
        (raw_temp * 0.25) + (elevation * 0.25) + (latitude * 0.5)
    }

    fn elevation(&self, pos: (f64, f64), coastline_proximity: f64) -> f64 {
        // sample height map in normalized range
        let raw_height = self.height.sample_wrapped_normalized(pos);

        // coastline tends toward 0 i.e. sea level
        if coastline_proximity >= 0.0 {
            raw_height * coastline_proximity
        } else {
            // underwater
            // TODO treat negative elevation as normal heightmap underwater
            0.0
        }
    }

    /// 0 at poles, 1 at equator
    fn latitude_mul(&self, y: f64) -> f64 {
        (y * self.latitude_coefficient).sin()
    }
}

impl Biome {
    fn map(coast_proximity: f64, _elevation: f64, temperature: f64, moisture: f64) -> Self {
        use Biome::*;
        // TODO 3d nearest neighbour into biome space instead of this noddy lookup

        if coast_proximity < 0.0 {
            return if temperature < 0.2 {
                IcyOcean
            } else if coast_proximity > -0.3 {
                CoastOcean
            } else {
                Ocean
            };
        }

        if coast_proximity < 0.2 && temperature > 0.3 {
            return Beach;
        }

        if temperature < 0.3 {
            Tundra
        } else if temperature < 0.75 {
            if moisture < 0.45 {
                Plains
            } else {
                Forest
            }
        } else {
            // hot
            if moisture < 0.7 {
                Desert
            } else {
                RainForest
            }
        }
    }
}

impl<N: NoiseFn<Point4<f64>>> Noise<N> {
    fn new(noise: N, params: &PlanetParams, what: &str) -> Self {
        let mut this = Noise {
            noise,
            limits: (f64::MIN, f64::MAX), // placeholders
            planet_size: params.planet_size as f64,
        };

        let limits = {
            let (mut min, mut max) = (1.0, 0.0);
            let mut r = thread_rng();
            let iterations = 10_000;
            let buffer = 0.25;

            trace!("finding generator limits"; "iterations" => iterations, "generator" => what);

            for _ in 0..iterations {
                let f = this.sample_wrapped((
                    r.gen_range(-this.planet_size, this.planet_size),
                    r.gen_range(-this.planet_size, this.planet_size),
                ));
                min = f.min(min);
                max = f.max(max);
            }

            debug!(
                "'{generator}' generator limits are {min:?} -> {max:?}",
                min = min - buffer,
                max = max + buffer,
                generator = what,
            );

            (min, max)
        };

        this.limits = limits;
        this
    }

    /// Produces seamlessly wrapping noise
    fn sample_wrapped(&self, (x, y): (f64, f64)) -> f64 {
        // thanks https://www.gamasutra.com/blogs/JonGallant/20160201/264587/Procedurally_Generating_Wrapping_World_Maps_in_Unity_C__Part_2.php

        // noise range
        let x1 = 0.0;
        let x2 = 2.0;
        let y1 = 0.0;
        let y2 = 2.0;
        let dx = x2 - x1;
        let dy = y2 - y1;

        // sample at smaller intervals
        let s = x / self.planet_size;
        let t = y / self.planet_size;

        // get 4d noise
        let nx = x1 + (s * TAU).cos() * dx / TAU;
        let ny = y1 + (t * TAU).cos() * dy / TAU;
        let nz = x1 + (s * TAU).sin() * dx / TAU;
        let nw = y1 + (t * TAU).sin() * dy / TAU;

        let value = self.noise.get([nx, ny, nz, nw]);
        // debug_assert!(
        //     (self.limits.0..self.limits.1).contains(&value),
        //     "noise limits are wrong (value={:?}, limits={:?} -> {:?})",
        //     value,
        //     self.limits.0,
        //     self.limits.1,
        // );
        value
    }

    /// Produces seamlessly wrapping noise scaled from 0-1 by limits of this generator
    fn sample_wrapped_normalized(&self, pos: (f64, f64)) -> f64 {
        let value = self.sample_wrapped(pos);
        map_range(self.limits, (0.0, 1.0), value)
    }
}
