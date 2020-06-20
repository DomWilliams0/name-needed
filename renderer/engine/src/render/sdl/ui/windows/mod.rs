use imgui::{ImStr, Ui};

use simulation::input::{Blackboard, InputCommand};
use simulation::PerfAvg;

use crate::render::sdl::ui::memory::PerFrameStrings;

mod debug_renderer;
mod perf;
mod selection;

pub(crate) use debug_renderer::DebugWindow;
pub(crate) use perf::PerformanceWindow;
pub(crate) use selection::SelectionWindow;

pub struct UiBundle<'a> {
    pub ui: &'a imgui::Ui<'a>,
    pub strings: &'a PerFrameStrings,
    pub perf: &'a PerfAvg,
    pub blackboard: &'a Blackboard<'a>,
    pub commands: &'a mut Vec<InputCommand>,
}

enum Value<'a> {
    Hide,
    None(&'static str),
    Some(&'a ImStr),
    Wrapped(&'a ImStr),
}

trait UiExt {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(&'a self, key: &ImStr, value: F, color: [f32; 4]);
}

impl UiExt for Ui<'_> {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(&'a self, key: &ImStr, value: F, color: [f32; 4]) {
        let value = value();
        if let Value::Hide = value {
            return;
        }

        self.text_colored(color, key);
        self.same_line_with_spacing(self.calc_text_size(key, false, 0.0)[0], 40.0);
        match value {
            Value::Some(val) => {
                self.text(val);
            }
            Value::Wrapped(val) => {
                self.text_wrapped(&val);
            }
            Value::None(val) => self.text_disabled(val),
            _ => unreachable!(),
        }
    }
}

const COLOR_GREEN: [f32; 4] = [0.4, 0.77, 0.33, 1.0];
const COLOR_ORANGE: [f32; 4] = [1.0, 0.46, 0.2, 1.0];
const COLOR_BLUE: [f32; 4] = [0.2, 0.66, 1.0, 1.0];
