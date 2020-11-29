use serde::Deserialize;
use std::collections::HashMap;

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
    pub debug_physics: bool,
    pub zoom: f32,
    pub initial_view_range: u16,
    pub nav_paths_by_default: bool,
}

#[derive(Deserialize)]
pub struct World {
    pub source: WorldSource,
    pub worker_threads: Option<usize>,
    pub generation_height_scale: f64,
}

#[derive(Deserialize, Clone)]
pub enum WorldSource {
    Preset(WorldPreset),
    Generate { radius: u32, seed: Option<u64> },
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
}
