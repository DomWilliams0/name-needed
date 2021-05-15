use crate::render::sdl::ui::context::{DefaultOpen, UiContext};
use crate::ui_str;
use imgui::im_str;
use simulation::TICKS_PER_SECOND;

#[derive(Default)]
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
        let node = context.new_tree_node(im_str!("Performance"), DefaultOpen::Open);
        if node.is_open() {
            mk_stat(context, "Tick:  ", perf.tick, 1.0 / TICKS_PER_SECOND as f64);
            mk_stat(context, "Render:", perf.render, 1.0 / 60.0);
        }
    }
}
