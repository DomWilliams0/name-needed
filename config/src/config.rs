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
    pub fov: f32,
    pub camera_turn_multiplier: f32,
}

#[derive(Deserialize)]
pub struct World {
    pub preset: WorldPreset,
}

#[derive(Deserialize)]
pub enum WorldPreset {
    OneChunkWonder,
    MultiChunkWonder,
    OneBlockWonder,
    FlatLands,
}

#[derive(Deserialize)]
pub struct Simulation {
    pub initial_entities: Vec<EntityDescriptor>,
    pub random_count: u32,
    pub move_speed: f32,
}

#[derive(Deserialize)]
pub struct EntityDescriptor {
    pub pos: (i32, i32, Option<i32>),
    pub color: (u8, u8, u8),
    pub size: (f32, f32, f32),
}