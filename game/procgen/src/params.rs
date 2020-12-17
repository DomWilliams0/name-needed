use common::{thread_rng, Rng};
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct PlanetParams {
    /// Random if not specified
    #[structopt(long)]
    seed: Option<u64>,

    /// Height and width of surface in some unit
    #[structopt(long, default_value = "128")]
    pub planet_size: u32,

    #[structopt(long, default_value = "6")]
    pub max_continents: usize,

    #[cfg(feature = "bin")]
    #[structopt(flatten)]
    pub render: RenderParams,
}

#[derive(Debug, Clone, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct RenderParams {
    #[structopt(long)]
    draw_debug_colours: bool,

    #[structopt(long)]
    draw_blob_outlines: bool,

    #[structopt(long)]
    draw_density: bool,
}

pub enum DrawMode {
    Outlines { debug_colors: bool, outlines: bool },
    Height,
}

impl PlanetParams {
    pub fn load() -> Self {
        // TODO from file too and merge
        let mut params = Self::from_args();

        // generate random seed
        if params.seed.is_none() {
            params.seed = Some(thread_rng().gen())
        }

        params
    }

    pub fn seed(&self) -> u64 {
        self.seed.expect("seed should have been initialized")
    }

    pub fn draw_mode(&self) -> DrawMode {
        if self.render.draw_density {
            DrawMode::Height
        } else {
            DrawMode::Outlines {
                debug_colors: self.render.draw_debug_colours,
                outlines: self.render.draw_blob_outlines,
            }
        }
    }
}
