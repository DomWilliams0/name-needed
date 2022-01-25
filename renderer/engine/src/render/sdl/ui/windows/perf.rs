use crate::render::sdl::ui::context::{DefaultOpen, UiContext};
use crate::{open_or_ret, ui_str};

use serde::{Deserialize, Serialize};
use simulation::TICKS_PER_SECOND;

#[derive(Default, Serialize, Deserialize)]
pub struct PerformanceWindow;

fn mk_stat(context: &UiContext, what: &'static str, value: f64, danger_limit: f64) {
    let ms = value * 1000.0;
    let string = ui_str!(in context, "{:7} {:.3}ms", what, ms);
    if value >= danger_limit {
        context.text_colored([0.89, 0.11, 0.11, 1.0], string);
    } else {
        context.text(string)
    }
}

impl PerformanceWindow {
    pub fn render(&mut self, context: &UiContext) {
        let perf = context.perf();
        let _node = open_or_ret!(context.new_tree_node("Performance", DefaultOpen::Open));

        mk_stat(context, "Tick:  ", perf.tick, 1.0 / TICKS_PER_SECOND as f64);
        mk_stat(context, "Render:", perf.render, 1.0 / 60.0);
    }
}
