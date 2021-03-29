use common::*;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use strum_macros::{EnumIter, EnumString};

use crate::RegionLocation;
use common::alloc::str::FromStr;
use noise::MultiFractal;
#[cfg(feature = "cache")]
use serde::Serialize;

#[derive(Debug, Clone, StructOpt)]
#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
#[structopt(rename_all = "kebab-case")]
pub struct PlanetParams {
    /// Random if not specified
    #[structopt(long)]
    seed: Option<u64>,

    /// Height and width of surface in some unit
    #[structopt(long, default_value = "128")]
    pub planet_size: u32,

    #[structopt(long, default_value = "5")]
    pub max_continents: usize,

    #[structopt(long, default_value = "10.0")]
    pub continent_start_radius: f32,

    #[structopt(long, default_value = "0.1")]
    pub continent_dec_min: f32,

    #[structopt(long, default_value = "0.3")]
    pub continent_dec_max: f32,

    #[structopt(long, default_value = "20")]
    pub continent_min_distance: i32,

    #[structopt(long, default_value = "0.2")]
    pub continent_polygon_epsilon: f64,

    #[structopt(long, default_value = "5")]
    pub climate_iterations: usize,

    #[structopt(long, default_value = "20")]
    pub wind_particles: usize,

    #[structopt(long, default_value = "0.005")]
    pub wind_transfer_rate: f64,

    #[structopt(long, default_value = "0.05")]
    pub wind_pressure_threshold: f64,

    #[structopt(long, default_value = "2.0")]
    pub wind_speed_modifier: f64,

    #[structopt(long, default_value = "1.2")]
    pub wind_speed_base: f64,

    #[structopt(long, default_value = "0.3")]
    pub wind_direction_conformity: f64,

    #[structopt(long, default_value = "0.8")]
    pub sunlight_max: f64,

    #[cfg(feature = "bin")]
    #[structopt(flatten)]
    pub render: RenderParams,

    #[structopt(long)]
    pub log_params_and_exit: bool,

    #[structopt(long, default_value = "NoiseParams::default()")]
    pub height_noise: NoiseParams,

    #[structopt(long, default_value = "NoiseParams::default()")]
    pub moisture_noise: NoiseParams,

    #[structopt(long, default_value = "NoiseParams::default()")]
    pub temp_noise: NoiseParams,

    #[structopt(long, default_value = "2.0")]
    pub coastline_thickness: f64,

    #[structopt(long, parse(try_from_str), default_value)]
    pub no_cache: bool,

    /// Set manually to "biomes.ron" as sibling to this file during loading
    #[structopt(skip)]
    pub biomes_cfg: BiomesConfig,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
pub enum BiomesConfig {
    // No overhead in non-test builds: "Data-carrying enums with a single variant without a repr()
    // annotation have the same layout as the variant field."
    File(PathBuf),

    #[cfg(test)]
    Hardcoded(String),
}

#[derive(Debug, Clone, Default, StructOpt)]
#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
pub struct NoiseParams {
    pub octaves: Option<usize>,
    pub freq: Option<f64>,
    pub persistence: Option<f64>,
    pub lacunarity: Option<f64>,
}

#[derive(Debug, Copy, Clone, EnumString, Deserialize, EnumIter, PartialEq, Eq)]
#[cfg_attr(feature = "cache", derive(Serialize))]
#[strum(serialize_all = "kebab-case")]
pub enum RenderOverlay {
    Moisture,

    #[strum(serialize = "temp")]
    Temperature,

    Elevation,
}

#[derive(Debug, Copy, Clone, EnumString, Deserialize, EnumIter, PartialEq, Eq)]
#[cfg_attr(feature = "cache", derive(Serialize))]
#[strum(serialize_all = "kebab-case")]
pub enum RenderProgressParams {
    #[strum(serialize = "temp")]
    Temperature,

    Wind,

    #[strum(serialize = "pressure")]
    AirPressure,
}

#[derive(Debug, Copy, Clone, EnumString, Deserialize, EnumIter, Eq, PartialEq)]
#[cfg_attr(feature = "cache", derive(Serialize))]
#[strum(serialize_all = "kebab-case")]
pub enum AirLayer {
    Surface,
    High,
}

#[derive(Debug, Clone, StructOpt, Deserialize)]
#[cfg_attr(feature = "cache", derive(Serialize))]
#[structopt(rename_all = "kebab-case")]
pub struct RenderParams {
    #[structopt(long)]
    pub draw_biomes: bool,

    #[structopt(long)]
    pub draw_overlay: Option<RenderOverlay>,

    #[structopt(long, default_value = "150")]
    pub overlay_alpha: u8,

    #[structopt(long)]
    pub draw_continent_polygons: bool,

    #[structopt(long, default_value = "temp")]
    pub gif_progress: RenderProgressParams,

    #[structopt(long)]
    pub create_climate_gif: bool,

    #[structopt(long, default_value = "surface")]
    pub gif_layer: AirLayer,

    #[structopt(long, default_value = "4")]
    pub gif_threads: usize,

    #[structopt(long, default_value = "4")]
    pub gif_fps: u32,

    /// Image scale per axis
    #[structopt(long, default_value = "2")]
    pub scale: u32,

    /// Per axis
    #[structopt(long, default_value = "1")]
    pub zoom: u32,

    #[structopt(long, default_value = "5")]
    pub region_start_slab: i32,

    #[structopt(long, default_value = "10")]
    pub region_max_depth: i32,

    #[structopt(long, default_value = "4")]
    pub threads: usize,
}

impl PlanetParams {
    pub fn load_with_args(file_path: impl AsRef<Path>) -> BoxedResult<Self> {
        Self::load(file_path.as_ref(), std::env::args())
    }

    pub fn load_with_only_file(file_path: impl AsRef<Path>) -> BoxedResult<Self> {
        let fake_args = once(env!("CARGO_PKG_NAME").to_owned());
        Self::load(file_path.as_ref(), fake_args)
    }

    // TODO return a result instead of panicking
    /// Must be at least len 1, where first elem is binary name
    fn load(file_path: &Path, mut args: impl Iterator<Item = String>) -> BoxedResult<Self> {
        let mut params = {
            let binary_name = args.next().expect("no 0th arg");
            let mut config_params = vec![binary_name];

            match File::open(file_path) {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // no file, no problem
                    warn!(
                        "couldn't find config file at '{}', continuing with defaults",
                        file_path.display()
                    );
                }
                Err(e) => panic!("failed to read config file: {}", e),
                Ok(file) => {
                    let lines = BufReader::new(file);
                    for line in lines.lines().filter_map(|line| line.ok()).filter(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    }) {
                        config_params.extend(line.split(' ').map(str::to_owned));
                    }
                }
            };

            // binary name || args from file || args from cmdline
            // TODO clap AppSettings::AllArgsOverrideSelf
            Self::from_iter_safe(config_params.into_iter().chain(args))?
        };

        // generate random seed
        if params.seed.is_none() {
            params.seed = Some(thread_rng().gen())
        }

        params.biomes_cfg = {
            let path = file_path.parent().unwrap(); // definitely a file by this point
            BiomesConfig::File(path.join("biomes.ron"))
        };

        Ok(params)
    }

    #[cfg(test)]
    pub fn dummy_with_biomes(biomes: String) -> Self {
        let mut params = Self::from_iter_safe(once("dummy")).expect("failed");
        params.biomes_cfg = BiomesConfig::Hardcoded(biomes);
        params
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        Self::dummy_with_biomes(
            r#"[ (biome: Plains, color: 0x84e065, elevation: (10, 18), sampling: ()) ]"#.to_owned(),
        )
    }

    pub fn seed(&self) -> u64 {
        self.seed.expect("seed should have been initialized")
    }

    pub fn planet_dims(&self, height: usize) -> [usize; 3] {
        [self.planet_size as usize, self.planet_size as usize, height]
    }

    pub fn is_region_in_range(&self, region: RegionLocation) -> bool {
        region.0 < self.planet_size && region.1 < self.planet_size
    }
}

impl std::str::FromStr for NoiseParams {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut this = NoiseParams::default();

        fn parse_val<T: FromStr>(str: &str) -> Result<Option<T>, T::Err> {
            if str.is_empty() {
                Ok(None)
            } else {
                Ok(Some(str.parse()?))
            }
        }

        // default check lmao
        if s != "NoiseParams::default()" {
            for (idx, int) in s.split(',').enumerate() {
                match idx {
                    0 => this.octaves = parse_val(int)?,
                    1 => this.freq = parse_val(int)?,
                    2 => this.persistence = parse_val(int)?,
                    3 => this.lacunarity = parse_val(int)?,
                    _ => return Err("too many fields".into()),
                }
            }
        }

        Ok(this)
    }
}

impl NoiseParams {
    pub fn configure<F: MultiFractal>(&self, mut noise: F) -> F {
        if let Some(val) = self.freq {
            noise = noise.set_frequency(val);
        }

        if let Some(val) = self.octaves {
            noise = noise.set_octaves(val);
        }

        if let Some(val) = self.persistence {
            noise = noise.set_persistence(val);
        }

        if let Some(val) = self.lacunarity {
            noise = noise.set_lacunarity(val);
        }

        noise
    }
}

impl Default for BiomesConfig {
    fn default() -> Self {
        Self::File(Default::default())
    }
}
