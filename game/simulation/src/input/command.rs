use unit::world::WorldPosition;
use world::block::BlockType;

/// Command from the player through the UI
pub enum InputCommand {
    ToggleDebugRenderer { ident: &'static str, enabled: bool },
    FillSelectedTiles(BlockPlacement, BlockType),
    IssueDivineCommand(DivineInputCommand),
}

pub enum DivineInputCommand {
    Goto(WorldPosition),
    Break(WorldPosition),
}

#[derive(Copy, Clone, PartialEq)]
pub enum BlockPlacement {
    Set,
    PlaceAbove,
}
