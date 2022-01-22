use std::cell::{RefCell, RefMut};
use std::fmt::{Display, Formatter};

use std::ops::Deref;

use imgui::{TabBarFlags, TabBarToken, TabItemToken, TreeNode, TreeNodeToken};

use simulation::input::{UiCommand, UiCommands, UiRequest, UiResponse};
use simulation::{ComponentRef, ComponentWorld, Entity, KindComponent, PerfAvg, SimulationRef};

use crate::render::sdl::ui::memory::PerFrameStrings;

/// Context for a single frame. Provides communication to the game
pub struct UiContext<'ctx> {
    ui: &'ctx imgui::Ui<'ctx>,
    strings: &'ctx PerFrameStrings,
    perf: PerfAvg,
    simulation: SimulationRef<'ctx>,
    commands: RefCell<&'ctx mut UiCommands>,
    cached_entity_logs: RefCell<String>,
}

#[macro_export]
macro_rules! open_or_ret {
    ($token:expr) => {
        match $token {
            Some(t) => t,
            None => return,
        }
    };
}

pub enum DefaultOpen {
    Closed,
    Open,
}

pub enum EntityDesc<'a> {
    Kind(ComponentRef<'a, KindComponent>),
    Overridden(&'a dyn Display),
    Fallback(Entity),
}

impl<'a> UiContext<'a> {
    pub fn new(
        ui: &'a imgui::Ui<'a>,
        strings: &'a PerFrameStrings,
        simulation: SimulationRef<'a>,
        commands: &'a mut UiCommands,
        perf: PerfAvg,
    ) -> Self {
        Self {
            ui,
            strings,
            perf,
            simulation,
            commands: RefCell::new(commands),
            cached_entity_logs: Default::default(),
        }
    }

    pub fn ui(&self) -> &'a imgui::Ui<'a> {
        self.ui
    }

    pub fn strings(&self) -> &PerFrameStrings {
        self.strings
    }

    pub fn perf(&self) -> &PerfAvg {
        &self.perf
    }

    pub fn simulation(&self) -> &SimulationRef {
        &self.simulation
    }

    pub fn issue_request(&self, req: UiRequest) -> UiResponse {
        let command = UiCommand::new(req);
        let response = command.response();
        self.commands.borrow_mut().push(command);
        response
    }

    pub fn new_tab(&self, title: &str) -> Option<TabItemToken> {
        self.ui.tab_item(title)
    }

    pub fn new_tab_bar(&self, id: &str) -> Option<TabBarToken> {
        self.ui
            .tab_bar_with_flags(id, TabBarFlags::FITTING_POLICY_SCROLL)
    }

    pub fn new_tree_node(&self, title: &str, open: DefaultOpen) -> Option<TreeNodeToken> {
        TreeNode::new(title)
            .default_open(matches!(open, DefaultOpen::Open))
            .push(self.ui)
    }

    pub fn description(&self, e: Entity) -> EntityDesc {
        match self.simulation.ecs.component::<KindComponent>(e) {
            Ok(comp) => EntityDesc::Kind(comp),
            Err(_) => EntityDesc::Fallback(e),
        }
    }

    pub fn entity_log_cached_string_mut(&self) -> RefMut<String> {
        self.cached_entity_logs.borrow_mut()
    }
}

impl Display for EntityDesc<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            EntityDesc::Kind(kind) => &**kind as &dyn Display,
            EntityDesc::Overridden(d) => d,
            EntityDesc::Fallback(e) => e as &dyn Display,
        };

        Display::fmt(display, f)
    }
}

impl<'a> EntityDesc<'a> {
    pub fn with_fallback(self, fallback: &'a dyn Display) -> Self {
        match self {
            EntityDesc::Fallback(_) => EntityDesc::Overridden(fallback),
            other => other,
        }
    }
}

impl<'a> Deref for UiContext<'a> {
    type Target = imgui::Ui<'a>;

    fn deref(&self) -> &Self::Target {
        self.ui
    }
}
