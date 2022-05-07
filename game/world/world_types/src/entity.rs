use std::borrow::Cow;
use unit::world::WorldPoint;

/// Describes an entity to spawn as part of world generation
pub struct EntityDescription {
    pub position: WorldPoint,

    // TODO trait object for other types
    pub desc: PlantDescription,
}

pub struct PlantDescription {
    pub species: Cow<'static, str>,
    // TODO species, initial growth progress, initial state (dehydrated, blooming, etc)
}

// TODO tree entity: list of blocks for trunk/roots/leaves, age, height, species
