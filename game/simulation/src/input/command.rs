use unit::world::WorldPosition;
use world::block::BlockType;

use crate::ecs::Entity;
use crate::scripting::ScriptingError;
use crate::society::job::SocietyCommand;
use crate::{Exit, SocietyHandle};
use common::*;

use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;

/// Command from the player through the UI
pub enum UiRequest {
    ExitGame(Exit),

    DisableAllDebugRenderers,

    SetDebugRendererEnabled {
        ident: Cow<'static, str>,
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

    /// Eval the script at the given path
    ExecuteScript(PathBuf),
}

pub enum UiResponsePayload {
    NoneExpected,

    ScriptOutput(Result<String, ScriptingError>),
}

pub struct UiCommand {
    req: UiRequest,
    /// Optional depending on type of request
    response: UiResponse,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct UiResponse {
    /// None if no response yet
    resp: Rc<parking_lot::Mutex<Option<UiResponsePayload>>>,
}

pub type UiCommands = Vec<UiCommand>;

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

impl UiCommand {
    pub fn new(req: UiRequest) -> UiCommand {
        Self {
            req,
            response: UiResponse {
                resp: Default::default(),
            },
        }
    }

    pub fn response(&self) -> UiResponse {
        self.response.clone()
    }

    pub fn consume(self) -> (UiRequest, UiResponse) {
        (self.req, self.response)
    }
}

impl UiResponse {
    pub fn has_response(&self) -> bool {
        self.resp.lock().is_some()
    }

    pub fn take_response(&self) -> Option<UiResponsePayload> {
        self.resp.lock().take()
    }

    pub fn set_response(&self, payload: UiResponsePayload) {
        let mut resp = self.resp.lock();
        debug_assert!(resp.is_none(), "response is already non none");

        *resp = Some(payload);
    }
}

impl Display for UiResponsePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use UiResponsePayload::*;
        match self {
            NoneExpected => Ok(()),
            ScriptOutput(res) => match res {
                Ok(s) => write!(f, "{}", s),
                Err(err) => write!(f, "Error: {}", err),
            },
        }
    }
}
