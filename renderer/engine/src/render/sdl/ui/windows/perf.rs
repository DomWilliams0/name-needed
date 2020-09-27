use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;
use imgui::{im_str, TreeNode};
use simulation::TICKS_PER_SECOND;

pub struct PerformanceWindow;

fn mk_stat(bundle: &UiBundle, what: &'static str, value: f64, danger_limit: f64) {
    let ms = value * 1000.0;
    let string = ui_str!(in bundle.strings, "{:7} {:.3}ms", what, ms);
    if value >= danger_limit {
        bundle.ui.text_colored([0.89, 0.11, 0.11, 1.0], string);
    } else {
        bundle.ui.text(string)
    }
}

impl PerformanceWindow {
    pub fn render(&mut self, bundle: &UiBundle) {
        TreeNode::new(im_str!("Performance"))
            .default_open(true)
            .build(&bundle.ui, || {
                mk_stat(
                    bundle,
                    "Tick:  ",
                    bundle.perf.tick,
                    1.0 / TICKS_PER_SECOND as f64,
                );
                mk_stat(bundle, "Render:", bundle.perf.render, 1.0 / 60.0);
            });
    }
}
