use simulation::input::{UiCommand, UiCommands, UiRequest, UiResponse};
use simulation::{PerfAvg, SimulationRef};

use crate::render::sdl::ui::memory::PerFrameStrings;
use std::ops::Deref;

/// Context for a single frame. Provides communication to the game
pub struct UiContext<'a> {
    ui: &'a imgui::Ui<'a>,
    strings: &'a PerFrameStrings,
    perf: PerfAvg,
    simulation: SimulationRef<'a>,
    commands: &'a mut UiCommands,
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
            commands,
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

    pub fn issue_request(&mut self, req: UiRequest) -> UiResponse {
        let command = UiCommand::new(req);
        let response = command.response();
        self.commands.push(command);
        response
    }
}

impl<'a> Deref for UiContext<'a> {
    type Target = imgui::Ui<'a>;

    fn deref(&self) -> &Self::Target {
        self.ui
    }
}
