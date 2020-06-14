use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;
use imgui::{im_str, CollapsingHeader, Condition, ImStr, Window};
use simulation::input::InputCommand;

pub struct DebugWindow;

fn checkbox(bundle: &mut UiBundle, title: &ImStr, ident: &'static str) {
    let mut enabled = bundle.blackboard.enabled_debug_renderers.contains(ident);
    if bundle.ui.checkbox(title, &mut enabled) {
        bundle
            .commands
            .push(InputCommand::ToggleDebugRenderer { ident, enabled })
    }
}

impl DebugWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        Window::new(im_str!("Debug"))
            .position([250.0, 10.0], Condition::Appearing)
            .always_auto_resize(true)
            .bg_alpha(0.65)
            .build(bundle.ui, || {
                let view_range = bundle.blackboard.world_view.expect("blackboard world view range not populated");
                // TODO helpers in Bundle
                bundle.ui.text(ui_str!(in bundle.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

                if CollapsingHeader::new(im_str!("Debug renderers"))
                    .default_open(true)
                    .build(bundle.ui) {
                    checkbox(bundle, im_str!("Navigation paths"), "navigation path");
                    checkbox(bundle, im_str!("Steering direction"), "steering");
                }

            });
    }
}
