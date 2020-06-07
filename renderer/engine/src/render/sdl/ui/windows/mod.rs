mod debug;
mod perf;
mod selection;

use crate::render::sdl::ui::memory::PerFrameStrings;
use simulation::input::{Blackboard, InputCommand};
use simulation::PerfAvg;

pub struct UiBundle<'a> {
    pub ui: &'a imgui::Ui<'a>,
    pub strings: &'a PerFrameStrings,
    pub perf: &'a PerfAvg,
    pub blackboard: &'a Blackboard<'a>,
    pub commands: &'a mut Vec<InputCommand>,
}

pub(crate) use debug::DebugWindow;
pub(crate) use perf::PerformanceWindow;
pub(crate) use selection::SelectionWindow;
