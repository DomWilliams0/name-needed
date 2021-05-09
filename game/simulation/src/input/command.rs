use unit::world::WorldPosition;
use world::block::BlockType;

use crate::ecs::Entity;
use crate::society::job::SocietyCommand;
use crate::{Exit, SocietyHandle};
use std::path::PathBuf;

/// Command from the player through the UI
pub enum UiCommand {
    ExitGame(Exit),

    ToggleDebugRenderer {
        ident: &'static str,
        enabled: bool,
    },

    FillSelectedTiles(BlockPlacement, BlockType),

    IssueDivineCommand(DivineInputCommand),

    IssueSocietyCommand(SocietyHandle, SocietyCommand),

    SetContainerOwnership {
        container: Entity,
        owner: Option<Option<Entity>>,
        communal: Option<Option<SocietyHandle>>,
    },

    ExecuteScript(PathBuf),
}

#[derive(Debug)]
pub enum DivineInputCommand {
    Goto(WorldPosition),
    Break(WorldPosition),
}

#[derive(Copy, Clone, PartialEq)]
pub enum BlockPlacement {
    Set,
    PlaceAbove,
}
