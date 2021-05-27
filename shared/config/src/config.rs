use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Config {
    pub display: Display,
    pub world: World,
    pub simulation: Simulation,
}

#[derive(Deserialize)]
pub struct Display {
    pub resolution: (u32, u32),
    pub resizable: bool,
    pub camera_speed: f32,
    pub zoom: f32,
    pub initial_view_range: u16,
    pub persist_ui: bool,
}

#[derive(Deserialize)]
pub struct World {
    pub source: WorldSource,
    /// Seconds
    pub load_timeout: u32,
    pub worker_threads: Option<usize>,
    pub initial_chunk: (i32, i32),
    pub initial_slab_depth: u32,
    pub initial_chunk_radius: u32,
}

#[derive(Deserialize, Clone)]
pub enum WorldSource {
    Preset(WorldPreset),
    /// Generate world using the given resource file in worldgen resources for options
    Generate(PathBuf),
}

#[derive(Deserialize, Clone, Debug)]
pub enum WorldPreset {
    OneChunkWonder,
    MultiChunkWonder,
    OneBlockWonder,
    FlatLands,
    Bottleneck,
    Stairs,
}

#[derive(Deserialize)]
pub struct Simulation {
    pub random_seed: Option<u64>,
    pub friction: f32,
    pub start_delay: u32,
    pub spawn_counts: HashMap<String, usize>,
    pub entity_logging_by_default: bool,
    /// Ring buffer size
    pub entity_logging_capacity: usize,
}

impl WorldSource {
    pub fn is_preset(&self) -> bool {
        matches!(self, Self::Preset(_))
    }
}
