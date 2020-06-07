use crate::render::sdl::ui::windows::UiBundle;
use imgui::{im_str, Condition, ImStr, Window};
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
                checkbox(bundle, im_str!("Navigation paths"), "navigation path");
                checkbox(bundle, im_str!("Steering direction"), "steering");
            });
    }
}
