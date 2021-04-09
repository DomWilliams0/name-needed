use imgui::{im_str, ImStr, TabItem};

use simulation::input::UiCommand;

use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;

pub struct DebugWindow;

impl UiBundle<'_> {
    fn checkbox(&mut self, title: &ImStr, ident: &'static str) {
        let mut enabled = self.blackboard.enabled_debug_renderers.contains(ident);
        if self.ui.checkbox(title, &mut enabled) {
            self.commands
                .push(UiCommand::ToggleDebugRenderer { ident, enabled })
        }
    }
}

impl DebugWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        TabItem::new(im_str!("Debug"))
            .build(bundle.ui, || {
                let view_range = bundle.blackboard.world_view.expect("blackboard world view range not populated");
                // TODO helpers in Bundle
                bundle.ui.text(ui_str!(in bundle.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

                bundle.ui.separator();

                bundle.checkbox(im_str!("Navigation paths"), "navigation path");
                bundle.checkbox(im_str!("Navigation areas"), "navigation areas");
                bundle.checkbox(im_str!("Steering direction"), "steering");
                bundle.checkbox(im_str!("Senses"), "senses");
                bundle.checkbox(im_str!("Feature boundaries"), "feature boundaries");

            });
    }
}
