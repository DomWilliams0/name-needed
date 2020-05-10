use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub display: Display,
    pub world: World,
    pub simulation: Simulation,
}

#[derive(Deserialize)]
pub struct Display {
    pub resolution: (u32, u32),
    pub camera_speed: f32,
    pub debug_physics: bool,
}

#[derive(Deserialize)]
pub struct World {
    pub preset: WorldPreset,
    pub worker_threads: Option<usize>,
}

#[derive(Deserialize)]
pub enum WorldPreset {
    OneChunkWonder,
    MultiChunkWonder,
    OneBlockWonder,
    FlatLands,
    PyramidMess,
    Bottleneck,
}

#[derive(Deserialize)]
pub struct Simulation {
    pub random_seed: Option<u64>,
    pub random_count: u32,
    pub acceleration: f32,
    pub max_speed: f32,
    pub friction: f32,
    pub start_delay: u32,
}
