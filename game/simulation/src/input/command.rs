use crate::SocietyHandle;
use unit::world::{WorldPosition, WorldPositionRange};
use world::block::BlockType;

/// Command from the player through the UI
pub enum UiCommand {
    ToggleDebugRenderer { ident: &'static str, enabled: bool },
    FillSelectedTiles(BlockPlacement, BlockType),
    IssueDivineCommand(DivineInputCommand),
    IssueSocietyCommand(SocietyHandle, SocietyInputCommand),
}

// TODO just use a dyn Job instead of redefining jobs as an identical enum?
pub enum SocietyInputCommand {
    BreakBlocks(WorldPositionRange),
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
