use common::*;
use serde::Deserialize;

use std::io::BufRead;

use structopt::StructOpt;
use strum_macros::{EnumIter, EnumString};

use crate::biome::BiomeConfig;
use crate::region::RegionLocationUnspecialized;
use common::alloc::str::FromStr;
use noise::MultiFractal;
use resources::{ReadResource, ResourceContainer, ResourceError, ResourceErrorKind, ResourceFile};
#[cfg(feature = "cache")]
use serde::Serialize;
use std::sync::Arc;
use unit::world::ChunkLocation;

pub type PlanetParamsRef = Arc<PlanetParams>;

#[derive(Debug, StructOpt)]
#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
#[structopt(rename_all = "kebab-case")]
pub struct PlanetParams {
    /// Random if not specified
    #[structopt(long)]
    seed: Option<u64>,
    // TODO remove overhead of option and default to 0
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

    /// Manually set after parsing arguments by reading a sibling file
    #[structopt(skip)]
    pub(crate) biomes_cfg: Vec<BiomeConfig>,

    /// The higher >1 the more relaxed the boundary
    #[structopt(long, default_value = "8.0")]
    pub feature_concavity: f64,

    /// Block radius for forest poisson disk sampling
    #[structopt(long, default_value = "8")]
    pub forest_pds_radius: u32,

    /// Max attempts to place a tree in forest poisson disk sampling
    #[structopt(long, default_value = "15")]
    pub forest_pds_attempts: u32,

    /// Blocks to expand regional feature boundary
    #[structopt(long, default_value = "2")]
    pub region_feature_expansion: u32,

    #[structopt(long, default_value = "0.15")]
    pub region_feature_vertical_expansion_threshold: f64,
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
    /// File path on disk
    #[cfg(feature = "bin")]
    pub fn load_file_with_args(config_path: impl AsRef<std::path::Path>) -> BoxedResult<PlanetParamsRef> {
        use std::path::Path;
        use std::io::ErrorKind;

        let read_file = |path: &Path| -> std::io::Result<String> {
            match std::fs::read_to_string(path) {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // no file, no problem
                    warn!(
                        "couldn't find config file '{}', continuing with defaults",
                        path.display()
                    );

                    Ok(String::new())
                }
                other => other,
            }
        };

        let cfg = read_file(config_path.as_ref())?;
        let biomes = read_file("biomes.ron".as_ref())?;

        Self::load(&cfg, &biomes, std::env::args())
    }

    /// path is relative to resource container. Expects "biomes.ron" in same directory
    pub fn load_with_only_file(
        resources: &impl ResourceContainer,
        path: impl AsRef<ResourceFile>,
    ) -> BoxedResult<PlanetParamsRef> {
        let read_resource = |path: &ResourceFile| -> BoxedResult<String> {
            match resources.get_file(path) {
                Ok(path) => String::read_resource(path).map_err(Into::into),
                Err(ResourceError(_, ResourceErrorKind::FileNotFound)) => {
                    // no file, no problem
                    warn!(
                        "couldn't find config file {:?}, continuing with defaults",
                        path
                    );

                    Ok(String::new())
                }
                Err(err) => Err(err.into()),
            }
        };

        let cfg = read_resource(path.as_ref())?;
        let biomes = read_resource("biomes.ron".as_ref())?;

        let fake_args = once(env!("CARGO_PKG_NAME").to_owned());
        Self::load(&cfg, &biomes, fake_args)
    }

    // TODO return a result instead of panicking
    /// Args must be at least len 1, where first elem is binary name
    fn load(
        cfg: &str,
        biomes_cfg: &str,
        mut args: impl Iterator<Item = String>,
    ) -> BoxedResult<PlanetParamsRef> {
        let mut params = {
            let binary_name = args.next().expect("no 0th arg");
            let mut config_params = vec![binary_name];

            for line in cfg.lines().filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            }) {
                config_params.extend(line.split(' ').map(str::to_owned));
            }

            // binary name || args from file || args from cmdline
            // TODO clap AppSettings::AllArgsOverrideSelf
            Self::from_iter_safe(config_params.into_iter().chain(args))?
        };

        // generate random seed
        if params.seed.is_none() {
            params.seed = Some(thread_rng().gen())
        }

        // parse biomes file
        params.biomes_cfg = ron::de::from_str(biomes_cfg)?;

        Ok(PlanetParamsRef::new(params))
    }

    #[cfg(any(test, feature = "benchmarking"))]
    pub fn dummy_with_biomes(biomes: String) -> PlanetParamsRef {
        let mut params = Self::from_iter_safe(once("dummy")).expect("failed");
        params.biomes_cfg = ron::de::from_str(&biomes).expect("bad biomes");
        PlanetParamsRef::new(params)
    }

    #[cfg(any(test, feature = "benchmarking"))]
    pub fn dummy() -> PlanetParamsRef {
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

    pub fn is_region_in_range<const SIZE: usize>(
        &self,
        region: RegionLocationUnspecialized<SIZE>,
    ) -> bool {
        let (x, y) = region.xy();
        x < self.planet_size && y < self.planet_size
    }

    pub fn is_chunk_in_range(&self, chunk: ChunkLocation) -> bool {
        crate::region::RegionLocation::try_from_chunk_with_params(chunk, self).is_some()
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
