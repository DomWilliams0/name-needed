use unit::world::{WorldPoint, WorldPosition};
use world::{InnerWorldRef, SliceRange};

#[derive(Debug)]
pub struct WorldColumn {
    pub x: f32,
    pub y: f32,
    pub slice_range: SliceRange,
}

#[derive(Debug)]
pub enum InputEvent {
    LeftClick(WorldColumn),
    RightClick(WorldColumn),
}

impl WorldColumn {
    pub fn find_highest_walkable(&self, world: &InnerWorldRef) -> Option<WorldPoint> {
        let block = WorldPosition(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.slice_range.top(),
        );
        world
            .find_accessible_block_in_column_with_range(block, Some(self.slice_range.bottom()))
            .map(|WorldPosition(_, _, z)| {
                // only use z from result, keep input precision
                WorldPoint(self.x, self.y, z.slice() as f32)
            })
    }
}
